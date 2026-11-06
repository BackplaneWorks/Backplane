//! The phone data API over a real socket (JSON frames, and CBOR for the default), with fake project/thread handlers that commit what the real ones commit.
use super::*;
use crate::api::{Caller, Fut, RpcHandler};
use crate::hub::Cfg;
use crate::presenter::Canned;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;

struct Fake(Arc<Hub>);
impl RpcHandler for Fake {
    fn methods(&self) -> &'static [&'static str] { &["project.add", "thread.create", "thread.modes", "thread.effort", "thread.provider", "ask.answer", "turn.interrupt", "thread.pin", "thread.rename", "thread.detach", "setting.set"] }
    fn call<'a>(&'a self, h: &'a Arc<Hub>, _w: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let _ = &self.0; let at = now_ms() / 1000;
            let n = h.store.len();
            match m {
                "project.add" => { let root = s(p, "path"); h.commit(&json!({"$":"ProjectCreated","id":format!("p{n}"),"title":root.rsplit('/').next().unwrap_or(""),"root":root,"at":at})).await.unwrap(); vec![json!({"t":"reply","id":id,"ok":true})] }
                "thread.create" => { let t = format!("t{n}"); h.commit(&json!({"$":"ThreadCreated","id":t,"project":s(p,"project"),"title":if s(p,"title").is_empty(){"New thread"}else{s(p,"title")},"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at})).await.unwrap(); vec![json!({"t":"reply","id":id,"ok":true,"thread":t})] }
                "ask.answer" => { h.commit(&json!({"$":"AskClosed","id":s(p,"id"),"answer":s(p,"answer"),"at":at})).await.unwrap(); vec![json!({"t":"reply","id":id,"ok":true})] }
                "thread.modes" | "thread.effort" | "thread.provider" | "turn.interrupt" | "thread.pin" | "thread.rename" | "thread.detach" | "setting.set" => vec![json!({"t":"reply","id":id,"ok":true})],
                _ => vec![json!({"t":"reply","id":id,"ok":false,"error":"no"})],
            }
        })
    }
}
const LOG: &str = r#"{"$":"ProjectCreated","id":"p1","title":"alpha","root":"/tmp/alpha","at":1700000000}
{"$":"ProjectCreated","id":"p2","title":"beta","root":"/tmp/beta","at":1700000001}
{"$":"ThreadCreated","id":"t1","project":"p1","title":"First","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000010}
{"$":"ThreadCreated","id":"t2","project":"p1","title":"Second","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000020}
{"$":"ThreadCreated","id":"t3","project":"p2","title":"Third","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000030}
{"$":"MessagePosted","thread":"t1","msg":"m1","role":"user","text":"hello there","at":1700000011}
{"$":"MessagePosted","thread":"t1","msg":"m2","role":"assistant","text":"hi, how can I help with this long reply that goes on","at":1700000012}
{"$":"ActivityLogged","thread":"t1","act":"a1","tone":"tool","kind":"Bash","summary":"ls -la","at":1700000013}
{"$":"AskOpened","id":"q1","thread":"t1","kind":"approval","tool":"Bash","detail":"rm -rf x","options":"","at":1700000014}
{"$":"ThreadArchived","id":"t3","archived":true,"at":1700000031}
"#;
async fn up(name: &str) -> (Arc<Hub>, u16) {
    let dir = std::env::temp_dir().join(format!("bp-data-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("token"), "tok").unwrap(); std::fs::write(dir.join("events.jsonl"), LOG).unwrap();
    let cfg = Cfg { home: dir, version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
    let hub = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    hub.register_rpc(Arc::new(Fake(hub.clone())));
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    { let hub = hub.clone(); tokio::spawn(async move { loop { let (st, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = wire::serve_with(h, st, false).await; }); } }); }
    (hub, port)
}
type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;
async fn open(port: u16, q: &str) -> Ws { connect_async(format!("ws://127.0.0.1:{port}/ws?data=1{q}&token=tok")).await.unwrap().0 }
async fn next(ws: &mut Ws) -> Value {
    loop {
        let m = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.expect("a frame in time").unwrap().unwrap();
        let v = match m { Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(), Message::Binary(b) => crate::cbor::decode(&b).unwrap(), _ => continue };
        if v["t"] != "hb" { return v }
    }
}
async fn ask(ws: &mut Ws, id: u64, m: &str, p: Value) -> Value {
    ws.send(Message::Text(json!({"id":id,"m":m,"p":p}).to_string().into())).await.unwrap();
    loop { let v = next(ws).await; if v["t"] == "reply" && v["id"] == id { return v } }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_hello_capabilities_and_both_encodings() {
    assert_eq!(crate::hub::capabilities()["data"]["proto"], "data/1"); assert_eq!(crate::hub::capabilities()["data"]["features"], json!(["images", "actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker", "voice", "order"]));
    let (_h, port) = up("hello").await;
    let mut ws = open(port, "&enc=json").await; let v = next(&mut ws).await;
    assert_eq!((v["t"].as_str(), v["proto"].as_str()), (Some("hello"), Some("data/1"))); assert_eq!(v["hub"], format!("127.0.0.1:{port}"), "the hub as asked (the Host header)"); assert_eq!(v["features"], json!(["images", "actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker", "voice", "order"]), "a hub from before images does not list it"); assert!(v["now"].as_u64().unwrap() > 1_600_000_000_000);
    // the default is binary CBOR with plain string keys
    let mut c = open(port, "").await; let m = tokio::time::timeout(Duration::from_secs(5), c.next()).await.unwrap().unwrap().unwrap();
    let Message::Binary(b) = m else { panic!("binary") }; assert!(b.windows(5).any(|w| w == b"proto"), "plain keys, not the hub's dictionary");
    assert!(b.windows(6).any(|w| w == b"data/1"), "hello names the protocol"); // (the hub's own decoder reads at most 32-bit widths; `now` is a 64-bit millisecond count)
    // no credential, no socket
    assert!(connect_async(format!("ws://127.0.0.1:{port}/ws?data=1&enc=json")).await.is_err());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_lists_page_newest_first_and_count_what_waits() {
    let (_h, port) = up("lists").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let r = ask(&mut ws, 1, "projects.list", json!({})).await; let ps = r["result"]["projects"].as_array().unwrap();
    assert_eq!(ps.iter().map(|p| p["id"].as_str().unwrap()).collect::<Vec<_>>(), ["p2", "p1"].iter().map(|x| *x).collect::<Vec<_>>().iter().map(|x| *x).collect::<Vec<_>>(), "recent first: p2 holds the newest thread");
    let p1 = ps.iter().find(|p| p["id"] == "p1").unwrap(); assert_eq!((p1["threadCount"].as_u64(), p1["waiting"].as_u64(), p1["root"].as_str()), (Some(2), Some(1), Some("/tmp/alpha")));
    let r = ask(&mut ws, 2, "threads.list", json!({"limit":1})).await; let ts = &r["result"]["threads"]; assert_eq!(ts[0]["id"], "t2"); let next_c = r["result"]["next"].as_str().unwrap().to_string();
    let r = ask(&mut ws, 3, "threads.list", json!({"limit":1,"before":next_c})).await; let t = &r["result"]["threads"][0]; assert_eq!(t["id"], "t1"); assert!(r["result"]["next"].is_null());
    assert_eq!((t["state"].as_str(), t["waiting"].as_u64(), t["preview"].as_str()), (Some("asking"), Some(1), Some("hi, how can I help with this long reply that goes on")), "an open ask makes the thread asking; the preview is the last thing said");
    let r = ask(&mut ws, 4, "threads.list", json!({"archived":true})).await; assert_eq!(r["result"]["threads"].as_array().unwrap().len(), 1); assert_eq!(r["result"]["threads"][0]["id"], "t3");
    let r = ask(&mut ws, 5, "threads.list", json!({"project":"p2"})).await; assert!(r["result"]["threads"].as_array().unwrap().is_empty(), "t3 is archived");
    let r = ask(&mut ws, 6, "nope", json!({})).await; assert_eq!(r["ok"], false);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_entries_newest_page_first_with_asks_and_the_live_reply() {
    let (hub, port) = up("entries").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let r = ask(&mut ws, 1, "thread.entries", json!({"thread":"t1","limit":2})).await; let e = r["result"]["entries"].as_array().unwrap();
    let kinds: Vec<&str> = e.iter().map(|x| x["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["assistant", "tool", "ask"], "the newest page, oldest to newest, with the open ask last: {e:?}");
    assert_eq!(e[1]["tool"], json!({"name":"Bash","summary":"ls -la","state":"done"}));
    let a = &e[2]["ask"]; assert_eq!((a["id"].as_str(), a["state"].as_str()), (Some("q1"), Some("open"))); assert_eq!(a["options"][0]["id"], "accept"); assert!(a["question"].as_str().unwrap().contains("rm -rf x"));
    let cursor = r["result"]["next"].as_str().unwrap().to_string();
    let r = ask(&mut ws, 2, "thread.entries", json!({"thread":"t1","before":cursor})).await; let e = r["result"]["entries"].as_array().unwrap();
    assert_eq!((e.len(), e[0]["kind"].as_str(), e[0]["text"].as_str()), (1, Some("user"), Some("hello there"))); assert!(r["result"]["next"].is_null());
    // a late joiner gets the reply so far, marked live
    hub.live_delta("t1", "partial text").await.unwrap();
    let r = ask(&mut ws, 3, "thread.entries", json!({"thread":"t1"})).await; let last = r["result"]["entries"].as_array().unwrap().last().unwrap().clone();
    assert_eq!((last["id"].as_str(), last["live"].as_bool(), last["text"].as_str()), (Some("live:t1"), Some(true), Some("partial text")));
    assert_eq!(ask(&mut ws, 4, "thread.entries", json!({"thread":"nope"})).await["ok"], false);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_writes_make_a_project_a_thread_and_a_message_once() {
    let (hub, port) = up("writes").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let d = ask(&mut ws, 1, "dirs.list", json!({"path":"/"})).await; assert_eq!(d["result"]["path"], "/"); assert!(d["result"]["parent"].is_null()); assert!(!d["result"]["dirs"].as_array().unwrap().is_empty());
    let dir = std::env::temp_dir().join(format!("bp-data-proj-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
    let r = ask(&mut ws, 2, "project.add", json!({"path":dir.to_string_lossy()})).await; assert_eq!(r["ok"], true, "{r}");
    let pid = r["result"]["project"]["id"].as_str().unwrap().to_string();
    let r = ask(&mut ws, 3, "thread.create", json!({"project":pid,"title":"Mine","text":"first words","permission":"ask"})).await; assert_eq!(r["ok"], true, "{r}");
    let th = r["result"]["thread"]["id"].as_str().unwrap().to_string(); assert_eq!(r["result"]["thread"]["title"], "Mine");
    let e = ask(&mut ws, 4, "thread.entries", json!({"thread":th})).await; assert_eq!(e["result"]["entries"][0]["text"], "first words");
    let a = ask(&mut ws, 5, "message.send", json!({"thread":th,"text":"again","clientId":"c-1"})).await; assert_eq!((a["result"]["entry"]["clientId"].as_str(), a["result"]["entry"]["kind"].as_str()), (Some("c-1"), Some("user")));
    let b = ask(&mut ws, 6, "message.send", json!({"thread":th,"text":"again","clientId":"c-1"})).await; assert_eq!(a["result"]["entry"], b["result"]["entry"], "the same clientId posts once");
    let n = ask(&mut ws, 7, "thread.entries", json!({"thread":th})).await["result"]["entries"].as_array().unwrap().iter().filter(|x| x["text"] == "again").count(); assert_eq!(n, 1);
    assert_eq!(ask(&mut ws, 8, "message.send", json!({"thread":th,"text":"  "})).await["ok"], false);
    assert_eq!(ask(&mut ws, 9, "ask.answer", json!({"ask":"q1","answer":"accept"})).await["ok"], true);
    let r = ask(&mut ws, 10, "thread.entries", json!({"thread":"t1"})).await; assert!(r["result"]["entries"].as_array().unwrap().iter().all(|x| x["kind"] != "ask"), "an answered ask is no longer open");
    let _ = hub;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_subscriptions_push_entries_text_turns_and_list_changes() {
    let (hub, port) = up("subs").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let lists = ask(&mut ws, 1, "subscribe", json!({"topic":"lists"})).await["result"]["sub"].as_u64().unwrap();
    let thr = ask(&mut ws, 2, "subscribe", json!({"topic":"thread","thread":"t1"})).await["result"]["sub"].as_u64().unwrap();
    assert_eq!(ask(&mut ws, 3, "subscribe", json!({"topic":"thread","thread":"zz"})).await["ok"], false);
    hub.commit(&json!({"$":"TurnChanged","thread":"t1","state":"running","at":1700000100})).await.unwrap();
    let mut got = vec![]; for _ in 0..2 { got.push(next(&mut ws).await) }
    assert!(got.iter().any(|v| v["k"] == "turn" && v["sub"] == thr && v["state"] == "working"), "{got:?}");
    assert!(got.iter().any(|v| v["k"] == "thread" && v["sub"] == lists && v["thread"]["state"] == "asking"), "an open ask outranks a running turn: {got:?}");
    // streamed text: the live entry first, then the appends, then the final entry replaces it
    hub.live_delta("t1", "Hel").await.unwrap(); hub.live_delta("t1", "lo").await.unwrap();
    async fn kind(ws: &mut Ws, k: &str) -> Value { loop { let v = next(ws).await; if v["k"] == k { return v } } } // (the thread's own state pushes come between)
    let a = kind(&mut ws, "entry").await; assert_eq!((a["k"].as_str(), a["entry"]["id"].as_str(), a["entry"]["live"].as_bool()), (Some("entry"), Some("live:t1"), Some(true)), "{a}");
    let t1 = kind(&mut ws, "text").await; let t2 = kind(&mut ws, "text").await; assert_eq!((t1["k"].as_str(), t1["append"].as_str(), t2["append"].as_str()), (Some("text"), Some("Hel"), Some("lo")));
    hub.live_end("t1").await.unwrap();
    hub.commit(&json!({"$":"MessagePosted","thread":"t1","msg":"m9","role":"assistant","text":"Hello","at":1700000101})).await.unwrap();
    let f = loop { let v = next(&mut ws).await; if v["k"] == "entry" { break v } };
    assert_eq!((f["entry"]["id"].as_str(), f["entry"]["replaces"].as_str(), f["entry"]["live"].is_null()), (Some("m9"), Some("live:t1"), true), "{f}");
    // a new thread and a project reach the lists subscription
    hub.commit(&json!({"$":"ThreadCreated","id":"t9","project":"p2","title":"Brand new","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000200})).await.unwrap();
    let n = loop { let v = next(&mut ws).await; if v["k"] == "thread" && v["thread"]["id"] == "t9" { break v } }; assert_eq!(n["sub"], lists);
    hub.commit(&json!({"$":"ThreadDeleted","id":"t9","at":1700000201})).await.unwrap();
    let g = loop { let v = next(&mut ws).await; if v["k"] == "thread.removed" { break v } }; assert_eq!(g["id"], "t9");
    assert_eq!(ask(&mut ws, 4, "unsubscribe", json!({"sub":thr})).await["ok"], true);
    hub.commit(&json!({"$":"MessagePosted","thread":"t1","msg":"m10","role":"user","text":"quiet","at":1700000300})).await.unwrap();
    hub.commit(&json!({"$":"ThreadCreated","id":"t10","project":"p2","title":"x","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000301})).await.unwrap();
    let v = loop { let v = next(&mut ws).await; if v["k"] == "thread" || v["k"] == "entry" { break v } }; assert_eq!(v["k"], "thread", "no entry push after unsubscribing the thread: {v}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_an_answered_ask_reaches_the_thread_subscription_whole() {
    let (hub, port) = up("askclosed").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let thr = ask(&mut ws, 1, "subscribe", json!({"topic":"thread","thread":"t1"})).await["result"]["sub"].as_u64().unwrap();
    assert_eq!(ask(&mut ws, 2, "ask.answer", json!({"ask":"q1","answer":"decline"})).await["ok"], true);
    let v = loop { let v = next(&mut ws).await; if v["k"] == "entry" && v["entry"]["kind"] == "ask" { break v } };
    assert_eq!(v["sub"], thr); let a = &v["entry"]["ask"];
    assert_eq!((a["id"].as_str(), a["state"].as_str(), a["answered"].as_str()), (Some("q1"), Some("answered"), Some("decline")), "{v}");
    assert!(a["question"].as_str().unwrap().contains("rm -rf x"), "the question stays: {v}"); assert_eq!(a["options"][0]["id"], "accept");
    assert_eq!(v["entry"]["id"], "ask:q1", "the same entry id as when it opened"); let _ = hub;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_images_are_stored_told_to_the_agent_and_read_back() {
    use base64::Engine; let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    let (hub, port) = up("images").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let png: Vec<u8> = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 1, 2, 3, 4];
    let r = ask(&mut ws, 1, "message.send", json!({"thread":"t2","text":"see this","clientId":"img-1","attachments":[{"name":"a b.png","mime":"image/png","data":b64(&png)},{"name":"c.jpg","mime":"image/jpeg","data":b64(b"jpegbytes")}]})).await;
    assert_eq!(r["ok"], true, "{r}"); let e = &r["result"]["entry"]; assert_eq!((e["clientId"].as_str(), e["text"].as_str()), (Some("img-1"), Some("see this")));
    let at = e["attachments"].as_array().unwrap(); assert_eq!(at.len(), 2); assert_eq!((at[0]["name"].as_str(), at[0]["mime"].as_str(), at[0]["bytes"].as_u64()), (Some("a b.png"), Some("image/png"), Some(png.len() as u64)));
    let id = at[0]["id"].as_str().unwrap().to_string(); assert!(id.starts_with("t2~") && !id.contains('/') && !id.contains(' '), "opaque and path-safe: {id}");
    // the file is in the thread's attachments folder, private, and the agent is told its path while the message keeps the person's words
    let dir = hub.cfg.home.join("attachments").join("t2"); let files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect(); assert_eq!(files.len(), 2);
    { use std::os::unix::fs::PermissionsExt; assert_eq!(files[0].metadata().unwrap().permissions().mode() & 0o777, 0o600); }
    let log = (0..hub.store.len()).map(|i| hub.store.get(i).unwrap()).collect::<Vec<_>>();
    let posted: Value = serde_json::from_str(log.iter().rev().find(|l| l.contains("\"MessagePosted\"") && l.contains("img-1")).unwrap()).unwrap();
    let queued: Value = serde_json::from_str(log.iter().rev().find(|l| l.contains("\"TurnQueued\"") && l.contains("img-1")).unwrap()).unwrap();
    assert_eq!(posted["text"], "see this"); assert!(posted["attachments"].is_array());
    assert!(queued["text"].as_str().unwrap().starts_with("see this\n\n[Attached image: ") && queued["text"].as_str().unwrap().contains(&dir.to_string_lossy().to_string()), "{queued}");
    // a thread read shows them, and attachment.get returns the bytes
    let en = ask(&mut ws, 2, "thread.entries", json!({"thread":"t2"})).await; let last = en["result"]["entries"].as_array().unwrap().iter().find(|x| x["id"] == "img-1").unwrap().clone();
    assert_eq!(last["attachments"].as_array().unwrap().len(), 2);
    let g = ask(&mut ws, 3, "attachment.get", json!({"id":id})).await; assert_eq!(g["result"]["mime"], "image/png");
    assert_eq!(base64::engine::general_purpose::STANDARD.decode(g["result"]["data"].as_str().unwrap()).unwrap(), png);
    // refused: a path out of the folder, an unknown id, too many, a wrong type, bad base64, an empty message
    for bad in ["../x", "t2~../../token", "t2~", "t2", "t2~nope.png", "/etc/passwd", "t2/x~y"] { assert_eq!(ask(&mut ws, 9, "attachment.get", json!({"id":bad})).await["ok"], false, "{bad}") }
    let img = json!({"name":"x.png","mime":"image/png","data":b64(&png)});
    assert_eq!(ask(&mut ws, 10, "message.send", json!({"thread":"t2","text":"x","attachments":[img,img,img,img,img]})).await["ok"], false, "five images");
    assert_eq!(ask(&mut ws, 11, "message.send", json!({"thread":"t2","text":"x","attachments":[{"name":"x.pdf","mime":"application/pdf","data":b64(&png)}]})).await["ok"], false);
    assert_eq!(ask(&mut ws, 12, "message.send", json!({"thread":"t2","text":"x","attachments":[{"name":"x.png","mime":"image/png","data":"!!!"}]})).await["ok"], false);
    assert_eq!(ask(&mut ws, 13, "message.send", json!({"thread":"t2","text":"","attachments":[]})).await["ok"], false);
    assert_eq!(ask(&mut ws, 14, "message.send", json!({"thread":"zz","text":"x","attachments":[img]})).await["ok"], false, "no files for an unknown thread");
    assert!(!hub.cfg.home.join("attachments").join("zz").exists());
    // an image-only message is a message
    assert_eq!(ask(&mut ws, 15, "message.send", json!({"thread":"t2","text":"","attachments":[img]})).await["ok"], true);
    // and a subscriber sees the images on the pushed entry
    ask(&mut ws, 16, "subscribe", json!({"topic":"thread","thread":"t2"})).await;
    ask(&mut ws, 17, "message.send", json!({"thread":"t2","text":"pushed","clientId":"img-2","attachments":[img]})).await;
    let v = loop { let v = next(&mut ws).await; if v["k"] == "entry" && v["entry"]["id"] == "img-2" { break v } };
    assert_eq!(v["entry"]["attachments"].as_array().unwrap().len(), 1); assert_eq!(v["entry"]["clientId"], "img-2");
}
#[test] fn data_plain_cbor_has_string_keys_and_the_hubs_decoder_reads_it_back() {
    let v = json!({"t":"reply","id":7,"ok":true,"result":{"xs":[1,-2,300,70000,"é"],"n":null,"f":false}});
    let mut o = vec![]; cbor_plain(&v, &mut o); assert_eq!(crate::cbor::decode(&o).unwrap(), v);
    assert!(o.windows(6).any(|w| w == b"result"));
}

// the phone additions (data/ext.rs)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_additions_search_models_mentions_directory_and_actions() {
    let (_h, port) = up("ext").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let r = ask(&mut ws, 1, "search", json!({"q":"SECOND"})).await; assert_eq!(r["result"]["threads"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["t2"], "case-insensitive, by title");
    let r = ask(&mut ws, 2, "search", json!({"q":"alpha"})).await; assert_eq!(r["result"]["projects"][0]["id"], "p1"); assert_eq!(r["result"]["threads"].as_array().unwrap().len(), 2, "a project's name finds its threads");
    assert!(ask(&mut ws, 3, "search", json!({"q":""})).await["result"]["threads"].as_array().unwrap().is_empty());
    let r = ask(&mut ws, 4, "models.list", json!({})).await; assert!(r["result"]["models"]["claude"].as_array().map_or(false, |a| !a.is_empty()), "claude's models are built in");
    let r = ask(&mut ws, 5, "mentions.list", json!({"q":"fir","kind":">"})).await; assert_eq!(r["result"]["items"][0]["label"], "First"); assert_eq!(r["result"]["items"][0]["sigil"], ">");
    let r = ask(&mut ws, 6, "mentions.list", json!({"q":"alp","kind":"%"})).await; assert_eq!(r["result"]["items"][0]["id"], "p1");
    let r = ask(&mut ws, 7, "bots.list", json!({})).await; assert_eq!((r["result"]["bots"].as_array().unwrap().len(), r["result"]["rooms"].as_array().unwrap().len()), (0, 0), "no bots lane on this hub: empty, not an error");
    let r = ask(&mut ws, 8, "thread.act", json!({"thread":"t1","action":"pin"})).await; assert_eq!(r["ok"], true); assert_eq!(r["result"]["thread"]["id"], "t1");
    let r = ask(&mut ws, 9, "thread.act", json!({"thread":"t1","action":"explode"})).await; assert_eq!(r["ok"], false);
    let r = ask(&mut ws, 10, "thread.act", json!({"thread":"nope","action":"pin"})).await; assert_eq!(r["ok"], false);
    let r = ask(&mut ws, 11, "thread.act", json!({"thread":"t1","action":"rename","title":"  "})).await; assert_eq!(r["ok"], false, "a blank title is refused before any request");
    // the badge and Move to top level
    let r = ask(&mut ws, 12, "badge.list", json!({})).await; assert_eq!((r["result"]["colors"][0]["name"].as_str(), r["result"]["icons"][0].as_str()), (Some("blue"), Some("folder")));
    assert_eq!(ask(&mut ws, 13, "project.act", json!({"project":"p1","action":"color","value":"pink"})).await["ok"], true);
    assert_eq!(ask(&mut ws, 14, "project.act", json!({"project":"p1","action":"icon","value":"cpu"})).await["ok"], true);
    assert_eq!(ask(&mut ws, 15, "project.act", json!({"project":"p1","action":"color","value":"mauve"})).await["ok"], false, "only a tint of the list");
    assert_eq!(ask(&mut ws, 16, "project.act", json!({"project":"p1","action":"icon","value":"skull"})).await["ok"], false);
    assert_eq!(ask(&mut ws, 17, "thread.act", json!({"thread":"t2","action":"detach"})).await["ok"], true);
    let r = ask(&mut ws, 18, "search", json!({"q":"alpha"})).await; let p = &r["result"]["projects"][0];
    assert!(p["css"].as_str().unwrap().starts_with('#') && !p["color"].as_str().unwrap().is_empty() && p["icon"] == "folder", "a project carries its badge: {p}");
    assert_eq!(r["result"]["threads"][0]["parent"], "", "a thread says what it is under");
}
#[test] fn notify_for_marks_asks_and_turn_ends_only() {
    let t = |k: &str| ext::notify_for(&json!({"$":"TurnChanged","state":k}), "t1", "T", "");
    assert_eq!(t("completed").unwrap()["kind"], "done"); assert_eq!(t("failed").unwrap()["kind"], "fail");
    assert!(t("running").is_none() && t("interrupted").is_none() && t("cancelled").is_none());
    assert_eq!(ext::notify_for(&json!({"$":"AskOpened"}), "t1", "T", "rm -rf x").unwrap()["body"], "rm -rf x");
    assert!(ext::notify_for(&json!({"$":"MessagePosted"}), "t1", "T", "").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_activity_arrives_ready_to_fold_in_the_desktops_words() {
    let (hub, port) = up("fold").await;
    for l in [r#"{"$":"ActivityLogged","thread":"t2","act":"a2","tone":"tool","kind":"mcp__backplane__viewer_show","summary":"open the board","at":1700000021}"#,
              r#"{"$":"ActivityLogged","thread":"t2","act":"a3","tone":"tool","kind":"TodoWrite","summary":"TodoWrite","at":1700000022}"#,
              r#"{"$":"ActivityLogged","thread":"t2","act":"a4","tone":"step","kind":"step","summary":"{\"p\":\"\",\"w\":\"start\"}","at":1700000023}"#,
              r#"{"$":"ActivityLogged","thread":"t2","act":"a5","tone":"step","kind":"step","summary":"{\"p\":\"abc\",\"w\":\"end\",\"f\":[\"M\\tboard.kicad_pcb\"]}","at":1700000024}"#,
              r#"{"$":"ActivityLogged","thread":"t2","act":"a6","tone":"error","kind":"result","summary":"","at":1700000025}"#,
              r#"{"$":"MessagePosted","thread":"t2","msg":"m9","role":"assistant","text":"  ","at":1700000026}"#] {
        hub.commit(&serde_json::from_str::<Value>(l).unwrap()).await.unwrap();
    }
    let mut ws = open(port, "&enc=json").await; let _ = next(&mut ws).await;
    let r = ask(&mut ws, 1, "thread.entries", json!({"thread":"t2"})).await; let es = r["result"]["entries"].as_array().unwrap().clone();
    assert_eq!(es.iter().map(|e| e["id"].as_str().unwrap()).collect::<Vec<_>>(), ["a2", "a3", "a5", "a6"], "the design before the edits and a blank reply are no entries");
    assert_eq!(es[0]["tool"]["name"], "viewer_show"); assert_eq!(es[0]["tool"]["summary"], "open the board");
    assert_eq!(es[1]["tool"]["name"], "To-do"); assert_eq!(es[1]["tool"]["summary"], "", "a summary that only repeats the name says nothing");
    assert_eq!(es[2]["kind"], "step"); assert_eq!(es[2]["label"], "Design change · board.kicad_pcb");
    assert_eq!(es[3]["kind"], "note"); assert_eq!(es[3]["tone"], "error"); assert_eq!(es[3]["text"], "The turn ended with an error; the agent gave no reason.");
    let r = ask(&mut ws, 2, "threads.list", json!({"project":"p1"})).await; let t1 = r["result"]["threads"].as_array().unwrap().iter().find(|t| t["id"] == "t1").unwrap().clone();
    assert_eq!(t1["preview"], "hi, how can I help with this long reply that goes on", "the last thing said, not the tool call after it");
    assert_eq!(t1["chip"], "claude / default / auto");
    let sub = ask(&mut ws, 3, "subscribe", json!({"topic":"thread","thread":"t2"})).await; assert!(sub["ok"] == true);
    hub.commit(&json!({"$":"ActivityLogged","thread":"t2","act":"a7","tone":"step","kind":"step","summary":"{\"p\":\"\",\"w\":\"start\"}","at":1700000030})).await.unwrap();
    hub.commit(&json!({"$":"ActivityLogged","thread":"t2","act":"a8","tone":"tool","kind":"Bash","summary":"ls","at":1700000031})).await.unwrap();
    let v = loop { let v = next(&mut ws).await; if v["k"] == "entry" { break v } };
    assert_eq!(v["entry"]["id"], "a8", "a design base is not pushed either"); assert_eq!(v["entry"]["tool"]["name"], "Bash"); assert_eq!(v["entry"]["at"], 1700000031000u64);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_the_picker_offers_what_the_desktop_does_and_a_pick_is_its_request() {
    let (_hub, port) = up("picker").await;
    let mut ws = open(port, "&enc=json").await; let hello = next(&mut ws).await;
    for f in ["fold", "picker"] { assert!(hello["features"].as_array().unwrap().iter().any(|x| x == f), "{f}"); }
    let r = ask(&mut ws, 1, "thread.picker", json!({"thread":"t1"})).await; let p = &r["result"]["picker"];
    assert_eq!(p["chip"], "claude / default / auto");
    let titles: Vec<&str> = p["sections"].as_array().unwrap().iter().map(|x| x["title"].as_str().unwrap()).collect(); assert_eq!(titles, ["Model", "Effort", "Provider", "Permissions"]);
    let model = &p["sections"][0]["choices"]; assert_eq!(model[0]["label"], "default"); assert_eq!(model[0]["on"], true); assert_eq!(model[2]["value"], "opus"); assert_eq!(model[2]["label"], "Opus 5.5 \u{b7} claude-opus-5-5");
    assert_eq!(p["sections"][2]["choices"][1]["value"], "codex", "a provider's value is its name, without the desktop's @");
    let r = ask(&mut ws, 2, "thread.picker", json!({"provider":"codex"})).await; let p = &r["result"]["picker"];
    assert_eq!(p["chip"], "codex / default / auto"); assert_eq!(p["sections"][0]["choices"].as_array().unwrap().len(), 1, "no listing: its default");
    assert_eq!(p["sections"][3]["choices"][0]["on"], true, "a new thread asks for approvals by default");
    let r = ask(&mut ws, 3, "thread.picker", json!({"provider":"opencode"})).await; assert_eq!(r["result"]["picker"]["sections"][1]["title"], "Provider", "no effort levels, no Effort section");
    for (i, (a, v)) in [("model", "opus"), ("effort", "high"), ("provider", "codex"), ("runtime", "auto")].iter().enumerate() {
        let r = ask(&mut ws, 10 + i as u64, "thread.pick", json!({"thread":"t1","action":a,"value":v})).await; assert_eq!(r["ok"], true, "{a}"); assert_eq!(r["result"]["thread"]["id"], "t1");
    }
    let r = ask(&mut ws, 20, "thread.pick", json!({"thread":"t1","action":"colour","value":"x"})).await; assert_eq!(r["ok"], false);
    let r = ask(&mut ws, 21, "thread.pick", json!({"thread":"nope","action":"model","value":"x"})).await; assert_eq!(r["ok"], false);
    let r = ask(&mut ws, 22, "thread.create", json!({"project":"p1","provider":"claude","model":"opus","effort":"high","runtime":"auto"})).await; assert_eq!(r["ok"], true);
}
#[test] fn the_picker_keeps_a_model_the_list_lacks_and_marks_what_is_in_force() {
    let models = json!({"claude":[{"value":"opus","label":"Opus","ok":true,"why":""}],"codex":[{"value":"gpt","label":"GPT","ok":false,"why":"its CLI is not installed on this machine"}]});
    let p = super::ext::picker_sections(&models, "claude", "old-model", "high", "auto");
    assert_eq!(p["chip"], "claude / old-model / high");
    let m = p["sections"][0]["choices"].as_array().unwrap(); assert_eq!(m.len(), 3); assert_eq!(m[2]["value"], "old-model"); assert_eq!(m[2]["on"], true); assert_eq!(m[0]["on"], false);
    assert!(p["sections"][1]["choices"].as_array().unwrap().iter().any(|c| c["value"] == "high" && c["on"] == true));
    assert!(p["sections"][3]["choices"].as_array().unwrap().iter().any(|c| c["value"] == "auto" && c["on"] == true));
    let p = super::ext::picker_sections(&models, "codex", "gpt", "", "");
    assert_eq!(p["sections"][0]["choices"][1]["ok"], false); assert_eq!(p["sections"][0]["choices"][1]["why"], "its CLI is not installed on this machine");
    assert!(p["sections"][3]["choices"].as_array().unwrap().iter().any(|c| c["value"] == "full-access" && c["on"] == true), "a thread with no mode set runs with full access");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_a_quote_shows_its_real_buy_now_link_and_a_plain_thread_has_no_order() {
    let (hub, port) = up("order").await;
    crate::settings::order::test_link("q_data", "http://127.0.0.1:3871/q/q_data?t=tok");
    hub.commit(&json!({"$":"MessagePosted","thread":"t2","msg":"q:q_data","role":"assistant","text":"**Your quote: $27.20**\n\n[Buy now · $27.20](order:q_data)","at":1700000040})).await.unwrap();
    let mut ws = open(port, "&enc=json").await; let hello = next(&mut ws).await;
    assert!(hello["features"].as_array().unwrap().iter().any(|x| x == "order"));
    let r = ask(&mut ws, 1, "thread.entries", json!({"thread":"t2"})).await;
    let q = r["result"]["entries"].as_array().unwrap().iter().find(|e| e["id"] == "q:q_data").cloned().unwrap_or_else(|| panic!("{r}"));
    assert!(q["text"].as_str().unwrap().contains("](http://127.0.0.1:3871/q/q_data?t=tok)"), "the phone gets the link it can open: {q}");
    let r = ask(&mut ws, 2, "thread.order", json!({"thread":"t1"})).await; assert_eq!(r["result"]["order"], Value::Null, "{r}");
    let r = ask(&mut ws, 3, "thread.order", json!({})).await; assert!(r["error"].is_string() || r["result"].is_null(), "a thread is needed: {r}");
}

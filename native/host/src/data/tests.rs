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
                "thread.create" => { let t = format!("t{n}"); h.commit(&json!({"$":"ThreadCreated","id":t,"project":s(p,"project"),"title":if s(p,"title").is_empty(){"New thread"}else{s(p,"title")},"env":if s(p,"env").is_empty(){"(setting)"}else{s(p,"env")},"branch":"","worktree":"","provider":"claude","model":"","at":at})).await.unwrap(); vec![json!({"t":"reply","id":id,"ok":true,"thread":t})] }
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
async fn up(name: &str) -> (Arc<Hub>, u16) { up_with(name, "").await }
/// `up` with more of the log after LOG's lines.
async fn up_with(name: &str, more: &str) -> (Arc<Hub>, u16) {
    let dir = std::env::temp_dir().join(format!("bp-data-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("token"), "tok").unwrap(); std::fs::write(dir.join("events.jsonl"), format!("{LOG}{more}")).unwrap();
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
    assert_eq!(crate::hub::capabilities()["data"]["proto"], "data/1"); assert_eq!(crate::hub::capabilities()["data"]["features"], json!(["images", "actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker", "voice", "order", "layers", "onboard", "push", "settled", "commands", "work", "star", "accounts", "machines", "history", "terms", "links", "devices", "queue", "buy", "status"]));
    let (_h, port) = up("hello").await;
    let mut ws = open(port, "&enc=json").await; let v = next(&mut ws).await;
    assert_eq!((v["t"].as_str(), v["proto"].as_str()), (Some("hello"), Some("data/1"))); assert_eq!(v["hub"], format!("127.0.0.1:{port}"), "the hub as asked (the Host header)"); assert_eq!(v["features"], json!(["images", "actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker", "voice", "order", "layers", "onboard", "push", "settled", "commands", "work", "star", "accounts", "machines", "history", "terms", "links", "devices", "queue", "buy", "status"]), "a hub from before images does not list it"); assert!(v["now"].as_u64().unwrap() > 1_600_000_000_000);
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
/// A phone's new thread is made as the desktop's +: its env follows the thread.env setting (a worktree in a git project), or the one it names.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn a_phones_new_thread_gets_a_worktree_like_the_desktops() {
    let (hub, port) = up("wtenv").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let dir = std::env::temp_dir().join(format!("bp-data-wtenv-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
    let pid = ask(&mut ws, 1, "project.add", json!({"path":dir.to_string_lossy()})).await["result"]["project"]["id"].as_str().unwrap().to_string();
    let env = |r: &Value| r["result"]["thread"]["id"].as_str().unwrap().to_string();
    let a = env(&ask(&mut ws, 2, "thread.create", json!({"project":pid})).await); let b = env(&ask(&mut ws, 3, "thread.create", json!({"project":pid,"env":"local"})).await);
    let m = hub.model().await; assert_eq!((m.threads[&a].env.as_str(), m.threads[&b].env.as_str()), ("(setting)", "local"), "no env of its own: the setting decides");
    let r = ask(&mut ws, 4, "thread.create", json!({"project":pid,"env":"elsewhere"})).await; assert_eq!(r["ok"], false, "{r}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_writes_make_a_project_a_thread_and_a_message_once() {
    let (hub, port) = up("writes").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let d = ask(&mut ws, 1, "dirs.list", json!({"path":"/"})).await; if cfg!(unix) { assert_eq!(d["result"]["path"], "/"); } assert!(d["result"]["parent"].is_null()); // Windows: "/" is the drive's root assert!(!d["result"]["dirs"].as_array().unwrap().is_empty());
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
    { use crate::plat::fs::PermissionsExt; assert_eq!(crate::plat::fs::mode_bits(files[0].metadata().unwrap().permissions().mode() & 0o777, 0o600), 0o600); }
    let log = (0..hub.store.len()).map(|i| hub.store.get(i).unwrap()).collect::<Vec<_>>();
    let posted: Value = serde_json::from_str(log.iter().rev().find(|l| l.contains("\"MessagePosted\"") && l.contains("img-1")).unwrap()).unwrap();
    let queued: Value = serde_json::from_str(log.iter().rev().find(|l| l.contains("\"TurnQueued\"") && l.contains("img-1")).unwrap()).unwrap();
    assert_eq!(posted["text"], "see this"); assert!(posted["attachments"].is_array());
    let told = queued["text"].as_str().unwrap();
    assert!(told.starts_with("see this\n\n[attachment: ") && told.contains(&dir.to_string_lossy().to_string()), "{queued}");
    // the agent gets the pictures themselves: its input reads the lines the phone's message ends with as image blocks, bytes and all
    let imgs = crate::agents::inputs::read_images(told);
    assert_eq!(imgs.iter().map(|i| i.0.as_str()).collect::<Vec<_>>(), ["image/png", "image/jpeg"], "{told}");
    assert_eq!((imgs[0].1.as_str(), imgs[1].1.as_str()), (b64(&png).as_str(), b64(b"jpegbytes").as_str()));
    assert_eq!(crate::agents::inputs::att_paths(told).len(), 2, "both lines are at the message's end, where the agent's input looks");
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

// a phone's push calls: a token reaches the log, and push.state's answer is its own fields, not the rpc's reply around them
struct PushState;
impl crate::api::RpcHandler for PushState {
    fn methods(&self) -> &'static [&'static str] { &["push.state"] }
    fn call<'a>(&'a self, _h: &'a Arc<Hub>, _w: &'a crate::api::Caller, id: &'a Value, _m: &'a str, _p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> { Box::pin(async move { vec![json!({"t":"reply","id":id,"ok":true,"result":{"keys":["k"],"badge":2,"quiet":"off","urgent":false}})] }) }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn a_phone_registers_its_token_and_reads_push_state_over_data() {
    let (h, port) = up("push").await; h.register_rpc(Arc::new(crate::net::devices::Devices)); h.register_rpc(Arc::new(PushState));
    let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let r = ask(&mut ws, 1, "device.register", json!({"token":"abc123","platform":"ios","kind":"alert","env":"sandbox","bundle":"dev.backplane.mobile"})).await; assert_eq!((r["ok"].as_bool(), &r["result"]), (Some(true), &json!({})), "{r}");
    assert!(h.store.range(0, usize::MAX).unwrap().iter().any(|l| l.contains("DeviceRegistered") && l.contains("abc123")));
    let r = ask(&mut ws, 2, "device.register", json!({"token":"a b"})).await; assert_eq!(r["ok"], false, "{r}");
    let r = ask(&mut ws, 3, "push.state", json!({})).await; assert_eq!(r["result"], json!({"keys":["k"],"badge":2,"quiet":"off","urgent":false}));
}

// the composer's own commands (feature commands): never a message, whatever the app
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn the_composers_commands_run_and_are_never_posted() {
    let (h, port) = up("cmds").await; let mut ws = open(port, "&enc=json").await;
    let hello = next(&mut ws).await; assert_eq!(hello["commands"][0]["word"], "/btw"); assert_eq!(hello["commands"][0]["label"], "Ask");
    let n = h.store.len();
    for text in ["/btw why", "/give >x", "/secret sk-abc123", "/intent # Goal"] { let r = ask(&mut ws, 1, "message.send", json!({"thread":"t1","text":text})).await; assert_eq!(r["ok"], false, "{text}"); assert!(r["error"].to_string().contains("Update it"), "{r}"); }
    assert_eq!(h.store.len(), n, "an old app's command reaches neither the log nor the thread");
    assert_eq!(ask(&mut ws, 2, "composer.run", json!({"thread":"t1","text":"/btw"})).await["error"], crate::session::composer::BTW_EMPTY);
    assert_eq!(ask(&mut ws, 3, "composer.run", json!({"thread":"t1","text":"hello"})).await["ok"], false);
    assert_eq!(ask(&mut ws, 4, "composer.run", json!({"thread":"zz","text":"/btw x"})).await["ok"], false);
    assert_eq!(ask(&mut ws, 5, "composer.run", json!({"thread":"t1","text":"/secret gh"})).await["result"], json!({"kind":"secret","name":"gh"}));
    assert_eq!(ask(&mut ws, 6, "composer.run", json!({"thread":"t1","text":"/btw why"})).await["result"], json!({"kind":"btw","q":"why"}));
    let a = loop { let v = next(&mut ws).await; if v["k"] == "btw" { break v } };
    assert_eq!((a["t"].as_str(), a["thread"].as_str(), a["q"].as_str()), (Some("d"), Some("t1"), Some("why"))); assert!(a["a"].as_str().unwrap().contains("Settings, Writing"), "no model here: it says where the model is chosen");
    assert_eq!(h.store.len(), n);
}

// Conversation history (feature history): the old Scratch chats, found by search and opened as they were; nothing starts there
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn scratch_chats_are_history_found_by_search_and_never_a_place_to_start() {
    let chat = |id: &str, title: &str, at: u64| format!("{{\"$\":\"ThreadCreated\",\"id\":\"{id}\",\"project\":\"scratch\",\"title\":\"{title}\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":{at}}}\n");
    let more = format!("{{\"$\":\"ProjectCreated\",\"id\":\"scratch\",\"title\":\"Scratch\",\"root\":\"/tmp/scratch\",\"at\":1700000002}}\n{}{}{{\"$\":\"MessagePosted\",\"thread\":\"s1\",\"msg\":\"m9\",\"role\":\"user\",\"text\":\"which crossing\",\"at\":1700000041}}\n",
        chat("s1", "zebra crossing chat", 1700000040), chat("s2", "old notes", 1700000050));
    let (_h, port) = up_with("history", &more).await; let mut ws = open(port, "&enc=json").await;
    let hello = next(&mut ws).await; assert!(hello["features"].as_array().unwrap().contains(&json!("history")));
    let ids = |v: &Value| v.as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let r = ask(&mut ws, 1, "projects.list", json!({})).await; assert!(!ids(&r["result"]["projects"]).contains(&"scratch".to_string()), "Scratch is no project to start a thread in: {r}");
    let r = ask(&mut ws, 2, "threads.list", json!({})).await; let t = ids(&r["result"]["threads"]); assert!(!t.contains(&"s1".into()) && !t.contains(&"s2".into()), "its chats are history, not a project's threads: {t:?}");
    assert!(r["result"]["threads"].as_array().unwrap().iter().all(|t| t["history"] == false));
    let r = ask(&mut ws, 3, "history.list", json!({})).await; let h = &r["result"];
    assert_eq!((ids(&h["threads"]), h["count"].as_u64(), ids(&h["live"]).len()), (vec!["s2".to_string(), "s1".into()], Some(2), 0), "newest first, every chat counted: {h}");
    assert!(h["threads"].as_array().unwrap().iter().all(|t| t["history"] == true && t["project"] == "scratch")); assert_eq!(h["threads"][1]["preview"], "which crossing");
    let r = ask(&mut ws, 4, "history.list", json!({"q":"ZEBRA cross"})).await; assert_eq!((ids(&r["result"]["threads"]), r["result"]["count"].as_u64()), (vec!["s1".to_string()], Some(2)), "every word of the search, any case; the count is the group's");
    assert!(ask(&mut ws, 5, "history.list", json!({"q":"nothing like it"})).await["result"]["threads"].as_array().unwrap().is_empty());
    let r = ask(&mut ws, 6, "history.list", json!({"limit":1})).await; assert_eq!(ids(&r["result"]["threads"]), ["s2"]); let c = r["result"]["next"].as_str().unwrap().to_string();
    let r = ask(&mut ws, 7, "history.list", json!({"limit":1,"before":c})).await; assert_eq!(ids(&r["result"]["threads"]), ["s1"]); assert!(r["result"]["next"].is_null());
    let r = ask(&mut ws, 8, "search", json!({"q":"zebra"})).await; assert_eq!(ids(&r["result"]["threads"]), ["s1"]); assert_eq!(r["result"]["threads"][0]["history"], true, "a found chat says it is history");
    let r = ask(&mut ws, 9, "search", json!({"q":"scratch"})).await; assert!(ids(&r["result"]["threads"]).is_empty() && ids(&r["result"]["projects"]).is_empty(), "Scratch's name is no project's: {r}");
    let r = ask(&mut ws, 10, "mentions.list", json!({"q":"scr","kind":"%"})).await; assert!(r["result"]["items"].as_array().unwrap().is_empty());
    // it opens as it was, read and write
    let r = ask(&mut ws, 11, "thread.entries", json!({"thread":"s1"})).await; assert_eq!(r["result"]["thread"]["history"], true); assert_eq!(r["result"]["entries"][0]["text"], "which crossing");
    let n = _h.store.len();
    let r = ask(&mut ws, 12, "thread.create", json!({"project":"scratch","text":"hi"})).await; assert_eq!(r["error"], super::SCRATCH_CLOSED);
    assert_eq!(_h.store.len(), n, "nothing was made");
}

// the phone additions (data/ext.rs)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_additions_search_models_mentions_directory_and_actions() {
    let (_h, port) = up("ext").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let r = ask(&mut ws, 1, "search", json!({"q":"SECOND"})).await; assert_eq!(r["result"]["threads"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["t2"], "case-insensitive, by title");
    let r = ask(&mut ws, 2, "search", json!({"q":"alpha"})).await; assert_eq!(r["result"]["projects"][0]["id"], "p1"); assert_eq!(r["result"]["threads"].as_array().unwrap().len(), 2, "a project's name finds its threads");
    assert!(ask(&mut ws, 3, "search", json!({"q":""})).await["result"]["threads"].as_array().unwrap().is_empty());
    // what was said: a word of a message finds it, in any thread or in the one named; `messages: false` leaves them out
    let r = ask(&mut ws, 30, "search", json!({"q":"HOW CAN"})).await; let m = &r["result"]["messages"];
    assert_eq!((m[0]["thread"].as_str(), m[0]["msg"].as_str(), m[0]["role"].as_str(), m[0]["title"].as_str()), (Some("t1"), Some("m2"), Some("assistant"), Some("First")), "{r}");
    assert!(ask(&mut ws, 31, "search", json!({"q":"hello","thread":"t2"})).await["result"]["messages"].as_array().unwrap().is_empty(), "another thread's search");
    assert!(ask(&mut ws, 32, "search", json!({"q":"hello","messages":false})).await["result"]["messages"].as_array().unwrap().is_empty());
    let r = ask(&mut ws, 4, "models.list", json!({})).await; assert!(r["result"]["models"]["claude"].as_array().map_or(false, |a| !a.is_empty()), "claude's models are built in");
    let r = ask(&mut ws, 5, "mentions.list", json!({"q":"fir","kind":">"})).await; assert_eq!(r["result"]["items"][0]["label"], "First"); assert_eq!(r["result"]["items"][0]["sigil"], ">");
    let r = ask(&mut ws, 6, "mentions.list", json!({"q":"alp","kind":"%"})).await; assert_eq!(r["result"]["items"][0]["id"], "p1");
    assert_eq!((r["result"]["items"][0]["word"].as_str(), ask(&mut ws, 61, "mentions.list", json!({"q":"sec","kind":">"})).await["result"]["items"][0]["word"].as_str()), (Some("%alpha"), Some(">second")), "the word the desktop's popup puts in the draft, which /give reads");
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
#[test] fn notify_says_what_the_gate_sent() {
    let n = ext::notify_of(&json!({"$":"PushSent","kind":"needs_you","thread":"t1","ask":"a1","key":"push-1","title":"Bub","body":"Approve?","badge":2,"through":false,"sent":1,"at":9}));
    assert_eq!(n, json!({"kind":"needs_you","thread":"t1","ask":"a1","key":"push-1","title":"Bub","body":"Approve?","badge":2,"through":false,"link":""}));
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
    for f in ["fold", "picker", "work"] { assert!(hello["features"].as_array().unwrap().iter().any(|x| x == f), "{f}"); }
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
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_the_hubs_note_to_the_agent_is_marked_agent_only_on_a_page_and_live() {
    let (hub, port) = up("agentonly").await;
    crate::settings::order::test_link("q_live", "http://127.0.0.1:3871/q/q_live?t=tok");
    let note = "[Backplane] The quote is posted in this thread. {\"status\":\"ready\",\"total\":27.2}";
    hub.commit(&json!({"$":"MessagePosted","thread":"t2","msg":"n1","role":"system","text":note,"at":1700000041})).await.unwrap();
    hub.commit(&json!({"$":"MessagePosted","thread":"t2","msg":"n2","role":"system","text":"[Backplane] Watched work \"build\" ended: exit 0","at":1700000042})).await.unwrap();
    // a secret saved (/secret): the agent is told its name; the person posted nothing, so no client shows it as theirs
    let told = format!("{}{}", crate::settings::secret::handle("gh", true), crate::present::thread::SAVED_NOTE);
    hub.commit(&json!({"$":"MessagePosted","thread":"t2","msg":"n4","role":"user","text":told,"at":1700000042})).await.unwrap();
    hub.commit(&json!({"$":"MessagePosted","thread":"t2","msg":"n5","role":"user","text":"{\"secret\":\"gh\"} is what I typed","at":1700000042})).await.unwrap();
    let mut ws = open(port, "&enc=json").await; let _ = next(&mut ws).await;
    let r = ask(&mut ws, 1, "thread.entries", json!({"thread":"t2"})).await; let es = r["result"]["entries"].as_array().unwrap().clone();
    let by = |id: &str| es.iter().find(|e| e["id"] == id).cloned().unwrap_or_else(|| panic!("{id}: {r}"));
    assert_eq!(by("n1")["agentOnly"], true, "the quote note is the agent's"); assert!(by("n2").get("agentOnly").is_none(), "other notes are the person's too"); assert_eq!(by("n4")["agentOnly"], true, "the saved secret's note is the agent's"); assert!(by("n5").get("agentOnly").is_none(), "the person's own words stay");
    // live: a quote and its note posted while the thread is open
    let sub = ask(&mut ws, 2, "subscribe", json!({"topic":"thread","thread":"t2"})).await["result"]["sub"].as_u64().unwrap();
    hub.commit(&json!({"$":"MessagePosted","thread":"t2","msg":"q:q_live","role":"assistant","text":"**Your quote: $27.20**\n\n[Buy now · $27.20](order:q_live)","at":1700000043})).await.unwrap();
    hub.commit(&json!({"$":"MessagePosted","thread":"t2","msg":"n3","role":"system","text":note,"at":1700000044})).await.unwrap();
    let (mut quote, mut live_note) = (Value::Null, Value::Null);
    for _ in 0..20 { let v = next(&mut ws).await; if v["k"] == "entry" && v["sub"] == sub { match v["entry"]["id"].as_str() { Some("q:q_live") => quote = v["entry"].clone(), Some("n3") => live_note = v["entry"].clone(), _ => {} } } if !quote.is_null() && !live_note.is_null() { break } }
    assert!(quote["text"].as_str().unwrap_or("").contains("](http://127.0.0.1:3871/q/q_live?t=tok)"), "a live quote has the link to open too: {quote}");
    assert_eq!(live_note["agentOnly"], true, "{live_note}");
}
#[test] fn data_a_messages_attachment_lines_are_not_shown() {
    assert_eq!(shown_text("hi\n\n[attachment: /a/x.png]\n[attachment: /a/y.jpg]"), "hi");
    assert_eq!(shown_text("hi\n\n[Attached image: /a/x.png]"), "hi", "a message from before the change still hides its paths");
    assert_eq!(shown_text("[attachment: /a/x.png]"), "", "a picture alone has no words");
    assert_eq!(shown_text("see [attachment: x] inline"), "see [attachment: x] inline", "only whole lines are attachment lines");
}

/// Settings > Updates on a phone: the status itself, the words included, not the reply wrapped around it (the Pixel showed "Backplane" and nothing else).
#[tokio::test] async fn update_status_is_the_status_itself() {
    let (h, port) = up("update").await; crate::update::svc::install(&h); tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let mut ws = open(port, "&enc=json").await; let _ = next(&mut ws).await;
    let r = ask(&mut ws, 1, "update.status", json!({})).await; let u = &r["result"];
    assert!(u["current"].as_str().map_or(false, |c| !c.is_empty()), "{r}"); assert!(u["line"].as_str().map_or(false, |l| !l.is_empty()) && u["canCheck"].is_boolean(), "{r}");
    assert!(u.get("t").is_none() && u.get("result").is_none(), "not a reply inside a reply: {r}");
}

#[test] fn data_settled_is_the_desktops_shelf_with_the_time_it_was_settled() {
    use crate::state::{Override, Thread};
    let mk = |o: Override, set: u64, unset: u64, pinned: bool, archived: bool| { let mut t = Thread::blank(); t.deleted = false; t.archived = archived; t.override_ = o; t.settled_at = set; t.unsettled_at = unset; t.pinned = pinned; t };
    // settled by the clock or the person: on the shelf, with its time in ms (event times are seconds or ms)
    assert_eq!(shelf_flags(&mk(Override::None, 1_700_000_000, 0, false, false)), (false, true, 1_700_000_000_000));
    assert_eq!(shelf_flags(&mk(Override::ForcedSettled, 1_700_000_000_000, 0, false, false)), (false, true, 1_700_000_000_000));
    // woken by a message or unsettled by hand: off the shelf, no time
    assert_eq!(shelf_flags(&mk(Override::None, 10, 20, false, false)), (false, false, 0));
    assert_eq!(shelf_flags(&mk(Override::ForcedActive, 30, 20, false, false)), (false, false, 0), "an unsettle by hand outlasts the old settle time");
    // pinned stays pinned (not settled) unless settled by hand; archived is on no shelf the phone lists
    assert_eq!(shelf_flags(&mk(Override::None, 30, 20, true, false)).1, false);
    assert_eq!(shelf_flags(&mk(Override::ForcedSettled, 30, 20, true, false)).1, true);
    assert_eq!(shelf_flags(&mk(Override::ForcedSettled, 30, 20, false, true)).1, false);
}

struct FakeStar(std::sync::Mutex<Vec<Value>>);
impl RpcHandler for FakeStar {
    fn methods(&self) -> &'static [&'static str] { &["star.offer", "star.nudge"] }
    fn call<'a>(&'a self, _h: &'a Arc<Hub>, _w: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if m == "star.offer" { return vec![json!({"t":"reply","id":id,"ok":true,"result":{"offer":{"token":"m5","text":"Like Backplane? Give us a star on GitHub!","url":"https://github.com/BackplaneWorks/Backplane","label":"Star on GitHub"}}})] }
            self.0.lock().unwrap().push(p.clone()); vec![json!({"t":"reply","id":id,"ok":true})]
        })
    }
}
#[tokio::test] async fn a_phone_reads_the_star_offer_decides_it_and_hears_it_hidden() {
    // feature "star": star.offer on (re)connect, star.nudge passes token and action through, and the hub's star frames (shown / hidden) reach the phone
    let (hub, port) = up("star").await; let seen = Arc::new(FakeStar(Default::default())); hub.register_rpc(seen.clone());
    let mut ws = open(port, "&enc=json").await; let hello = next(&mut ws).await;
    assert!(hello["features"].as_array().unwrap().contains(&json!("star")));
    let r = ask(&mut ws, 1, "star.offer", json!({})).await; assert_eq!(r["result"]["offer"]["token"], "m5", "{r}"); assert_eq!(r["result"]["offer"]["text"], "Like Backplane? Give us a star on GitHub!");
    let r = ask(&mut ws, 2, "star.nudge", json!({"token":"m5","action":"dismiss"})).await; assert_eq!(r["ok"], true, "{r}");
    assert_eq!(*seen.0.lock().unwrap(), vec![json!({"token":"m5","action":"dismiss"})]);
    hub.push_all(&json!({"t":"star","offer":null}));
    loop { let v = next(&mut ws).await; if v["t"] == "star" { assert_eq!(v["offer"], Value::Null); break } }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_link_resolve_says_what_a_link_opens_or_why_not() {
    let root = std::env::temp_dir().join(format!("bp-links-{}", std::process::id())); let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap(); std::fs::write(root.join("src/x.rs"), "one\ntwo\n").unwrap();
    let more = format!("{{\"$\":\"ProjectCreated\",\"id\":\"p9\",\"title\":\"links\",\"root\":{},\"at\":1700000100}}\n{{\"$\":\"ThreadCreated\",\"id\":\"t9\",\"project\":\"p9\",\"title\":\"Nine\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1700000101}}\n", serde_json::Value::from(root.to_string_lossy()));   // JSON-quoted: a Windows path has backslashes
    let (_h, port) = up_with("links", &more).await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let mut n = 0u64;
    let mut r = |u: &str, t: &str| { n += 1; json!({"id":n,"m":"link.resolve","p":{"url":u,"thread":t}}) };
    let cases: Vec<(Value, Value)> = vec![
        (r("backplane://thread/t1", ""), json!({"kind":"thread","thread":"t1"})),
        (r("backplane://thread/t404", ""), json!({"kind":"none"})),
        (r("backplane://thread/L~t9", ""), json!({"kind":"none"})),
        (r("backplane://project/p1", ""), json!({"kind":"project","project":"p1"})),
        (r("backplane://project/p404", ""), json!({"kind":"none"})),
        (r("backplane://conv/room%3Anone", ""), json!({"kind":"none"})),
        (r("backplane://person", ""), json!({"kind":"none"})),
        (r("src/x.rs:2", "t9"), json!({"kind":"file","thread":"t9","path":"src/x.rs","line":2})),
        (r("src/y.rs:2", "t9"), json!({"kind":"none"})),
        (r("../etc/passwd", "t9"), json!({"kind":"none"})),
        (r("src/x.rs", ""), json!({"kind":"none"})),
    ];
    for (m, want) in cases {
        let id = m["id"].as_u64().unwrap(); ws.send(Message::Text(m.to_string().into())).await.unwrap();
        let v = loop { let v = next(&mut ws).await; if v["t"] == "reply" && v["id"] == id { break v } };
        let got = &v["result"]; assert_eq!(got["kind"], want["kind"], "{m}: {v}");
        for k in ["thread", "project", "path", "line"] { if want.get(k).is_some() { assert_eq!(got[k], want[k], "{m}"); } }
        if want["kind"] == "none" { assert!(!got["note"].as_str().unwrap_or("").is_empty(), "a refusal says why: {m}") }
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// An upload over the cap: the client sends its whole body before it reads (as most do), and still gets the 413 and why, not a reset.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn an_oversized_upload_gets_its_413_after_sending_everything() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (hub, port) = up("cap").await; crate::files::imgroutes::start(&hub);
    hub.commit(&json!({"$":"SettingSet","key":"files.max.mb","value":"1","at":1700000500})).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let n = 24usize << 20; // 24 MB over a 1 MB cap
    let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    s.write_all(format!("POST /attach?thread=t1&key=k9&name=big.bin&token=tok HTTP/1.1\r\nHost: x\r\nContent-Type: application/octet-stream\r\nContent-Length: {n}\r\n\r\n").as_bytes()).await.unwrap();
    let chunk = vec![7u8; 1 << 20]; let mut sent = 0usize;
    while sent < n { if s.write_all(&chunk).await.is_err() { break } sent += chunk.len(); }
    let mut out = Vec::new(); let read = s.read_to_end(&mut out).await;
    let text = String::from_utf8_lossy(&out);
    assert!(read.is_ok(), "the answer was read, not reset ({sent} of {n} bytes sent): {read:?}");
    assert!(text.starts_with("HTTP/1.1 413"), "{text}"); assert!(text.contains("upload.cap"), "it says why: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_the_queue_and_buy_it_answer_for_a_thread() {
    let (_hub, port) = up("queuebuy").await;
    let mut ws = open(port, "&enc=json").await; let hello = next(&mut ws).await;
    for f in ["queue", "buy", "status"] { assert!(hello["features"].as_array().unwrap().iter().any(|x| x == f), "{f}") }
    let r = ask(&mut ws, 1, "thread.queue", json!({"thread":"t1"})).await; assert_eq!(r["result"]["rows"], json!([]), "nothing waits: {r}");
    let r = ask(&mut ws, 2, "thread.queue", json!({})).await; assert_eq!(r["ok"], false, "a thread is needed: {r}");
    let r = ask(&mut ws, 3, "queue.act", json!({"thread":"t1","action":"q-fly","value":"m1"})).await; assert!(r["error"].as_str().unwrap_or("").starts_with("action:"), "{r}");
    let r = ask(&mut ws, 4, "queue.act", json!({"thread":"t1","action":"q-up","value":"m1"})).await; assert_eq!(r["ok"], false, "q-up needs a place: {r}");
    let r = ask(&mut ws, 5, "thread.buy", json!({"thread":"t1"})).await; assert_eq!(r["result"]["buy"], Value::Null, "no KiCad project, nothing to buy: {r}");
    let r = ask(&mut ws, 6, "order.buy", json!({})).await; assert_eq!(r["ok"], false, "{r}");
    let r = ask(&mut ws, 7, "threads.list", json!({"project":"p1"})).await;
    assert!(r["result"]["threads"].as_array().unwrap().iter().all(|t| t["status"].is_string()), "every thread says its dot's word: {r}");
}

//! The phone data API over a real socket (JSON frames, and CBOR for the default), with fake project/thread handlers that commit what the real ones commit.
use super::*;
use crate::api::{Caller, Fut, RpcHandler};
use crate::hub::Cfg;
use crate::presenter::Canned;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;

struct Fake(Arc<Hub>);
impl RpcHandler for Fake {
    fn methods(&self) -> &'static [&'static str] { &["project.add", "project.new", "thread.create", "thread.modes", "thread.effort", "thread.provider", "ask.answer", "turn.interrupt", "thread.pin", "thread.rename", "thread.detach", "setting.set", "thread.review"] }
    fn call<'a>(&'a self, h: &'a Arc<Hub>, _w: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let _ = &self.0; let at = now_ms() / 1000;
            let n = h.store.len();
            match m {
                "project.add" => { let root = s(p, "path"); h.commit(&json!({"$":"ProjectCreated","id":format!("p{n}"),"title":root.rsplit('/').next().unwrap_or(""),"root":root,"at":at})).await.unwrap(); vec![json!({"t":"reply","id":id,"ok":true})] }
                "thread.create" => { let t = format!("t{n}"); h.commit(&json!({"$":"ThreadCreated","id":t,"project":s(p,"project"),"title":if s(p,"title").is_empty(){"New thread"}else{s(p,"title")},"env":if s(p,"env").is_empty(){"(setting)"}else{s(p,"env")},"branch":"","worktree":"","provider":"claude","model":"","at":at})).await.unwrap(); vec![json!({"t":"reply","id":id,"ok":true,"thread":t})] }
                // /project's request: a folder named "full" holds something, any other is made with its first thread
                "project.new" => if s(p, "path") == "full" { vec![json!({"t":"reply","id":id,"ok":false,"error":"/h/full is there and is not empty.","result":{"error":"/h/full is there and is not empty.","exists":"/h/full"}})] }
                    else { assert_eq!(p["empty"], true); vec![json!({"t":"reply","id":id,"ok":true,"thread":format!("made-{}", s(p, "text").len())})] },
                "thread.review" => { h.commit(&json!({"$":"SettingSet","key":format!("review.ro.{}", s(p, "thread")),"value":if p["on"] == true { "1" } else { "0" },"at":at})).await.unwrap(); vec![json!({"t":"reply","id":id,"ok":true})] }
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
async fn up(name: &str) -> (crate::hub::TestHub, u16) { up_with(name, "").await }
/// `up` with more of the log after LOG's lines.
async fn up_with(name: &str, more: &str) -> (crate::hub::TestHub, u16) {
    let dir = crate::plat::tmp::dir(&format!("data-{name}"));
    std::fs::write(dir.join("token"), "tok").unwrap(); std::fs::write(dir.join("events.jsonl"), format!("{LOG}{more}")).unwrap();
    let cfg = Cfg { home: dir.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
    let hub = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    hub.register_rpc(Arc::new(Fake(hub.clone())));
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    { let hub = hub.clone(); tokio::spawn(async move { loop { let (st, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = wire::serve_with(h, st, false).await; }); } }); }
    (crate::hub::TestHub { hub, _tmp: dir }, port)
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
    assert_eq!(crate::hub::capabilities()["data"]["proto"], "data/1"); assert_eq!(crate::hub::capabilities()["data"]["features"], json!(["images", "actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker", "voice", "order", "layers", "onboard", "push", "settled", "commands", "work", "star", "accounts", "machines", "history", "terms", "links", "devices", "queue", "buy", "status", "subview", "find", "notifysettings", "ring", "hubOffer", "swarm", "sections", "telemetryNotice", "mcpsettings", "fabprofile", "plan", "mgmt", "replyImages", "cats", "projectcard", "termChat", "copy", "reviewmode", "rail", "gerbers", "follow", "unanswered", "browserstart"]));
    let (_h, port) = up("hello").await;
    let mut ws = open(port, "&enc=json").await; let v = next(&mut ws).await;
    assert_eq!((v["t"].as_str(), v["proto"].as_str()), (Some("hello"), Some("data/1"))); assert_eq!(v["hub"], format!("127.0.0.1:{port}"), "the hub as asked (the Host header)"); assert!(v["name"].is_string(), "the machine's name: {v}"); assert_eq!(v["features"], json!(["images", "actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker", "voice", "order", "layers", "onboard", "push", "settled", "commands", "work", "star", "accounts", "machines", "history", "terms", "links", "devices", "queue", "buy", "status", "subview", "find", "notifysettings", "ring", "hubOffer", "swarm", "sections", "telemetryNotice", "mcpsettings", "fabprofile", "plan", "mgmt", "replyImages", "cats", "projectcard", "termChat", "copy", "reviewmode", "rail", "gerbers", "follow", "unanswered", "browserstart"]), "a hub from before images does not list it"); assert!(v["now"].as_u64().unwrap() > 1_600_000_000_000);
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
    // (t3 was archived: an archive is a settle now, state/mod.rs, so it is listed like any settled thread, newest first)
    let r = ask(&mut ws, 2, "threads.list", json!({"limit":1})).await; let ts = &r["result"]["threads"]; assert_eq!(ts[0]["id"], "t3"); let next_c = r["result"]["next"].as_str().unwrap().to_string();
    let r = ask(&mut ws, 17, "threads.list", json!({"limit":1,"before":next_c})).await; assert_eq!(r["result"]["threads"][0]["id"], "t2"); let next_c = r["result"]["next"].as_str().unwrap().to_string();
    let r = ask(&mut ws, 3, "threads.list", json!({"limit":1,"before":next_c})).await; let t = &r["result"]["threads"][0]; assert_eq!(t["id"], "t1"); assert!(r["result"]["next"].is_null());
    assert_eq!((t["state"].as_str(), t["waiting"].as_u64(), t["preview"].as_str()), (Some("asking"), Some(1), Some("hi, how can I help with this long reply that goes on")), "an open ask makes the thread asking; the preview is the last thing said");
    let r = ask(&mut ws, 4, "threads.list", json!({"archived":true})).await; assert!(r["result"]["threads"].as_array().unwrap().is_empty(), "nothing is archived any more");
    let r = ask(&mut ws, 5, "threads.list", json!({"project":"p2"})).await; assert_eq!(r["result"]["threads"][0]["id"], "t3", "t3 is listed on its project, not lost");
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
    let dir = crate::plat::tmp::dir("data-wtenv");
    let pid = ask(&mut ws, 1, "project.add", json!({"path":dir.to_string_lossy()})).await["result"]["project"]["id"].as_str().unwrap().to_string();
    let env = |r: &Value| r["result"]["thread"]["id"].as_str().unwrap().to_string();
    let a = env(&ask(&mut ws, 2, "thread.create", json!({"project":pid})).await); let b = env(&ask(&mut ws, 3, "thread.create", json!({"project":pid,"env":"local"})).await);
    let m = hub.model().await; assert_eq!((m.threads[&a].env.as_str(), m.threads[&b].env.as_str()), ("(setting)", "local"), "no env of its own: the setting decides");
    let r = ask(&mut ws, 4, "thread.create", json!({"project":pid,"env":"elsewhere"})).await; assert_eq!(r["ok"], false, "{r}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_writes_make_a_project_a_thread_and_a_message_once() {
    let (hub, port) = up("writes").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let d = ask(&mut ws, 1, "dirs.list", json!({"path":"/"})).await; if cfg!(unix) { assert_eq!(d["result"]["path"], "/"); } assert!(d["result"]["parent"].is_null()); // Windows: "/" is the drive's root assert!(!d["result"]["dirs"].as_array().unwrap().is_empty());
    let dir = crate::plat::tmp::dir("data-proj");
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
    assert!(hello["commands"].as_array().unwrap().iter().any(|c| c["word"] == "/project"), "{hello}");
    assert_eq!(ask(&mut ws, 7, "composer.run", json!({"thread":"t1","text":"/project"})).await["result"], json!({"kind":"project"}), "alone: the app's Add project");
    assert_eq!(ask(&mut ws, 8, "composer.run", json!({"thread":"t1","text":"/project puck Build it"})).await["result"], json!({"kind":"made","thread":"made-8","note":""}));
    assert_eq!(ask(&mut ws, 9, "composer.run", json!({"thread":"t1","text":"/new-project full","exists":true})).await["result"], json!({"kind":"exists","path":"/h/full","error":"/h/full is there and is not empty."}));
    let r = ask(&mut ws, 10, "composer.run", json!({"thread":"t1","text":"/project full"})).await; assert_eq!((r["ok"].as_bool(), r["error"].as_str()), (Some(false), Some("/h/full is there and is not empty.")), "an app that offers no Add existing gets the words");
    assert_eq!(ask(&mut ws, 11, "composer.run", json!({"thread":"t1","text":"/project a?b"})).await["ok"], false);
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
    // find in a thread: every message with every word, oldest first, `at` in ms; a line appended since is found at once (the index reads only it)
    let r = ask(&mut ws, 33, "find", json!({"thread":"t1","q":"HE"})).await; let m = &r["result"]["matches"];
    assert_eq!((m[0]["msg"].as_str(), m[1]["msg"].as_str(), r["result"]["total"].as_u64()), (Some("m1"), Some("m2"), Some(2)), "{r}"); assert_eq!(m[0]["at"], 1700000011000u64);
    _h.store.append(r#"{"$":"MessagePosted","thread":"t1","msg":"m9","role":"user","text":"and the pinecone?","at":1700000040}"#).unwrap();
    let r = ask(&mut ws, 34, "find", json!({"thread":"t1","q":"pinecone"})).await; assert_eq!(r["result"]["matches"][0]["msg"], "m9", "{r}");
    assert!(ask(&mut ws, 35, "find", json!({"thread":"t2","q":"pinecone"})).await["result"]["matches"].as_array().unwrap().is_empty(), "another thread's");
    assert!(ask(&mut ws, 36, "find", json!({"thread":"nope","q":"pinecone"})).await["error"].is_string(), "an unknown thread");
    let r = ask(&mut ws, 4, "models.list", json!({})).await; assert!(r["result"]["models"]["claude"].as_array().map_or(false, |a| !a.is_empty()), "claude's models are built in");
    let r = ask(&mut ws, 5, "mentions.list", json!({"q":"fir","kind":">"})).await; assert_eq!(r["result"]["items"][0]["label"], "First"); assert_eq!(r["result"]["items"][0]["sigil"], ">");
    let r = ask(&mut ws, 6, "mentions.list", json!({"q":"alp","kind":"%"})).await; assert_eq!(r["result"]["items"][0]["id"], "p1");
    assert_eq!((r["result"]["items"][0]["word"].as_str(), ask(&mut ws, 61, "mentions.list", json!({"q":"sec","kind":">"})).await["result"]["items"][0]["word"].as_str()), (Some("%alpha"), Some(">second")), "the word the desktop's popup puts in the draft, which /give reads");
    let r = ask(&mut ws, 7, "bots.list", json!({})).await; assert_eq!((r["result"]["bots"].as_array().unwrap().len(), r["result"]["rooms"].as_array().unwrap().len()), (0, 0), "no bots lane on this hub: empty, not an error");
    // feature cats: a name's cat in a colour, as the desktop draws it; no uberbot here to name
    let r = ask(&mut ws, 71, "cat.rig", json!({"name":"Bub","color":"#6fb3f2","mood":"idle"})).await;
    assert_eq!((r["result"]["look"].as_u64(), r["result"]["color"].as_str()), (Some(crate::bots::cat::look_of("Bub")), Some("#6fb3f2")));
    assert!(r["result"]["parts"].as_array().unwrap().iter().any(|p| p["name"] == "body" && p["fill"] == 0x6fb3f2)); assert_eq!(r["result"]["presets"].as_array().unwrap().len(), 10);
    assert_eq!(ask(&mut ws, 72, "bot.cat", json!({"color":"#000000"})).await["ok"], false, "no uberbot on this hub: nothing to colour");
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
// lane 2: a subagent's own timeline, opened from its Agent call (`tool.sub`), read whole and pushed as it moves
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_a_subagents_view_is_read_from_its_call_and_pushed_as_it_moves() {
    let (hub, port) = up("subview").await;
    let mut ws = open(port, "&enc=json").await; let hello = next(&mut ws).await; assert!(hello["features"].as_array().unwrap().iter().any(|x| x == "subview"));
    for e in [json!({"$":"ActivityLogged","thread":"t1","act":"c9","tone":"tool","kind":"Agent","summary":"Survey","at":1700000040}),
        json!({"$":"ActivityLogged","thread":"@sub:t1","act":"c9/about","tone":"c9","kind":"about","summary":"{\"type\":\"Explore\",\"model\":\"\",\"prompt\":\"Find the boards.\",\"description\":\"Survey\"}","at":1700000040}),
        json!({"$":"ActivityLogged","thread":"@sub:t1","act":"c9/s1","tone":"c9","kind":"Bash","summary":"ls","at":1700000041}),
        json!({"$":"ActivityLogged","thread":"@sub:t1","act":"c9/s1/result","tone":"c9","kind":"result","summary":"{\"use\":\"s1\",\"text\":\"main.kicad_pcb\",\"error\":false}","at":1700000042})] { hub.commit(&e).await.unwrap(); }
    let r = ask(&mut ws, 1, "thread.entries", json!({"thread":"t1"})).await;
    let call = r["result"]["entries"].as_array().unwrap().iter().find(|e| e["id"] == "c9").unwrap().clone(); assert_eq!(call["tool"]["sub"], "c9", "the Agent call names its subagent");
    let r = ask(&mut ws, 2, "thread.sub", json!({"thread":"t1","key":"c9"})).await; let v = &r["result"]["view"];
    assert_eq!((v["title"].as_str(), v["ask"].as_str(), v["key"].as_str()), (Some("Survey"), Some("Find the boards."), Some("c9")));
    let kinds: Vec<&str> = v["entries"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect(); assert_eq!(kinds, ["user", "tool"]);
    assert_eq!((v["entries"][1]["tool"]["name"].as_str(), v["entries"][1]["tool"]["result"].as_str()), (Some("Bash"), Some("main.kicad_pcb")));
    assert!(ask(&mut ws, 3, "thread.sub", json!({"thread":"t1","key":"nope"})).await["result"]["view"].is_null());
    assert_eq!(ask(&mut ws, 4, "thread.sub", json!({"thread":"t1"})).await["ok"], false);
    let sub = ask(&mut ws, 5, "subscribe", json!({"topic":"sub","thread":"t1","key":"c9"})).await; assert_eq!(sub["ok"], true); let n = sub["result"]["sub"].as_u64().unwrap();
    assert_eq!(ask(&mut ws, 6, "subscribe", json!({"topic":"sub","thread":"nope","key":"c9"})).await["ok"], false);
    hub.commit(&json!({"$":"ActivityLogged","thread":"@sub:t2","act":"c9/x","tone":"c9","kind":"say","summary":"another thread's","at":1700000043})).await.unwrap();
    hub.commit(&json!({"$":"ActivityLogged","thread":"@sub:t1","act":"c9/m1:0","tone":"c9","kind":"say","summary":"One board.","at":1700000044})).await.unwrap();
    let v = loop { let v = next(&mut ws).await; if v["k"] == "sub" { break v } };
    assert_eq!((v["sub"].as_u64(), v["thread"].as_str(), v["key"].as_str()), (Some(n), Some("t1"), Some("c9")));
    let es = v["view"]["entries"].as_array().unwrap(); assert_eq!((es.len(), es[2]["kind"].as_str(), es[2]["text"].as_str()), (3, Some("assistant"), Some("One board.")));
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
/// #1305: a phone reads a thread's review bar and presses its button through `thread.review`, as the desktop's bar does.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_the_review_bar_reads_and_switches_review_mode() {
    let (hub, port) = up_with("review", "{\"$\":\"SettingSet\",\"key\":\"review.ro.t1\",\"value\":\"1\",\"at\":1700000050}\n").await; let _ = &hub;
    let mut ws = open(port, "&enc=json").await; let hello = next(&mut ws).await;
    assert!(hello["features"].as_array().unwrap().iter().any(|x| x == "reviewmode"));
    let r = ask(&mut ws, 1, "thread.review", json!({"thread":"t1"})).await;
    assert_eq!((r["result"]["review"]["on"].as_bool(), r["result"]["review"]["button"].as_str(), r["result"]["review"]["value"].as_bool()), (Some(true), Some("Allow edits"), Some(false)), "{r}");
    assert!(r["result"]["review"]["text"].as_str().unwrap().starts_with("Read-only: this request looked like a review"));
    let r = ask(&mut ws, 2, "thread.review", json!({"thread":"t1","on":false})).await;
    assert_eq!((r["result"]["review"]["on"].as_bool(), r["result"]["review"]["button"].as_str(), r["result"]["review"]["value"].as_bool()), (Some(false), Some("Read-only again"), Some(true)), "the press, then the bar as it is now: {r}");
    let r = ask(&mut ws, 3, "thread.review", json!({"thread":"t1","on":true})).await; assert_eq!(r["result"]["review"]["button"], "Allow edits", "{r}");
    let r = ask(&mut ws, 4, "thread.review", json!({"thread":"t2"})).await; assert_eq!(r["result"]["review"], Value::Null, "no review: no bar: {r}");
    let r = ask(&mut ws, 5, "thread.review", json!({})).await; assert!(r["error"].is_string() || r["result"].is_null(), "a thread is needed: {r}");
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
// docs/PHONE-DATA.md, "Handoffs": the agent reads the handoff message, the phone gets its card; the summary the popup rows carry is the model's
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_a_handoff_is_a_card_on_a_page_and_live_and_its_message_is_the_agents() {
    let (hub, port) = up("handoff").await;
    let card = crate::commands::handoff::card("t1", "First", "check DRC", "Routing the rail.", &[json!({"kind":"thread","label":"First","url":"backplane://thread/t1","thread":"t1"}), json!({"kind":"board","label":"a.kicad_pcb","path":"a.kicad_pcb","url":"a.kicad_pcb","thread":"t1"})]);
    let text = crate::commands::handoff::agent_text(&card);
    hub.commit_many(&[json!({"$":"MessagePosted","thread":"t2","msg":"h1","role":"user","text":text,"at":1700000041}), crate::commands::handoff::card_event("t2", "k1", 1700000041, &card)]).await.unwrap();
    let mut ws = open(port, "&enc=json").await; let _ = next(&mut ws).await;
    let r = ask(&mut ws, 1, "thread.entries", json!({"thread":"t2"})).await; let es = r["result"]["entries"].as_array().unwrap().clone();
    assert_eq!(es.iter().find(|e| e["id"] == "h1").unwrap()["agentOnly"], true, "the handoff message is the agent's");
    let c = es.iter().find(|e| e["kind"] == "handoff").unwrap_or_else(|| panic!("{r}"));
    assert_eq!((c["id"].as_str(), c["text"].as_str(), c["handoff"]["from"].as_str(), c["handoff"]["title"].as_str(), c["handoff"]["note"].as_str()), (Some("k1"), Some("Routing the rail."), Some("t1"), Some("First"), Some("check DRC")), "{c}");
    assert_eq!((c["handoff"]["refs"][0]["url"].as_str(), c["handoff"]["refs"][1]["kind"].as_str(), c["handoff"]["refs"][1]["thread"].as_str()), (Some("backplane://thread/t1"), Some("board"), Some("t1")));
    // live
    let sub = ask(&mut ws, 2, "subscribe", json!({"topic":"thread","thread":"t2"})).await["result"]["sub"].as_u64().unwrap();
    hub.commit(&crate::commands::handoff::card_event("t2", "k2", 1700000050, &card)).await.unwrap();
    let mut live = Value::Null; for _ in 0..20 { let v = next(&mut ws).await; if v["k"] == "entry" && v["sub"] == sub && v["entry"]["id"] == "k2" { live = v["entry"].clone(); break } }
    assert_eq!((live["kind"].as_str(), live["handoff"]["summary"].as_str()), (Some("handoff"), Some("Routing the rail.")), "{live}");
    // a link to the message the card names: its thread, and the message
    let r = ask(&mut ws, 3, "link.resolve", json!({"url":"backplane://thread/t1#m1"})).await; assert_eq!((r["result"]["kind"].as_str(), r["result"]["thread"].as_str(), r["result"]["msg"].as_str()), (Some("thread"), Some("t1"), Some("m1")), "{r}");
    // the popup rows say what the writing model made of a thread, else its project
    let r = ask(&mut ws, 4, "mentions.list", json!({"q":"fir","kind":">"})).await; assert_eq!(r["result"]["items"][0]["detail"], "alpha", "{r}");
    hub.commit(&json!({"$":"SettingSet","key":"summary.t1","value":"Routing the rail; DRC left.","at":1700000060})).await.unwrap();
    let r = ask(&mut ws, 5, "mentions.list", json!({"q":"fir","kind":">"})).await; assert_eq!(r["result"]["items"][0]["detail"], "Routing the rail; DRC left.", "{r}");
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
    let root = crate::plat::tmp::dir("links");
    std::fs::create_dir_all(root.join("src")).unwrap(); std::fs::write(root.join("src/x.rs"), "one\ntwo\n").unwrap();
    std::fs::create_dir_all(root.join("boards")).unwrap(); std::fs::write(root.join("boards/a.kicad_sch"), "(kicad_sch)").unwrap();
    let sch = format!("file://{}/boards/a.kicad_sch", root.to_string_lossy());
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
        // a design file of another project (a file:// address, the thread given is another's): that project's thread, the path from its root
        (r(&sch, "t1"), json!({"kind":"file","thread":"t9","path":"boards/a.kicad_sch","line":0})),
        (r(&sch, ""), json!({"kind":"file","thread":"t9","path":"boards/a.kicad_sch"})),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_the_viewer_lists_boards_for_a_picker_and_split_takes_one_of_the_folder() {
    let (_hub, port) = up("viewboards").await;
    let mut ws = open(port, "&enc=json").await; let _ = next(&mut ws).await;
    // the viewer's files carry the picker's boards: none for a folder with no second board
    let r = ask(&mut ws, 3, "viewer.files", json!({"thread":"t1"})).await;
    if r["ok"] == true { assert_eq!(r["result"]["boards"], json!([]), "{r}"); }
    // Split takes a picked board only from the thread's own folder
    let r = ask(&mut ws, 4, "viewer.watch", json!({"thread":"t1","kind":"split","board":"/etc/x.kicad_pcb"})).await; assert_eq!(r["ok"], false, "{r}");
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
    // a project's page asks by project: the same answer, here nothing to buy
    let r = ask(&mut ws, 8, "thread.buy", json!({"project":"p1"})).await; assert_eq!((r["ok"].as_bool(), &r["result"]["buy"]), (Some(true), &Value::Null), "{r}");
    // the phones' project page card: the goal and the boards by name (none here), never an error for a project with no design yet
    let r = ask(&mut ws, 9, "project.card", json!({"project":"p1"})).await; assert_eq!((r["ok"].as_bool(), &r["result"]["boards"]), (Some(true), &json!([])), "{r}");
    let r = ask(&mut ws, 10, "project.card", json!({})).await; assert_eq!(r["ok"], false, "{r}");
    assert_eq!(super::ext::project_card(&json!({"goal":"A gimbal node","board_files":["Nodes/A/A.kicad_pcb","B.kicad_pcb"]})), json!({"goal":"A gimbal node","boards":["A","B"]}));
    let r = ask(&mut ws, 7, "threads.list", json!({"project":"p1"})).await;
    assert!(r["result"]["threads"].as_array().unwrap().iter().all(|t| t["status"].is_string()), "every thread says its dot's word: {r}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_a_bot_is_deleted_only_on_its_own_hub() {
    let (_hub, port) = up("botdel").await;
    let mut ws = open(port, "&enc=json").await; let _ = next(&mut ws).await;
    // Delete bot: a linked machine's bot is deleted there, and a bot is named
    let r = ask(&mut ws, 1, "bot.delete", json!({"bot":"L~b1"})).await; assert_eq!(r["ok"], false, "{r}");
    let r = ask(&mut ws, 2, "bot.delete", json!({})).await; assert_eq!(r["ok"], false, "{r}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_the_files_approval_of_a_buy_says_send_or_not_now_as_the_desktop() {
    let (_hub, port) = up_with("quoteask", "{\"$\":\"AskOpened\",\"id\":\"q2\",\"thread\":\"t2\",\"kind\":\"approval\",\"tool\":\"order_quote\",\"detail\":\"Send 11 files from mfg/x to Backplane's quote service\",\"options\":\"\",\"at\":1700000041}\n").await;
    let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let r = ask(&mut ws, 1, "thread.entries", json!({"thread":"t2"})).await;
    let a = r["result"]["entries"].as_array().unwrap().iter().find(|e| e["kind"] == "ask").cloned().unwrap_or_else(|| panic!("{r}"));
    assert_eq!(a["ask"]["options"], json!([{"id":"accept","label":"Send"},{"id":"decline","label":"Not now"}]), "{a}");
    assert!(a["ask"]["question"].as_str().unwrap().starts_with("Send these files for a quote?\n"), "{a}");
    let r = ask(&mut ws, 2, "thread.entries", json!({"thread":"t1"})).await;
    let a = r["result"]["entries"].as_array().unwrap().iter().find(|e| e["kind"] == "ask").cloned().unwrap();
    assert_eq!(a["ask"]["options"][0]["label"], "Allow", "any other approval keeps Allow: {a}");
}

/// A phone's message to a thread on a linked machine goes through the far router (held there while that machine is away, cutting into its turn when it arrives),
/// never refused as "unknown thread"; pictures are refused with why; its queue rows' buttons go the same way.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_a_message_to_a_thread_on_another_machine_goes_through_the_far_router() {
    struct View; impl crate::api::FarView for View { fn has(&self, t: &str) -> bool { t == "L1~ta" } fn page(&self, _t: &str, _b: u64, _l: usize) -> Option<crate::model::Page> { None } fn latest_context(&self, _t: &str) -> Vec<crate::model::Row> { vec![] } }
    struct Router(std::sync::Mutex<Vec<(String, Value)>>);
    impl crate::api::FarRouter for Router {
        fn wants(&self, _m: &str, p: &Value) -> bool { p["thread"].as_str().map_or(false, |t| t.contains('~')) }
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, _c: u64, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
            Box::pin(async move { self.0.lock().unwrap().push((m.to_string(), p.clone())); if m == "turn.start" { vec![json!({"t":"reply","id":id,"ok":true,"result":{"held":true}})] } else { vec![] } })
        }
        // data/1 asks and waits for the answer (net/rpc.rs Router::ask: a message is held there and answered at once)
        fn ask<'a>(&'a self, _h: &'a Arc<Hub>, _c: u64, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Option<Value>> {
            Box::pin(async move { if !self.wants(m, p) { return None } self.0.lock().unwrap().push((m.to_string(), p.clone())); Some(json!({"t":"reply","id":id,"ok":true,"result":if m == "turn.start" { json!({"held":true}) } else { json!({}) }})) })
        }
    }
    let (hub, port) = up("farsend").await;
    hub.set_far(Arc::new(View)); let r = Arc::new(Router(Default::default())); hub.set_far_router(r.clone());
    let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let a = ask(&mut ws, 1, "message.send", json!({"thread":"L1~ta","text":"cut in there","clientId":"far-1"})).await;
    assert_eq!((a["ok"].as_bool(), a["result"]["entry"]["clientId"].as_str()), (Some(true), Some("far-1")), "{a}");
    let seen = r.0.lock().unwrap().clone(); assert_eq!(seen.len(), 1); assert_eq!((seen[0].0.as_str(), seen[0].1["thread"].as_str(), seen[0].1["text"].as_str(), seen[0].1["msg"].as_str()), ("turn.start", Some("L1~ta"), Some("cut in there"), Some("far-1")));
    let png = "iVBORw0K";
    assert_eq!(ask(&mut ws, 2, "message.send", json!({"thread":"L1~ta","text":"x","attachments":[{"name":"a.png","mime":"image/png","data":png}]})).await["ok"], false, "pictures are not sent there yet");
    assert_eq!(ask(&mut ws, 3, "message.send", json!({"thread":"L1~zz","text":"x"})).await["ok"], false, "a thread no mirror has is unknown");
    let q = ask(&mut ws, 4, "queue.act", json!({"thread":"L1~ta","action":"q-remove","value":"m9"})).await; assert_eq!(q["ok"], true, "{q}");
    assert_eq!(r.0.lock().unwrap().last().map(|x| x.0.clone()).as_deref(), Some("queue.remove"), "a far queue button goes to that machine");
    // the thread can be subscribed to, and hears when its held rows change (a message delivered once that machine is back: the row clears)
    let sub = ask(&mut ws, 5, "subscribe", json!({"topic":"thread","thread":"L1~ta"})).await; assert_eq!(sub["ok"], true, "{sub}");
    let _ = hub.frames.send(json!({"t":"queue","thread":"L1~ta"}));
    let push = loop { let v = next(&mut ws).await; if v["t"] == "d" && v["k"] == "queue" { break v } };
    assert_eq!((push["thread"].as_str(), push["sub"].as_u64()), (Some("L1~ta"), sub["result"]["sub"].as_u64()));
}

/// A reply cut by a message: its final message replaces its own stream's entry even when it comes after the next reply began streaming, so the new reply's
/// first words stay (they were lost before: the final message replaced the one shared `live:<thread>` entry).
#[test] fn data_a_cut_replys_final_message_replaces_its_own_stream_not_the_next() {
    use super::Streams;
    let mut l = Streams::default();
    assert_eq!(l.delta(), (0, true)); assert_eq!(l.delta(), (0, false)); // the first reply streams
    l.end(); // it is cut: its end comes in order with the deltas
    assert_eq!(l.delta(), (1, true), "the next reply has an entry of its own"); assert_eq!(Streams::id("t", 1), "live:t.1");
    assert_eq!(l.finished(), Some(0), "the first reply's final message, late, replaces the first stream");
    assert_eq!(l.delta(), (1, false)); l.end(); assert_eq!(l.finished(), Some(1));
    // a final message before its stream's end: it replaces the open stream, and that end closes nothing else
    assert_eq!(l.delta(), (2, true)); assert_eq!(l.finished(), Some(2)); l.end(); assert_eq!(l.delta(), (3, true)); assert_eq!(l.finished(), Some(3));
    assert_eq!(Streams::default().finished(), None, "a final message with no stream replaces nothing");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_improvements_lists_the_projects_and_remembers_the_choice() {
    let roots = crate::plat::tmp::dir("improve-roots"); let (a, b) = (roots.join("alpha"), roots.join("beta"));
    for d in [&a, &b] { std::fs::create_dir_all(d).unwrap(); } // only a project whose folder is there is offered
    let more = format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"alpha\",\"root\":{},\"at\":1700000100}}\n{{\"$\":\"ProjectCreated\",\"id\":\"p2\",\"title\":\"beta\",\"root\":{},\"at\":1700000101}}\n", serde_json::Value::from(a.to_string_lossy()), serde_json::Value::from(b.to_string_lossy()));
    let (h, port) = up_with("improve", &more).await; h.register_rpc(Arc::new(crate::settings::improvements::ImprovementsRpc)); let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let r = ask(&mut ws, 1, "improvements.projects", json!({})).await; let ids: Vec<&str> = r["result"]["projects"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(r["result"]["project"], "", "nothing chosen yet: {r}"); assert!(ids.contains(&"p1") && ids.contains(&"p2"), "{ids:?}");
    let r = ask(&mut ws, 2, "improvements.project", json!({"project":"p2"})).await; assert_eq!(r["ok"], true, "{r}");
    let r = ask(&mut ws, 3, "improvements.projects", json!({})).await; assert_eq!((r["result"]["project"].as_str(), r["result"]["projects"][0]["id"].as_str()), (Some("p2"), Some("p2")), "the last choice, first");
    let r = ask(&mut ws, 4, "improvements.project", json!({"project":"nope"})).await; assert_eq!(r["ok"], false, "no such project");
}
/// Bot.private (Nov 10: Bub's chat on the phone showed a launched thread's whole report): what the hub hands a bot is not the person's chat
#[test] fn a_bots_conversation_hides_what_the_hub_hands_the_bot() {
    use crate::agents::{launch, tasks, work};
    let hide = |id: &str, role: &str, text: &str| crate::present::thread::bot_private(id, role, text);
    // reports and notes to the bot
    assert!(hide("t:t9:lr-r1", "user", &launch::note("t9", "Always-on hub", "completed", "Bub: **Prep** done")), "a launched thread's report");
    assert!(hide("t:t9:lr-r2", "user", &launch::note("t9", "Always-on hub", "failed", "")), "a failed one");
    assert!(hide("t:t9:td-k1", "system", &tasks::note("k1", "claude", "opus", "completed", "done it")), "a delegated task's result");
    assert!(hide("w1", "system", &work::text("build", "exit 0", "")), "watched work");
    assert!(hide("m1", "user", crate::settings::driver::SIGNIN_TEXT), "a provider handover");
    assert!(hide("m2", "user", crate::settings::driver::RESUME_TEXT), "a limit reset");
    assert!(hide("m3", "user", crate::agents::ACCOUNT_RETRY_TEXT), "an account retry");
    assert!(hide("x1", "user", "[mail c1 · email from Ann]\nCurrent actionable input: x1\nhello"), "mailroom input");
    assert!(hide("x2", "system", "[mailroom escalation from c1] look at this"), "an escalation");
    assert!(hide("told:m9", "user", "[Backplane] Your private mailroom just told the person this in your conversation (key k)."), "the mailroom's told note");
    assert!(hide("t:t3:abc", "user", "[message from t3] the board is routed"), "a message from another thread");
    assert!(hide("r1", "user", "[routine: daily] summarise"), "a routine's prompt");
    assert!(hide("h1", "user", "[webhook: ring] note"), "a webhook");
    assert!(hide("c1", "user", "Backplane restarted while you were working. Continue where you left off."), "the restart carry");
    assert!(hide("s1", "system", "anything the hub says"), "any system note");
    // what the person needs
    assert!(!hide("t:t9:lr-r3", "user", &launch::ask_note("t9", "Always-on hub", "question (AskUserQuestion): which box?")), "a launched thread that waits on the person");
    assert!(!hide("m10", "user", "move the hub to the Boat"), "the person's own words");
    assert!(!hide("m11", "user", "[message from person@phone] hi"), "the person from another device");
    assert!(!hide("m12", "user", "[message from ann@far] hi"), "a peer");
    assert!(!hide("m13", "user", "[message from ann in #ops] hi"), "a room");
    assert!(!hide("a1", "assistant", "[Backplane] anything"), "the bot's own reply");
    // only in a bot's conversation
    let report = launch::note("t9", "x", "completed", "done");
    assert!(crate::present::thread::hidden(true, "t:t9:lr-r1", "user", &report) && !crate::present::thread::hidden(false, "t:t9:lr-r1", "user", &report));
    assert!(crate::present::thread::hidden(false, "h", "user", &format!("{}\"First\" (t1).", crate::present::thread::HANDOFF_HEAD)), "agent_only stays everywhere");
}

/// Nov 11: the person's Reply to one of Bub's messages. A phone gets what they wrote as the text and what it answers as `replyTo`
/// (docs/PHONE-DATA.md, Entry), never the raw `<in_reply_to>` block.
#[test]
fn a_reply_reaches_a_phone_as_its_body_and_reply_to() {
    let text = crate::bots::reply::wrap("msg_A:0:aa", "All done:\n- **KiStack PR #14 is merged**", "make KiCad faster");
    let row = |text: &str| crate::model::Row { seq: 1, ord: 1, id: "c-1".into(), thread: "t1".into(), src: String::new(), kind: "msg", role: "user".into(), akind: String::new(), text: text.into(), at: 1 };
    let e = row_entry_hid(&row(&text), false, &HashSet::new()).unwrap();
    assert_eq!((e["kind"].as_str(), e["text"].as_str()), (Some("user"), Some("make KiCad faster")));
    assert_eq!(e["replyTo"], json!({"id": "msg_A:0:aa", "quote": "All done:"}));
    let plain = row_entry_hid(&row("just a message"), false, &HashSet::new()).unwrap();
    assert!(plain.get("replyTo").is_none() && plain["text"] == "just a message");
}
#[test] fn phones_get_the_management_panels_and_checks_info() {
    use super::phone_info;
    for k in ["hub.offer", "mgmt.state.abc", "mgmt.bom.abc", "mgmt.life.abc", "mgmt.fab.abc", "kicad.checks.abc"] { assert!(phone_info(k), "{k}") }
    for k in ["editor.kicad.abc", "kicad.progress.abc", "browser.tabs.x", "terms"] { assert!(!phone_info(k), "{k}") }
    assert!(super::FEATURES.contains(&"mgmt"));
    // a set_info frame as the hub pushes it: the phone's keys kept, the rest left out
    let f = super::phone_info_frame(&json!({"t":"info","info":{"mgmt.bom.d1":"{}","editor.kicad.d1":"x"}})).unwrap();
    assert_eq!(f, json!({"t":"info","info":{"mgmt.bom.d1":"{}"}}));
    assert!(super::phone_info_frame(&json!({"t":"info","info":{"terms":"[]"}})).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn data_reply_images_are_listed_and_read_back() {
    use base64::Engine;
    let (hub, port) = up("replyimg").await; let mut ws = open(port, "&enc=json").await; next(&mut ws).await;
    let src = crate::plat::tmp::canonical("replyimg-src");
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 4, 0, 0, 0, 4, 8, 6, 0, 0, 0]; png.extend([0u8; 16]);
    std::fs::write(src.join("hs_top.png"), &png).unwrap(); let p = src.join("hs_top.png").to_string_lossy().into_owned();
    let other = src.join("other.png"); std::fs::write(&other, &png).unwrap();
    // a path on its own line, as the agents write them
    hub.commit(&json!({"$":"MessagePosted","thread":"t2","msg":"r1","role":"assistant","text":format!("The heatsink is on the board now.\n{p}\nThe model is a simplified one."),"at":1700000050})).await.unwrap();
    let en = ask(&mut ws, 1, "thread.entries", json!({"thread":"t2"})).await;
    let r = en["result"]["entries"].as_array().unwrap().iter().find(|x| x["id"] == "r1").unwrap().clone();
    assert_eq!(r["images"], json!([p]), "{r}");
    let g = ask(&mut ws, 2, "message.image", json!({"entry":"r1","path":p})).await;
    assert_eq!(g["result"]["mime"], "image/png", "{g}");
    assert_eq!(base64::engine::general_purpose::STANDARD.decode(g["result"]["data"].as_str().unwrap()).unwrap(), png);
    // only what the message named, only in its spelling; a user's message grants nothing
    assert_eq!(ask(&mut ws, 3, "message.image", json!({"entry":"r1","path":other.to_string_lossy()})).await["ok"], false);
    assert_eq!(ask(&mut ws, 4, "message.image", json!({"entry":"nope","path":p})).await["ok"], false);
    assert_eq!(ask(&mut ws, 5, "message.image", json!({"entry":"r1","path":"/etc/passwd"})).await["ok"], false);
    // a reply with no image has no list
    let plain = en["result"]["entries"].as_array().unwrap().iter().find(|x| x["kind"] == "assistant" && x["id"] != "r1").cloned();
    if let Some(x) = plain { assert!(x.get("images").is_none(), "{x}") }
}
/// Copyable (md/copy.rs): a phone draws its own Markdown, so a reply and a notice carry what of them is to copy as `copy: [{text, cmd}]`, and nothing
/// when there is none (the JSON stays as it was).
#[test]
fn a_reply_and_a_notice_carry_their_code_to_copy() {
    let row = |role: &str, text: &str| crate::model::Row { seq: 1, ord: 1, id: "a-1".into(), thread: "t1".into(), src: String::new(), kind: "msg", role: role.into(), akind: String::new(), text: text.into(), at: 1 };
    let e = row_entry_hid(&row("assistant", "Run `sudo apt-get install -y x`:\n```bash\nmake\nmake install\n```\nthen see `foo()`."), false, &HashSet::new()).unwrap();
    assert_eq!(e["copy"], json!([{"text":"sudo apt-get install -y x","cmd":true},{"text":"make\nmake install","cmd":true}]));
    assert!(row_entry_hid(&row("assistant", "no code"), false, &HashSet::new()).unwrap().get("copy").is_none());
    assert!(row_entry_hid(&row("user", "`git status`"), false, &HashSet::new()).unwrap().get("copy").is_none(), "the person's own words get none");
    let n = act_entry("t1", "x1", "warn", "error", "the build failed: run `scripts/land.sh --dry` in `native/host`", 1).unwrap();
    assert_eq!(n["kind"], "note"); assert_eq!(n["copy"], json!([{"text":"scripts/land.sh --dry","cmd":true},{"text":"native/host","cmd":false}]));
    assert!(act_entry("t1", "x2", "warn", "error", "plain words", 1).unwrap().get("copy").is_none());
    assert!(super::FEATURES.contains(&"copy") && super::FEATURES.contains(&"termChat"));
}

#[tokio::test]
async fn viewer_rail_returns_hub_rail_for_local_and_far_threads() {
    use crate::present::rail::{rail, RailInput};
    let (hub, port) = up("viewer-rail").await;
    // Set up a rootless local thread and a shown mirror (L) plus an unshown mirror (H).
    hub.commit(&json!({"$":"ProjectCreated","id":"p_noroot","title":"empty","root":"","at":1700000040})).await.unwrap();
    hub.commit(&json!({"$":"ThreadCreated","id":"t_noroot","project":"p_noroot","title":"Rootless","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000041})).await.unwrap();
    hub.mirrors().set_names(Arc::new(|| vec![("L".to_string(), "desktop".to_string())]));
    hub.mirrors().fold("L", &[
        json!({"$":"ProjectCreated","id":"L~p1","title":"Board","root":"/far/board","at":1}),
        json!({"$":"ThreadCreated","id":"L~t1","project":"L~p1","title":"far one","at":10}),
        json!({"$":"ThreadCreated","id":"L~del","project":"L~p1","title":"far gone","at":11}),
        json!({"$":"ThreadDeleted","id":"L~del","at":12}),
        json!({"$":"ProjectCreated","id":"L~p2","title":"NoRoot","root":"","at":1}),
        json!({"$":"ThreadCreated","id":"L~noroot","project":"L~p2","title":"far rootless","at":13}),
    ]);
    hub.mirrors().fold("H", &[
        json!({"$":"ProjectCreated","id":"H~p1","title":"Hidden","root":"/hidden/board","at":1}),
        json!({"$":"ThreadCreated","id":"H~t1","project":"H~p1","title":"hidden one","at":10}),
    ]);

    let mut ws = open(port, "&enc=json").await;
    let hello = next(&mut ws).await;
    assert!(hello["features"].as_array().unwrap().iter().any(|f| f == "rail"));

    // Local thread: exact match against the shared rail builder for the thread's design root
    let root = crate::files::kicad::canonical::design_root("/tmp/alpha");
    let src = crate::present::viewer::rail_sources(&root);
    let r = ask(&mut ws, 1, "viewer.rail", json!({"thread":"t1","page":"files"})).await;
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["mode"], "viewer");
    let expected_local = serde_json::to_value(rail(RailInput {
        board: src.board, schematic: src.schematic, mech: src.mech, gerbers: src.gerbers,
        simulation: false, altium: src.altium, page: "files", diff: false,
    })).unwrap();
    assert_eq!(r["result"]["rail"], expected_local);
    assert_eq!(r["result"]["has"], json!({"board": src.board, "schematic": src.schematic, "mech": src.mech, "gerbers": src.gerbers}));
    assert!(!r["result"]["rail"].as_array().unwrap().iter().any(|x| x["id"] == "simulation"));
    // Even when page=="gerbers" with no Gerber set, `has.gerbers` stays false so phones reconcile away from a removed source
    let rg = ask(&mut ws, 4, "viewer.rail", json!({"thread":"t1","page":"gerbers"})).await;
    assert_eq!(rg["ok"], true);
    assert_eq!(rg["result"]["has"]["gerbers"], src.gerbers);

    // diff=true lights the diff entry regardless of page
    let rd = ask(&mut ws, 2, "viewer.rail", json!({"thread":"t1","page":"files","diff":true})).await;
    assert_eq!(rd["ok"], true);
    let expected_diff = serde_json::to_value(rail(RailInput {
        board: src.board, schematic: src.schematic, mech: src.mech, gerbers: src.gerbers,
        simulation: false, altium: src.altium, page: "files", diff: true,
    })).unwrap();
    assert_eq!(rd["result"]["rail"], expected_diff);

    // Known linked thread on a shown mirror with a folder: exact match against optimistic far rail
    let rf = ask(&mut ws, 3, "viewer.rail", json!({"thread":"L~t1","page":"board"})).await;
    assert_eq!(rf["ok"], true);
    assert_eq!(rf["result"]["mode"], "viewer");
    let expected_far = serde_json::to_value(rail(RailInput {
        board: true, schematic: true, mech: true, gerbers: false,
        simulation: false, altium: false, page: "board", diff: false,
    })).unwrap();
    assert_eq!(rf["result"]["rail"], expected_far);
    assert_eq!(rf["result"]["has"], json!({"board": true, "schematic": true, "mech": true, "gerbers": false}));

    // Refused cases: unknown/deleted/unshown/rootless linked threads, unknown local, rootless local
    for (i, th) in ["L~nope", "L~del", "L~noroot", "H~t1", "nope", "t_noroot"].iter().enumerate() {
        let bad = ask(&mut ws, 10 + i as u64, "viewer.rail", json!({"thread":th,"page":"board"})).await;
        assert_eq!(bad["ok"], false, "expected refusal for thread {th}, got {bad}");
    }
    drop(hub);
}

#[tokio::test]
async fn viewer_gerbers_files_and_watch() {
    struct WatchSpy(Arc<Mutex<Vec<Value>>>);
    impl RpcHandler for WatchSpy {
        fn methods(&self) -> &'static [&'static str] { &["kicad.watch", "boards.list"] }
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, _w: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
            if m == "boards.list" {
                return Box::pin(async move { vec![json!({"t":"reply","id":id,"ok":true,"result":{"board_default":"","board_list":[]}})] });
            }
            self.0.lock().unwrap().push(p.clone());
            let key = format!("0|{}|{}", s(p, "root"), s(p, "path"));
            Box::pin(async move { vec![json!({"t":"reply","id":id,"ok":true,"result":{"keys":[key]}})] })
        }
    }

    let base = crate::plat::tmp::dir("viewer-gerbers");
    let proj = base.join("proj");
    let outside = base.join("outside");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("ext-F_Cu.gbr"), "G04 ext*\nM02*\n").unwrap();
    let root = proj.to_string_lossy().into_owned();
    let more = format!(
        "{}\n{}\n",
        json!({"$":"ProjectCreated","id":"pg","title":"gerb","root":root,"at":1700000040}),
        json!({"$":"ThreadCreated","id":"tg","project":"pg","title":"Gerb","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000041}),
    );
    let (hub, port) = up_with("viewer-gerbers", &more).await;
    hub.mirrors().set_names(Arc::new(|| vec![("L".to_string(), "desktop".to_string())]));
    hub.mirrors().fold("L", &[
        json!({"$":"ProjectCreated","id":"L~p1","title":"Far","root":root,"at":1}),
        json!({"$":"ThreadCreated","id":"L~t1","project":"L~p1","title":"far one","at":10}),
    ]);
    let seen = Arc::new(Mutex::new(Vec::new()));
    hub.register_rpc(Arc::new(WatchSpy(seen.clone())));

    let mut ws = open(port, "&enc=json").await;
    let hello = next(&mut ws).await;
    assert!(hello["features"].as_array().unwrap().iter().any(|f| f == "gerbers"));

    // Without Gerbers: viewer.files reports hasGerbers=false, viewer.watch kind="gerbers" errors and enqueues no watch
    let f0 = ask(&mut ws, 1, "viewer.files", json!({"thread":"tg"})).await;
    assert_eq!(f0["ok"], true);
    assert_eq!(f0["result"]["hasGerbers"], false);
    let w0 = ask(&mut ws, 2, "viewer.watch", json!({"thread":"tg","kind":"gerbers"})).await;
    assert_eq!(w0["ok"], false);
    assert_eq!(w0["error"], "no Gerbers in this project");
    assert!(seen.lock().unwrap().is_empty(), "rejected watch must enqueue nothing");

    // Linked set directory outside root is ignored (hasGerbers=false, watch refused, no watch enqueued)
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, proj.join("gerbers")).unwrap();
        let fl = ask(&mut ws, 3, "viewer.files", json!({"thread":"tg"})).await;
        assert_eq!(fl["result"]["hasGerbers"], false);
        let wl = ask(&mut ws, 4, "viewer.watch", json!({"thread":"tg","kind":"gerbers"})).await;
        assert_eq!(wl["ok"], false);
        assert_eq!(wl["error"], "no Gerbers in this project");
        assert!(seen.lock().unwrap().is_empty());
        std::fs::remove_file(proj.join("gerbers")).unwrap();
    }

    // With in-root Gerbers under gerbers/ (and an outside-linked member skipped): hasGerbers=true, watch resolves default_set
    let gdir = proj.join("gerbers");
    std::fs::create_dir_all(&gdir).unwrap();
    std::fs::write(gdir.join("board-F_Cu.gbr"), "G04 test*\nM02*\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.join("ext-F_Cu.gbr"), gdir.join("leak-B_Cu.gbr")).unwrap();
    let f1 = ask(&mut ws, 5, "viewer.files", json!({"thread":"tg"})).await;
    assert_eq!(f1["ok"], true);
    assert_eq!(f1["result"]["hasGerbers"], true);

    let w1 = ask(&mut ws, 6, "viewer.watch", json!({"thread":"tg","kind":"gerbers"})).await;
    assert_eq!(w1["ok"], true, "{w1}");
    let expected_set = gdir.to_string_lossy().into_owned();
    let calls = seen.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["kind"], "board");
    assert_eq!(calls[0]["path"], expected_set);
    assert!(calls[0].get("force").is_none(), "normal watch omits force");

    // Retry with force:true forwards force=true to kicad.watch
    let w_force = ask(&mut ws, 11, "viewer.watch", json!({"thread":"tg","kind":"board","force":true})).await;
    assert_eq!(w_force["ok"], true, "{w_force}");
    assert_eq!(seen.lock().unwrap().last().unwrap()["force"], true);

    // Explicit path (even inside project or outside) is refused for kind="gerbers" and enqueues no watch
    let w_exp = ask(&mut ws, 7, "viewer.watch", json!({"thread":"tg","kind":"gerbers","path":"board.kicad_pcb"})).await;
    assert_eq!(w_exp["ok"], false);
    let w_bad = ask(&mut ws, 8, "viewer.watch", json!({"thread":"tg","kind":"gerbers","path":"/etc/passwd"})).await;
    assert_eq!(w_bad["ok"], false);

    // Far thread: viewer.files reports hasGerbers=false, viewer.watch refused before any local fs access
    let ff = ask(&mut ws, 9, "viewer.files", json!({"thread":"L~t1"})).await;
    assert_eq!(ff["ok"], true, "{ff}");
    assert_eq!(ff["result"]["hasGerbers"], false);
    let wf = ask(&mut ws, 10, "viewer.watch", json!({"thread":"L~t1","kind":"gerbers"})).await;
    assert_eq!(wf["ok"], false);
    assert_eq!(wf["error"], "Gerbers of a project on another machine are not shown yet");
    assert_eq!(seen.lock().unwrap().len(), 2, "rejected requests must enqueue no extra kicad.watch");
    drop(hub);
}

#[tokio::test]
async fn socket_with_browser_enforces_policy_activity_and_revoke_over_generic_stream() {
    let (hub, _port) = up_with("socket-with-browser", "{\"$\":\"SettingSet\",\"key\":\"web.enabled\",\"value\":\"1\",\"at\":1700000040}\n").await;
    let (reg, _routes) = crate::net::start::open(&hub.hub.cfg.home, "127.0.0.1", 0).unwrap();
    let (req_json, poll_cookie) = reg.browser_request("person@example.com", "Test Browser").unwrap();
    assert!(reg.browser_decide(req_json["id"].as_str().unwrap(), true));
    let session_cookie = reg.browser_wait("person@example.com", &poll_cookie).await.unwrap().unwrap();
    let device_id = reg.browser_verify("person@example.com", &session_cookie, None, false).unwrap();

    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let h2 = hub.hub.clone();
    let dev2 = device_id.clone();
    let srv = tokio::spawn(async move { socket_with(h2, server_io, true, Principal::Browser { device_id: dev2 }, "bp.ts.net".into()).await });
    let (mut ws, _) = tokio_tungstenite::client_async("ws://bp.ts.net/ws?data=1&enc=json", client_io).await.unwrap();
    let hello: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(hello["t"], "hello");
    ws.send(Message::Text(json!({"id":1,"m":"projects.list","p":{}}).to_string().into())).await.unwrap();
    let r1: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(r1["ok"], true);
    ws.send(Message::Text(json!({"id":2,"m":"devices.page.act","p":{"action":"device-invite"}}).to_string().into())).await.unwrap();
    let r2: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!((r2["id"].clone(), r2["ok"].clone(), r2["error"].clone()), (json!(2), json!(false), json!("not allowed for browser")));
    reg.browser_request("another@example.com", "Pending device").unwrap();
    for params in [json!({}), json!({"view":"native","action":"device-invite","state":{"invite":"show"}})] {
        ws.send(Message::Text(json!({"id":3,"m":"devices.page","p":params}).to_string().into())).await.unwrap();
        let page: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(page["result"], json!({"rows":[{"title":"Test Browser","detail":"This browser"}]}));
    }
    let revoked = reg.revoke(&device_id);
    let clients: Vec<u64> = revoked["clients"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
    assert!(!clients.is_empty(), "browser socket must be bound to registry device");
    hub.hub.close_clients(&clients);
    let next = tokio::time::timeout(Duration::from_secs(1), ws.next()).await.unwrap();
    assert!(matches!(next, None | Some(Ok(Message::Close(_))) | Some(Err(_))), "expected close on revoke, got {next:?}");
    let _ = tokio::time::timeout(Duration::from_secs(1), srv).await.unwrap();
}

#[tokio::test]
async fn viewer_follow_and_hello_theme_and_unanswered_on_data1() {
    let unans = crate::agents::unanswered::value(&[("m1".into(), "Status?".into(), 100)]);
    let more = format!(
        "{}\n{}\n{}\n{}\n",
        json!({"$":"SettingSet","key":"ui.theme","value":"light","at":1700000040}),
        json!({"$":"SettingSet","key":"ui.lang","value":"en","at":1700000041}),
        json!({"$":"TurnChanged","thread":"t1","state":"running","at":1700000042}),
        json!({"$":"SettingSet","key":crate::agents::unanswered::key("t1"),"value":unans,"at":1700000043}),
    );
    let (hub, port) = up_with("viewer-follow-unans", &more).await;
    let (mut ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws?data=1&enc=json&token=tok")).await.unwrap();
    let hello: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(hello["theme"], "light");
    assert_eq!(hello["locale"], "en");
    let feats: Vec<&str> = hello["features"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    for f in ["follow", "unanswered"] { assert!(feats.contains(&f), "missing feature {f}"); }

    ws.send(Message::Text(json!({"id":1,"m":"thread.entries","p":{"thread":"t1"}}).to_string().into())).await.unwrap();
    let en: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(en["ok"], true);
    assert_eq!(en["result"]["unanswered"]["text"], "Status?");

    ws.send(Message::Text(json!({"id":2,"m":"viewer.follow","p":{"thread":"t1","action":"on"}}).to_string().into())).await.unwrap();
    let f1: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!((f1["ok"].clone(), f1["result"]["on"].clone(), f1["result"]["paused"].clone()), (json!(true), json!(true), json!(false)));
    ws.send(Message::Text(json!({"id":3,"m":"viewer.follow","p":{"thread":"t1","action":"pause"}}).to_string().into())).await.unwrap();
    let f2: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(f2["result"]["paused"], json!(true));
    ws.send(Message::Text(json!({"id":4,"m":"viewer.follow","p":{"thread":"t1","action":"bogus"}}).to_string().into())).await.unwrap();
    let f3: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(f3["ok"], false);
    drop(hub);
}

#[tokio::test]
async fn socket_with_browser_refused_and_passive_rpcs_do_not_extend_idle_and_outbound_guards_expiry() {
    use std::sync::atomic::{AtomicU64, Ordering};
    const DAY: u64 = 86_400_000;
    let (hub, _port) = up_with("socket-with-clock", "{\"$\":\"SettingSet\",\"key\":\"web.enabled\",\"value\":\"1\",\"at\":1700000040}\n").await;
    let clock = Arc::new(AtomicU64::new(DAY));
    let c2 = clock.clone();
    let reg = Arc::new(crate::net::pairing::Registry::with(
        &hub.hub.cfg.home,
        "tok",
        Box::new(move || c2.load(Ordering::SeqCst)),
        Box::new(crate::net::pairing::urandom),
    ).unwrap());
    let (req_json, poll_cookie) = reg.browser_request("person@example.com", "Clock Browser").unwrap();
    assert!(reg.browser_decide(req_json["id"].as_str().unwrap(), true));
    let session_cookie = reg.browser_wait("person@example.com", &poll_cookie).await.unwrap().unwrap();
    let device_id = reg.browser_verify("person@example.com", &session_cookie, None, false).unwrap();

    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let (h2, r2, dev2) = (hub.hub.clone(), reg.clone(), device_id.clone());
    let srv = tokio::spawn(async move { socket_with_reg(h2, server_io, true, Principal::Browser { device_id: dev2 }, "bp.ts.net".into(), Some(r2)).await });
    let (mut ws, _) = tokio_tungstenite::client_async("ws://bp.ts.net/ws?data=1&enc=json", client_io).await.unwrap();
    let hello: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(hello["t"], "hello");
    assert_eq!(hello["hubOffer"], "");
    assert_eq!(hello["migrating"], Value::Null);

    // Subscribe to t1, then advance 20 days and send repeated refused + passive refresh requests
    ws.send(Message::Text(json!({"id":1,"m":"subscribe","p":{"topic":"thread","thread":"t1"}}).to_string().into())).await.unwrap();
    assert_eq!(serde_json::from_str::<Value>(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap()["ok"], true);
    clock.fetch_add(20 * DAY, Ordering::SeqCst);
    for id in 2..=4 {
        ws.send(Message::Text(json!({"id":id,"m":"devices.page.act","p":{"action":"device-invite"}}).to_string().into())).await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap()["ok"], false);
    }
    for (id, m, p) in [(5, "projects.list", json!({})), (6, "thread.entries", json!({"thread":"t1"}))] {
        ws.send(Message::Text(json!({"id":id,"m":m,"p":p}).to_string().into())).await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap()["ok"], true);
    }

    // Advance 10 more days (total 30 days since creation): must now be expired (passive/refused did NOT extend idle)
    clock.fetch_add(10 * DAY, Ordering::SeqCst);
    // Trigger an outbound push (MessagePosted on subscribed t1): must close immediately without delivering the entry!
    hub.hub.commit(&json!({"$":"MessagePosted","thread":"t1","msg":"m-after-expiry","role":"assistant","text":"secret after expiry","at":1700000099})).await.unwrap();
    let next = tokio::time::timeout(Duration::from_secs(1), ws.next()).await.unwrap();
    assert!(matches!(next, None | Some(Ok(Message::Close(_))) | Some(Err(_))), "expired socket must close instead of delivering push, got {next:?}");
    let _ = tokio::time::timeout(Duration::from_secs(1), srv).await.unwrap();
}

#[tokio::test]
async fn socket_with_browser_closes_immediately_on_web_enabled_setting_set() {
    let (hub, _port) = up_with("socket-with-disable", "{\"$\":\"SettingSet\",\"key\":\"web.enabled\",\"value\":\"1\",\"at\":1700000040}\n").await;
    let (reg, _routes) = crate::net::start::open(&hub.hub.cfg.home, "127.0.0.1", 0).unwrap();
    let (req_json, poll_cookie) = reg.browser_request("person@example.com", "Disable Browser").unwrap();
    assert!(reg.browser_decide(req_json["id"].as_str().unwrap(), true));
    let session_cookie = reg.browser_wait("person@example.com", &poll_cookie).await.unwrap().unwrap();
    let device_id = reg.browser_verify("person@example.com", &session_cookie, None, false).unwrap();

    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let h2 = hub.hub.clone();
    let srv = tokio::spawn(async move { socket_with(h2, server_io, true, Principal::Browser { device_id }, "bp.ts.net".into()).await });
    let (mut ws, _) = tokio_tungstenite::client_async("ws://bp.ts.net/ws?data=1&enc=json", client_io).await.unwrap();
    let _hello: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();

    // Turning web.enabled off via SettingSet must close the browser socket immediately (< 1s, not waiting for 15s heartbeat)
    hub.hub.commit(&json!({"$":"SettingSet","key":"web.enabled","value":"0","at":1700000050})).await.unwrap();
    let next = tokio::time::timeout(Duration::from_secs(1), ws.next()).await.unwrap();
    assert!(matches!(next, None | Some(Ok(Message::Close(_))) | Some(Err(_))), "expected immediate close on web.enabled=0, got {next:?}");
    let _ = tokio::time::timeout(Duration::from_secs(1), srv).await.unwrap();
}

#[tokio::test]
async fn socket_with_browser_drops_in_flight_handle_reply_when_disabled_mid_call() {
    struct HeldRpc { started: Arc<tokio::sync::Notify>, release: Arc<tokio::sync::Notify> }
    impl RpcHandler for HeldRpc {
        fn methods(&self) -> &'static [&'static str] { &["thread.again"] }
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, _w: &'a Caller, id: &'a Value, _m: &'a str, _p: &'a Value) -> Fut<'a, Vec<Value>> {
            let (started, release, id) = (self.started.clone(), self.release.clone(), id.clone());
            Box::pin(async move {
                started.notify_one();
                release.notified().await;
                vec![json!({"t":"reply","id":id,"ok":true,"result":{"secret":"must-not-escape"}})]
            })
        }
    }

    let (hub, _port) = up_with("socket-with-held", "{\"$\":\"SettingSet\",\"key\":\"web.enabled\",\"value\":\"1\",\"at\":1700000040}\n").await;
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    hub.hub.register_rpc(Arc::new(HeldRpc { started: started.clone(), release: release.clone() }));
    let (reg, _routes) = crate::net::start::open(&hub.hub.cfg.home, "127.0.0.1", 0).unwrap();
    let (req_json, poll_cookie) = reg.browser_request("person@example.com", "Held Browser").unwrap();
    assert!(reg.browser_decide(req_json["id"].as_str().unwrap(), true));
    let session_cookie = reg.browser_wait("person@example.com", &poll_cookie).await.unwrap().unwrap();
    let device_id = reg.browser_verify("person@example.com", &session_cookie, None, false).unwrap();

    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let h2 = hub.hub.clone();
    let srv = tokio::spawn(async move { socket_with(h2, server_io, true, Principal::Browser { device_id }, "bp.ts.net".into()).await });
    let (mut ws, _) = tokio_tungstenite::client_async("ws://bp.ts.net/ws?data=1&enc=json", client_io).await.unwrap();
    let _hello: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();

    ws.send(Message::Text(json!({"id":1,"m":"thread.again","p":{"thread":"t1"}}).to_string().into())).await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), started.notified()).await.unwrap();
    // Disable web access while handle() is mid-flight, then release the handler
    hub.hub.commit(&json!({"$":"SettingSet","key":"web.enabled","value":"0","at":1700000051})).await.unwrap();
    release.notify_one();
    let next = tokio::time::timeout(Duration::from_secs(1), ws.next()).await.unwrap();
    assert!(matches!(next, None | Some(Ok(Message::Close(_))) | Some(Err(_))), "in-flight reply must not be sent after disable, got {next:?}");
    let _ = tokio::time::timeout(Duration::from_secs(1), srv).await.unwrap();
}

#[tokio::test]
async fn viewer_rail_omits_split_for_browser_only() {
    let base = crate::plat::tmp::dir("rail-principal-split");
    let proj = base.join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    let root = proj.to_string_lossy().into_owned();
    let more = format!(
        "{}\n{}\n{}\n",
        json!({"$":"SettingSet","key":"web.enabled","value":"1","at":1700000040}),
        json!({"$":"ProjectCreated","id":"pr","title":"rail","root":root,"at":1700000041}),
        json!({"$":"ThreadCreated","id":"tr","project":"pr","title":"Rail","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000042}),
    );
    let (hub, port) = up_with("rail-principal-split", &more).await;
    // Native/phone principal keeps `split` in viewer.rail.
    let mut native_ws = open(port, "&enc=json").await;
    let rn = ask(&mut native_ws, 1, "viewer.rail", json!({"thread":"tr","page":"board"})).await;
    assert_eq!(rn["ok"], true, "{rn}");
    assert!(rn["result"]["rail"].as_array().unwrap().iter().any(|x| x["id"] == "split"), "native/phone rail must keep split: {rn}");

    // Browser principal omits `split` in viewer.rail.
    let (reg, _routes) = crate::net::start::open(&hub.hub.cfg.home, "127.0.0.1", 0).unwrap();
    let (req_json, poll_cookie) = reg.browser_request("person@example.com", "Rail Browser").unwrap();
    assert!(reg.browser_decide(req_json["id"].as_str().unwrap(), true));
    let session_cookie = reg.browser_wait("person@example.com", &poll_cookie).await.unwrap().unwrap();
    let device_id = reg.browser_verify("person@example.com", &session_cookie, None, false).unwrap();

    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let h2 = hub.hub.clone();
    let srv = tokio::spawn(async move { socket_with(h2, server_io, true, Principal::Browser { device_id }, "bp.ts.net".into()).await });
    let (mut bws, _) = tokio_tungstenite::client_async("ws://bp.ts.net/ws?data=1&enc=json", client_io).await.unwrap();
    let _hello: Value = serde_json::from_str(&bws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
    bws.send(Message::Text(json!({"id":2,"m":"viewer.rail","p":{"thread":"tr","page":"board"}}).to_string().into())).await.unwrap();
    let rb: Value = serde_json::from_str(&tokio::time::timeout(Duration::from_secs(2), bws.next()).await.unwrap().unwrap().unwrap().into_text().unwrap()).unwrap();
    assert_eq!(rb["ok"], true, "{rb}");
    assert!(!rb["result"]["rail"].as_array().unwrap().iter().any(|x| x["id"] == "split"), "browser rail must omit split: {rb}");
    assert!(rb["result"]["rail"].as_array().unwrap().iter().any(|x| x["id"] == "board"), "browser rail keeps other entries: {rb}");
    drop(bws);
    let _ = tokio::time::timeout(Duration::from_secs(1), srv).await;
    drop(hub);
    let _ = std::fs::remove_dir_all(&base);
}

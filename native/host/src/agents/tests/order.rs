//! The order tools through the agent's own MCP path: what an agent asked to review for manufacturing calls, and what a buy thread's questions do.
use super::*;
use super::lost::rig;
use crate::plat::fs::PermissionsExt;

const SESSION_OK: &str = "{\"$\":\"SessionSet\",\"id\":\"t1700000001-2\",\"session\":\"fresh-old\",\"at\":1700000060}\n";
async fn call(ag: &Arc<Agents>, name: &str, a: Value) -> (bool, String) {
    let (_, r) = rpc_mcp(ag, T1, "", "tools/call", json!({"name":name,"arguments":a})).await;
    (r["result"]["isError"] == false, r["result"]["content"][0]["text"].as_str().unwrap_or("").to_string())
}
async fn names_of(ag: &Arc<Agents>, th: &str) -> Vec<String> { let (_, l) = rpc_mcp(ag, th, "", "tools/list", json!({})).await; l["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str().map(str::to_string)).collect() }
async fn names(ag: &Arc<Agents>) -> Vec<String> { names_of(ag, T1).await }
async fn set(hub: &Arc<Hub>, k: &str, v: &str) { hub.commit(&json!({"$":"SettingSet","key":k,"value":v,"at":1700000070})).await.unwrap(); }
async fn cwd(ag: &Arc<Agents>) -> std::path::PathBuf { std::path::PathBuf::from(ag.world().await.cwd(T1)) }

#[tokio::test(flavor = "multi_thread")] async fn a_review_is_recorded_and_goes_stale_when_the_design_changes() {
    let r = rig("or-review", "result", SESSION_OK).await; let dir = cwd(&r.ag).await;
    assert!(names(&r.ag).await.contains(&"order_review".to_string()), "the prompt tells every agent to call it: it must exist");
    std::fs::write(dir.join("a.kicad_pcb"), "(kicad_pcb)").unwrap();
    let (ok, t) = call(&r.ag, "order_review", json!({"action":"check"})).await; assert!(ok, "{t}"); assert_eq!(serde_json::from_str::<Value>(&t).unwrap()["state"], "none");
    let (ok, t) = call(&r.ag, "order_review", json!({"action":"record","verdict":"issues","summary":"one thing","issues":["Shade: walls 0.6 mm, need 0.8"]})).await; assert!(ok, "{t}");
    assert_eq!(serde_json::from_str::<Value>(&t).unwrap(), json!({"recorded":true,"verdict":"issues"}));
    let rec: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".backplane/review.json")).unwrap()).unwrap();
    assert_eq!(rec["verdict"], "issues"); assert_eq!(rec["issues"], json!(["Shade: walls 0.6 mm, need 0.8"])); assert_eq!(rec["fingerprint"].as_str().unwrap().len(), 32);
    let (_, t) = call(&r.ag, "order_review", json!({"action":"check"})).await; let s: Value = serde_json::from_str(&t).unwrap(); assert_eq!((s["state"].clone(), s["verdict"].clone()), (json!("current"), json!("issues")));
    std::fs::write(dir.join("a.kicad_pcb"), "(kicad_pcb changed)").unwrap();
    let (_, t) = call(&r.ag, "order_review", json!({"action":"check"})).await; assert_eq!(serde_json::from_str::<Value>(&t).unwrap()["state"], "stale");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn ordering_off_lists_no_order_tool_and_refuses_one_remembered() {
    let r = rig("or-off", "result", SESSION_OK).await; set(&r.hub, "order.enabled", "off").await; tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!names(&r.ag).await.iter().any(|n| n.starts_with("order_")));
    let (ok, t) = call(&r.ag, "order_review", json!({"action":"record","verdict":"ok"})).await; assert!(!ok); assert!(t.contains(crate::settings::order::OFF), "{t}");
    assert!(!cwd(&r.ag).await.join(".backplane/review.json").exists(), "nothing is recorded");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn only_a_buy_thread_may_ask_and_its_answer_is_the_persons_message() {
    let r = rig("or-ask", "result", SESSION_OK).await;
    let (ok, t) = call(&r.ag, "order_ask", json!({"question":"How many?"})).await; assert!(!ok); assert!(t.contains(crate::settings::order::NOT_BUY), "{t}");
    set(&r.hub, &format!("buy.{T1}"), "t0").await; tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(names(&r.ag).await.contains(&"order_ask".to_string()));
    let from = len(&r.hub).await;
    let (ok, t) = call(&r.ag, "order_ask", json!({"question":"Ship where?","typed":true,"last":"Germany","options":["France"]})).await; assert!(ok, "{t}"); assert!(t.contains("End your turn now"), "it never waits on the person: {t}");
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "AskOpened"))).await; let ask = ev.iter().find(|e| is(e, "AskOpened")).unwrap().clone();
    assert_eq!((ask["kind"].clone(), ask["tool"].clone(), ask["detail"].clone(), ask["options"].clone()), (json!("input"), json!("order_ask.typed"), json!("Ship where?"), json!("Germany · last time\nFrance")));
    let from = len(&r.hub).await; assert_eq!(rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"Germany · last time"})).await["ok"], true);
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "user" && e["text"] == "Germany"), "the mark is not sent: {ev:?}");
    assert!(ev.iter().any(|e| is(e, "AskClosed") && e["id"] == ask["id"]), "{ev:?}"); assert!(r.users().iter().any(|u| u.contains("Germany")), "the agent hears it: {:?}", r.users());
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_gate_asks_for_itself_unless_the_review_passed() {
    let r = rig("or-gate", "result", SESSION_OK).await; set(&r.hub, &format!("buy.{T1}"), "t0").await; tokio::time::sleep(Duration::from_millis(100)).await;
    let dir = cwd(&r.ag).await; std::fs::write(dir.join("a.kicad_pcb"), "(kicad_pcb)").unwrap();
    let opened = |hub: Arc<Hub>, from: usize| async move { until(&hub, from, 20, |ev| ev.iter().any(|e| is(e, "AskOpened"))).await };
    // none: the two buttons, and the agent is told to end its turn
    let from = len(&r.hub).await; let (ok, t) = call(&r.ag, "order_review", json!({"action":"gate"})).await; assert!(ok); assert!(t.contains("no current manufacturing review"), "{t}");
    let ev = opened(r.hub.clone(), from).await; assert!(ev.iter().any(|e| is(e, "AskOpened") && e["options"] == "Review for manufacturing\nOrder without a review"), "{ev:?}");
    // current with issues: the hub posts them, an agent cannot skip it
    call(&r.ag, "order_review", json!({"action":"record","verdict":"issues","summary":"x","issues":["Walls too thin."]})).await;
    let from = len(&r.hub).await; let (_, t) = call(&r.ag, "order_review", json!({"action":"gate"})).await; assert!(t.contains("found issues"), "{t}");
    let ev = opened(r.hub.clone(), from).await; assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "assistant" && e["text"].as_str().unwrap().contains("- Walls too thin\n")), "{ev:?}");
    assert!(ev.iter().any(|e| is(e, "AskOpened") && e["options"] == "Fix them first\nOrder anyway"), "{ev:?}");
    // current and passed: go on, nothing asked
    call(&r.ag, "order_review", json!({"action":"record","verdict":"ok","summary":"fine"})).await;
    let from = len(&r.hub).await; let (_, t) = call(&r.ag, "order_review", json!({"action":"gate"})).await; assert!(t.contains("go on with the order"), "{t}");
    tokio::time::sleep(Duration::from_millis(200)).await; assert!(!log_from(&r.hub, from).await.iter().any(|e| is(e, "AskOpened")));
    r.ag.shutdown();
}

// ---- Buy it ----
#[tokio::test(flavor = "multi_thread")] async fn buy_it_starts_a_buy_thread_once_and_its_agent_gets_the_buy_block() {
    let r = rig("or-buy", "result", SESSION_OK).await; crate::commands::start(r.hub.clone()); let from = len(&r.hub).await;
    let a = rpc(&r.hub, "order.buy", json!({"thread":T1})).await; assert_eq!(a["ok"], true, "{a}"); let th = a["thread"].as_str().unwrap().to_string(); assert_ne!(th, T1);
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
    let made = ev.iter().find(|e| is(e, "ThreadCreated")).unwrap(); assert!(made["title"].as_str().unwrap().starts_with("Buy: "), "{made}");
    let w = r.ag.world().await; assert_eq!(w.setting(&format!("buy.{th}")), T1, "marked with its source"); assert_eq!(w.cwd(&th), w.cwd(T1), "the same folder"); drop(w);
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["thread"] == th.as_str() && e["role"] == "user" && e["text"] == "Buy it"), "the person's Buy it starts it: {ev:?}");
    let dir = r.ag.world().await.cwd(&th);
    assert!(r.ag.prompt_for(&th, &dir).await.contains("<backplane_buy>"), "its agent gets the buy instructions"); assert!(!r.ag.prompt_for(T1, &dir).await.contains("<backplane_buy>"), "its source's does not");
    assert!(names_of(&r.ag, &th).await.contains(&"order_ask".to_string()), "and the buy thread's tools"); assert!(!names(&r.ag).await.contains(&"order_ask".to_string()));
    // a double press and a stale press in the buy thread make nothing new
    let n = len(&r.hub).await;
    assert_eq!(rpc(&r.hub, "order.buy", json!({"thread":th})).await["thread"], th.as_str(), "a buy thread has no Buy of its own");
    assert_eq!(len(&r.hub).await, n);
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn buy_it_is_refused_with_ordering_off_and_for_no_thread() {
    let r = rig("or-buyoff", "result", SESSION_OK).await; crate::commands::start(r.hub.clone());
    assert_eq!(rpc(&r.hub, "order.buy", json!({"thread":"ghost"})).await["error"], "no such thread");
    set(&r.hub, "order.enabled", "off").await; let n = len(&r.hub).await;
    assert_eq!(rpc(&r.hub, "order.buy", json!({"thread":T1})).await["error"], "Ordering is off: turn it on in Settings to buy"); assert_eq!(len(&r.hub).await, n, "nothing is made");
    r.ag.shutdown();
}

// ---- quotes, against a stand-in order server ----
#[derive(Default)] struct Fake { hits: Vec<String>, upload: String, polls: usize, ready_after: usize, order: Value, pilot: bool, refuse: bool, flaky: usize, fails: bool }
type Shared = Arc<StdMutex<Fake>>;
const TOKEN: &str = "tok-0123456789abcdef-secret";
fn answer_of(f: &mut Fake, line: &str) -> (u16, Value) {
    if line.starts_with("POST /v1/quotes") { return if f.refuse { (400, json!({"error":"unknown file type"})) } else { (202, json!({"id":"q_fake1","token":TOKEN,"status":"received"})) } }
    f.polls += 1;
    if f.polls <= f.flaky { return (if f.polls == 1 { 503 } else { 429 }, json!({"error":"too many requests; try again in 1 min"})) }
    if f.fails { return (200, json!({"id":"q_fake1","status":"failed","pilot":false,"error":"the quote could not be worked out; try again later"})) }
    if f.polls <= f.ready_after { return (200, json!({"id":"q_fake1","status":"analysing"})) }
    let mut q = json!({"id":"q_fake1","status":"ready","pilot":f.pilot,"quote":{"total":112.4,"lead_days":9,"pay_label":"Buy now · $112.40","markdown":"| Total | $112.40 |","problems":[],"warnings":[],"shipping":{"ddp":false}}});
    if !f.order.is_null() { q["order"] = f.order.clone() } (200, q)
}
fn fake_server(ready_after: usize) -> (String, Shared) {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let f: Shared = Arc::new(StdMutex::new(Fake { ready_after, ..Default::default() })); let f2 = f.clone();
    std::thread::spawn(move || for s in l.incoming() { let Ok(mut s) = s else { return }; let mut b = vec![]; let mut t = [0u8; 8192];
        let (head, len) = loop { let Ok(n) = s.read(&mut t) else { break (0, 0) }; if n == 0 { break (0, 0) } b.extend_from_slice(&t[..n]);
            if let Some(i) = b.windows(4).position(|w| w == b"\r\n\r\n") { let h = String::from_utf8_lossy(&b[..i]).to_lowercase(); break (i + 4, h.lines().find_map(|l| l.strip_prefix("content-length: ")).and_then(|v| v.trim().parse().ok()).unwrap_or(0usize)) } };
        while b.len() < head + len { let Ok(n) = s.read(&mut t) else { break }; if n == 0 { break } b.extend_from_slice(&t[..n]) }
        let text = String::from_utf8_lossy(&b).to_string(); let line = text.lines().next().unwrap_or("").to_string();
        let (code, body) = { let mut g = f2.lock().unwrap(); g.hits.push(line.clone()); if line.starts_with("POST") { g.upload = text.clone() } answer_of(&mut g, &line) };
        let body = body.to_string(); let _ = s.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()); });
    (url, f)
}
async fn buy_rig(name: &str, ready_after: usize) -> (super::lost::Rig, Shared, std::path::PathBuf) {
    let r = rig(name, "result", SESSION_OK).await; let (url, f) = fake_server(ready_after);
    set(&r.hub, "order.server", &url).await; set(&r.hub, &format!("buy.{T1}"), "t0").await; tokio::time::sleep(Duration::from_millis(100)).await;
    let d = cwd(&r.ag).await; export(&d);
    (r, f, d)
}
fn export(d: &std::path::Path) { let o = d.join("mfg/1"); std::fs::create_dir_all(&o).unwrap(); std::fs::write(o.join("gerbers.zip"), "ZIPBYTES").unwrap();
    std::fs::write(o.join("manifest.json"), json!({"spec":1,"ship_to":"Germany","parts":[{"name":"Main","qty":5}],"files":{"gerbers.zip":"abc"}}).to_string()).unwrap() }
async fn ask_of(r: &super::lost::Rig, from: usize) -> Value { let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "AskOpened"))).await; ev.into_iter().find(|e| is(e, "AskOpened")).unwrap() }
async fn whole_log(hub: &Arc<Hub>) -> String { hub.store.range(0, usize::MAX).unwrap().join("\n") }
fn posted(ev: &[Value], msg: &str) -> usize { ev.iter().filter(|e| is(e, "MessagePosted") && e["msg"] == msg).count() }

#[tokio::test(flavor = "multi_thread")] async fn nothing_leaves_before_the_persons_send_and_the_token_stays_in_the_record() {
    let (r, f, _d) = buy_rig("or-send", 1).await;
    let from = len(&r.hub).await; let (ok, t) = call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; assert!(ok, "{t}"); assert!(t.contains("End your turn now"), "it never waits on the person: {t}");
    let ask = ask_of(&r, from).await; assert_eq!((ask["kind"].clone(), ask["tool"].clone()), (json!("approval"), json!("order_quote")));
    let d = ask["detail"].as_str().unwrap(); assert!(d.contains("Send 1 file from mfg/1") && d.contains("Ships to Germany") && d.ends_with("gerbers.zip"), "{d}");
    tokio::time::sleep(Duration::from_millis(300)).await; assert!(f.lock().unwrap().hits.is_empty(), "the card is the gate: {:?}", f.lock().unwrap().hits);
    let (_, t) = call(&r.ag, "order_status", json!({})).await; assert!(t.contains("not sent yet"), "{t}");
    let from = len(&r.hub).await; assert_eq!(rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"accept"})).await["ok"], true);
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    assert!(ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("order.wait.{T1}") && e["value"] != ""), "the thread waits on the quote: {ev:?}");
    let up = f.lock().unwrap().upload.clone(); assert!(up.contains("ZIPBYTES") && up.contains("filename=\"gerbers.zip\"") && up.contains("name=\"manifest\""), "{up}"); assert!(up.to_lowercase().contains("x-backplane-install:"));
    let q = ev.iter().find(|e| is(e, "MessagePosted") && e["msg"] == "q:q_fake1").expect("the quote is posted"); assert_eq!(q["role"], "assistant");
    let text = q["text"].as_str().unwrap(); assert!(text.contains("Excludes customs") && text.ends_with("[Buy now · $112.40](order:q_fake1)"), "{text}");
    let note = ev.iter().find(|e| is(e, "MessagePosted") && e["role"] == "system").unwrap()["text"].as_str().unwrap().to_string(); assert!(note.starts_with("[Backplane] The quote is posted in this thread. ") && note.contains("112.4"), "{note}");
    assert!(ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("order.wait.{T1}") && e["value"] == ""), "and waits no more: {ev:?}");
    // the token: in the record and in what is drawn for a client, nowhere an agent or the log can reach
    let rec = r.hub.cfg.home.join("orders/q_fake1.json"); assert!(std::fs::read_to_string(&rec).unwrap().contains(TOKEN)); assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(&rec).unwrap().permissions().mode() & 0o777, 0o600), 0o600);
    assert!(!whole_log(&r.hub).await.contains(TOKEN), "no token in the log");
    let (_, read) = call(&r.ag, "thread_read", json!({"threadId":T1,"limit":200})).await; assert!(read.contains("order:q_fake1") && !read.contains(TOKEN), "no token in thread_read");
    let (_, st) = call(&r.ag, "order_status", json!({})).await; assert!(st.contains("112.4") && !st.contains(TOKEN), "{st}");
    assert!(crate::settings::order::pay_shown(text).contains(TOKEN), "the person's link is real");
    assert!(until(&r.hub, 0, 5, |_| f.lock().unwrap().hits.iter().filter(|h| h.starts_with("GET")).count() >= 4).await.len() > 0, "and it is followed");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn not_now_tells_the_agent_and_sends_nothing() {
    let (r, f, _d) = buy_rig("or-not", 0).await;
    let from = len(&r.hub).await; call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; let ask = ask_of(&r, from).await;
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"decline"})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.starts_with("[Backplane] The person chose not to send the files"))), "{ev:?}"); assert!(f.lock().unwrap().hits.is_empty());
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_files_approval_outlives_a_restart_and_its_send_still_sends() {
    let (r, f, _d) = buy_rig("or-keepask", 0).await;
    let from = len(&r.hub).await; call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; let ask = ask_of(&r, from).await; let id = ask["id"].as_str().unwrap().to_string();
    assert!(r.hub.cfg.home.join(format!("orders/ask-{id}.json")).is_file(), "kept on disk beside the quotes");
    assert!(crate::settings::order::survives_restart("order_quote") && crate::settings::order::survives_restart("order_ask.typed") && !crate::settings::order::survives_restart("Bash"), "Rec.asks leaves it open");
    r.ag.order_asks.lock().unwrap().clear(); // what a restart leaves of it: the card (still open) and the file
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":id,"answer":"accept"})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    assert!(ev.iter().any(|e| is(e, "MessagePosted") && e["msg"] == "q:q_fake1"), "the files went and the quote is posted: {ev:?}"); assert!(f.lock().unwrap().upload.contains("ZIPBYTES"));
    assert!(!r.hub.cfg.home.join(format!("orders/ask-{id}.json")).exists(), "and its record is gone");
    assert!(ev.iter().any(|e| is(e, "ActivityLogged") && e["summary"] == "Sent for a quote: 1 file from mfg/1"), "one line says what was sent: {ev:?}");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_send_whose_list_was_lost_tells_the_agent_to_ask_again() {
    let (r, f, _d) = buy_rig("or-lostask", 0).await;
    let from = len(&r.hub).await; call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; let ask = ask_of(&r, from).await; let id = ask["id"].as_str().unwrap().to_string();
    r.ag.order_asks.lock().unwrap().clear(); std::fs::remove_file(r.hub.cfg.home.join(format!("orders/ask-{id}.json"))).unwrap(); // an older hub's approval
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":id,"answer":"accept"})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("Call order_quote again"))), "{ev:?}"); assert!(f.lock().unwrap().hits.is_empty(), "nothing sent");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_approval_covers_only_what_the_person_saw() {
    let (r, f, d) = buy_rig("or-swap", 0).await;
    let from = len(&r.hub).await; call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; let ask = ask_of(&r, from).await;
    // after the card, before the Send: another file joins the manifest
    std::fs::write(d.join("mfg/1/secret.step"), "x").unwrap(); std::fs::write(d.join("mfg/1/manifest.json"), json!({"ship_to":"Germany","files":{"gerbers.zip":"abc","secret.step":"def"}}).to_string()).unwrap();
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"accept"})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("The files were not taken") && t.contains("changed after the person approved"))), "{ev:?}"); assert!(f.lock().unwrap().hits.is_empty(), "nothing went");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn what_must_not_go_is_refused_before_the_person_is_asked() {
    let (r, _f, d) = buy_rig("or-bad", 0).await; let from = len(&r.hub).await;
    for dir in ["", "/etc", "../..", "mfg/none"] { let (ok, t) = call(&r.ag, "order_quote", json!({"dir":dir})).await; assert!(!ok, "{dir}: {t}") }
    crate::plat::fs::symlink("/etc/hostname", d.join("mfg/1/x.zip")).unwrap(); let (ok, t) = call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; assert!(!ok && t.contains("no links"), "{t}");
    tokio::time::sleep(Duration::from_millis(150)).await; assert!(!log_from(&r.hub, from).await.iter().any(|e| is(e, "AskOpened")));
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn files_the_server_refuses_are_said_and_the_thread_stops_waiting() {
    let (r, f, _d) = buy_rig("or-refuse", 0).await; f.lock().unwrap().refuse = true;
    let from = len(&r.hub).await; call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; let ask = ask_of(&r, from).await;
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"accept"})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("The files were not taken: unknown file type"))), "{ev:?}");
    assert!(ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("order.wait.{T1}") && e["value"] == ""), "{ev:?}"); assert_eq!(r.ag.world().await.asks.len(), 0);
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_busy_or_restarting_server_does_not_end_the_wait_for_a_quote() {
    // the server answers 503 (nginx while it restarts) then 429 (the read budget) to the first polls: the quote is still on its way, not lost
    let (r, f, _d) = buy_rig("or-flaky", 0).await; f.lock().unwrap().flaky = 2;
    let from = len(&r.hub).await; call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; let ask = ask_of(&r, from).await;
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"accept"})).await;
    let ev = until(&r.hub, from, 20, |ev| posted(ev, "q:q_fake1") == 1).await;
    assert!(!ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("could not be worked out"))), "no false failure: {ev:?}");
    assert!(f.lock().unwrap().polls >= 3, "it asked again"); r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_quote_the_server_could_not_work_out_says_why_and_posts_no_link() {
    let (r, f, _d) = buy_rig("or-failed", 0).await; f.lock().unwrap().fails = true;
    let from = len(&r.hub).await; call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; let ask = ask_of(&r, from).await;
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"accept"})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    let note = ev.iter().find(|e| is(e, "MessagePosted") && e["role"] == "system").unwrap()["text"].as_str().unwrap().to_string();
    assert!(note.contains("could not be worked out") && note.contains("try again later") && !note.contains("is posted in this thread") && !note.contains("posted the quote"), "the agent hears why: {note}");
    assert_eq!(posted(&ev, "q:q_fake1"), 0, "nothing to post");
    let v: Value = serde_json::from_str(&r.ag.hub.setting(&format!("order.quote.{T1}")).await.unwrap()).unwrap(); assert_eq!((v["state"].clone(), v["payable"].clone()), (json!("failed"), json!(false)), "the card says it");
    r.ag.shutdown();
}
async fn quoted_rig(name: &str, pilot: bool) -> (super::lost::Rig, Shared) {
    let (r, f, _d) = buy_rig(name, 0).await; f.lock().unwrap().pilot = pilot;
    let from = len(&r.hub).await; call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; let ask = ask_of(&r, from).await;
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"accept"})).await;
    until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await; (r, f)
}
#[tokio::test(flavor = "multi_thread")] async fn the_watcher_says_each_step_once_and_alerts_then_stops_at_delivered() {
    let (r, f) = quoted_rig("or-watch", false).await;
    let step = |st: &str, tracking: &str| f.lock().unwrap().order = json!({"status":st,"tracking":tracking}); let from = len(&r.hub).await;
    step("paid", ""); let ev = until(&r.hub, from, 20, |ev| posted(ev, "o:q_fake1:paid") == 1).await; assert!(ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("alert.{T1}") && e["value"].as_str().unwrap().starts_with("note\to:q_fake1:paid\t")), "a note alert: {ev:?}");
    step("shipped", "TRK1"); let ev = until(&r.hub, from, 20, |ev| posted(ev, "o:q_fake1:shipped") == 1).await; assert!(ev.iter().any(|e| e["text"] == "**Shipped.** Tracking: TRK1"), "{ev:?}");
    step("delivered", "TRK1"); until(&r.hub, from, 20, |ev| posted(ev, "o:q_fake1:delivered") == 1).await;
    let v: Value = serde_json::from_str(&r.ag.hub.setting(&format!("order.quote.{T1}")).await.unwrap()).unwrap(); assert_eq!(v["state"], "delivered"); assert_eq!(v["tracking"], "TRK1");
    tokio::time::sleep(Duration::from_millis(200)).await; let n = f.lock().unwrap().hits.len(); tokio::time::sleep(Duration::from_millis(300)).await; assert_eq!(f.lock().unwrap().hits.len(), n, "nothing more will happen: it stops asking");
    assert_eq!(posted(&log_from(&r.hub, from).await, "o:q_fake1:paid"), 1, "said once");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_pilot_payment_says_so_alerts_no_one_and_is_not_followed() {
    let (r, f) = quoted_rig("or-pilot", true).await; let from = len(&r.hub).await;
    f.lock().unwrap().order = json!({"status":"paid"});
    let ev = until(&r.hub, from, 20, |ev| posted(ev, "o:q_fake1:pilot") == 1).await;
    assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("your card was not charged"))), "{ev:?}"); assert!(!ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("alert.{T1}") && e["value"].as_str().unwrap_or("").starts_with("note")), "alerts no one: {ev:?}");
    assert_eq!(crate::settings::fab::order_status(&r.hub.cfg.home.join("orders"), &[T1.to_string()]), "", "never shows as Ordered");
    tokio::time::sleep(Duration::from_millis(200)).await; let n = f.lock().unwrap().hits.len(); tokio::time::sleep(Duration::from_millis(300)).await; assert_eq!(f.lock().unwrap().hits.len(), n);
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn alerts_for_settled_work_turned_off_silence_the_note_not_the_thread() {
    let (r, f) = quoted_rig("or-quiet", false).await; set(&r.hub, "notify.settled", "off").await; let from = len(&r.hub).await;
    f.lock().unwrap().order = json!({"status":"paid"}); let ev = until(&r.hub, from, 20, |ev| posted(ev, "o:q_fake1:paid") == 1).await;
    assert!(!ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("alert.{T1}") && e["value"].as_str().unwrap_or("").starts_with("note")), "{ev:?}"); r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_restart_goes_on_getting_the_quote_and_posts_it() {
    let r = rig("or-resume", "result", SESSION_OK).await; let (url, f) = fake_server(1);
    set(&r.hub, &format!("buy.{T1}"), "t0").await; let o = r.hub.cfg.home.join("orders"); std::fs::create_dir_all(&o).unwrap();
    std::fs::write(o.join("q_fake1.json"), json!({"id":"q_fake1","token":TOKEN,"thread":T1,"server":url,"status":"quoting"}).to_string()).unwrap();
    let from = len(&r.hub).await; crate::settings::order::resume(r.ag.clone());
    let ev = until(&r.hub, from, 20, |ev| posted(ev, "q:q_fake1") == 1 && ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    assert!(ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("order.wait.{T1}") && e["value"] != ""), "it waits on it again: {ev:?}"); assert!(f.lock().unwrap().hits.iter().all(|h| h.starts_with("GET")), "nothing is sent again");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_restart_fills_the_card_and_goes_on_watching_a_quote_with_no_repeat() {
    let r = rig("or-rewatch", "result", SESSION_OK).await; let (url, f) = fake_server(0); f.lock().unwrap().order = json!({"status":"paid"});
    set(&r.hub, &format!("buy.{T1}"), "t0").await; let o = r.hub.cfg.home.join("orders"); std::fs::create_dir_all(&o).unwrap();
    std::fs::write(o.join("q_fake1.json"), json!({"id":"q_fake1","token":TOKEN,"thread":T1,"server":url,"status":"paid"}).to_string()).unwrap();
    let from = len(&r.hub).await; crate::settings::order::resume(r.ag.clone());
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| is(e, "SettingSet") && e["key"] == format!("order.quote.{T1}"))).await; assert!(!ev.is_empty());
    tokio::time::sleep(Duration::from_millis(250)).await; assert_eq!(posted(&log_from(&r.hub, from).await, "o:q_fake1:paid"), 0, "already said before the restart");
    f.lock().unwrap().order = json!({"status":"shipped"}); until(&r.hub, from, 20, |ev| posted(ev, "o:q_fake1:shipped") == 1).await;
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_step_taken_while_the_hub_was_away_is_said_at_the_restart() {
    let r = rig("or-away", "result", SESSION_OK).await; let (url, f) = fake_server(0); f.lock().unwrap().order = json!({"status":"shipped","tracking":"TRK9"});
    set(&r.hub, &format!("buy.{T1}"), "t0").await; let o = r.hub.cfg.home.join("orders"); std::fs::create_dir_all(&o).unwrap();
    std::fs::write(o.join("q_fake1.json"), json!({"id":"q_fake1","token":TOKEN,"thread":T1,"server":url,"status":"paid"}).to_string()).unwrap();
    let from = len(&r.hub).await; crate::settings::order::resume(r.ag.clone());
    // the watch would look again only after its pause (ten minutes once paid); the restart says it at once, once
    let ev = until(&r.hub, from, 20, |ev| posted(ev, "o:q_fake1:shipped") == 1).await; assert!(ev.iter().any(|e| e["text"] == "**Shipped.** Tracking: TRK9"), "{ev:?}");
    assert!(std::fs::read_to_string(o.join("q_fake1.json")).unwrap().contains("\"status\":\"shipped\""), "the record follows, so the next restart does not say it again");
    tokio::time::sleep(Duration::from_millis(250)).await; assert_eq!(posted(&log_from(&r.hub, from).await, "o:q_fake1:shipped"), 1, "said once");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn the_order_tools_are_refused_with_ordering_off_even_mid_order() {
    let (r, f, _d) = buy_rig("or-offq", 0).await; set(&r.hub, "order.enabled", "off").await; tokio::time::sleep(Duration::from_millis(100)).await;
    for (n, a) in [("order_quote", json!({"dir":"mfg/1"})), ("order_status", json!({})), ("order_ask", json!({"question":"?"}))] { let (ok, t) = call(&r.ag, n, a).await; assert!(!ok && t.contains(crate::settings::order::OFF), "{n}: {t}") }
    assert!(f.lock().unwrap().hits.is_empty()); r.ag.shutdown();
}
/// BACKPLANE_COPY is read once a process, so the copy's half runs in a child (the shape of net/copy_outs.rs).
#[tokio::test(flavor = "multi_thread")] async fn copy_probe() {
    if std::env::var("BP_ORDER_COPY_PROBE").is_err() { return }
    assert!(crate::copy::on());
    let (r, f, d) = buy_rig("or-copy", 0).await;
    let from = len(&r.hub).await; let (ok, t) = call(&r.ag, "order_quote", json!({"dir":"mfg/1"})).await; assert!(!ok && t.contains("sends nothing"), "{t}");
    let (ok, t) = call(&r.ag, "order_review", json!({"action":"record","verdict":"ok"})).await; assert!(!ok, "{t}"); assert!(!d.join(".backplane/review.json").exists(), "nothing is written into the project");
    let (ok, _) = call(&r.ag, "order_review", json!({"action":"check"})).await; assert!(ok, "reading stays");
    let o = r.hub.cfg.home.join("orders"); std::fs::create_dir_all(&o).unwrap(); let url = r.ag.hub.setting("order.server").await.unwrap();
    std::fs::write(o.join("q_fake1.json"), json!({"id":"q_fake1","token":TOKEN,"thread":T1,"server":url,"status":"ready"}).to_string()).unwrap();
    crate::settings::order::resume(r.ag.clone()); let (_, st) = call(&r.ag, "order_status", json!({})).await; assert!(st.contains("sends nothing"), "{st}");
    assert!(crate::copy::rpc_refused("order.buy"));
    tokio::time::sleep(Duration::from_millis(400)).await; assert!(f.lock().unwrap().hits.is_empty(), "nothing reached the server: {:?}", f.lock().unwrap().hits); assert!(!log_from(&r.hub, from).await.iter().any(|e| is(e, "AskOpened")));
    r.ag.shutdown();
}
#[test] fn a_copy_sends_nothing_to_the_order_server() {
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, std::env::current_exe().unwrap());
    c.args(["agents::tests::order::copy_probe", "--exact", "--nocapture", "--test-threads=1"]).env("BP_ORDER_COPY_PROBE", "1").env("BACKPLANE_COPY", "1");
    let o = c.output().unwrap(); let out = String::from_utf8_lossy(&o.stdout); assert!(o.status.success() && out.contains("copy_probe ... ok"), "{out}{}", String::from_utf8_lossy(&o.stderr));
}

/// The whole way against the real order server (test/order/local_server.py: order/fabsrv with its stand-in fab pack and a Stripe double), the one thing a fake cannot show:
/// that it takes what this hub sends. Runs only with BP_ORDER_LOCAL=<its address>, BP_ORDER_EXPORT=<an --export folder> and BP_ORDER_PILOT=1 or 0; never another address.
#[tokio::test(flavor = "multi_thread")] async fn the_real_order_server_takes_it_and_a_test_payment_comes_back() {
    let Ok(url) = std::env::var("BP_ORDER_LOCAL") else { return }; assert!(url.starts_with("http://127.0.0.1:"), "a local order server only: {url}");
    let pilot = std::env::var("BP_ORDER_PILOT").as_deref() == Ok("1");
    let r = rig("or-real", "result", SESSION_OK).await; set(&r.hub, "order.server", &url).await; set(&r.hub, &format!("buy.{T1}"), "t0").await; tokio::time::sleep(Duration::from_millis(100)).await;
    let d = cwd(&r.ag).await; let o = d.join("mfg/real"); std::fs::create_dir_all(o.join("main")).unwrap(); let src = std::path::PathBuf::from(std::env::var("BP_ORDER_EXPORT").unwrap());
    for f in ["manifest.json", "main/gerbers.zip"] { std::fs::copy(src.join(f), o.join(f)).unwrap(); }
    let from = len(&r.hub).await; let (ok, t) = call(&r.ag, "order_quote", json!({"dir":"mfg/real"})).await; assert!(ok, "{t}"); let ask = ask_of(&r, from).await;
    let from = len(&r.hub).await; rpc(&r.hub, "ask.answer", json!({"id":ask["id"],"answer":"accept"})).await;
    let ev = until(&r.hub, from, 60, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["role"] == "system")).await;
    let note = ev.iter().find(|e| is(e, "MessagePosted") && e["role"] == "system").expect("a note").clone(); println!("NOTE {note}");
    let q = ev.iter().find(|e| is(e, "MessagePosted") && e["msg"].as_str().map_or(false, |m| m.starts_with("q:"))).unwrap_or_else(|| panic!("no quote posted: {note}")).clone(); println!("QUOTE {}", q["text"]);
    let text = q["text"].as_str().unwrap(); assert!(text.contains("](order:q_"), "{text}");
    let sum: Value = serde_json::from_str(&r.ag.hub.setting(&format!("order.quote.{T1}")).await.unwrap()).unwrap(); println!("SUM {sum}"); assert_eq!(sum["payable"], true, "{sum}");
    let qid = sum["id"].as_str().unwrap().to_string(); let tok = serde_json::from_str::<Value>(&std::fs::read_to_string(r.hub.cfg.home.join(format!("orders/{qid}.json"))).unwrap()).unwrap()["token"].as_str().unwrap().to_string();
    assert!(tok.len() >= 32); assert!(!whole_log(&r.hub).await.contains(&tok), "no token in the log");
    // Buy now: the link the person is shown goes to the double's checkout, and its test card pays
    let link = crate::settings::order::pay_link(&qid).unwrap(); assert!(link.starts_with(&url) && link.contains(&tok));
    let o = std::process::Command::new("curl").args(["-sS", "-L", "-o", "/dev/null", "-w", "%{http_code} %{url_effective}", &link]).output().unwrap(); let said = String::from_utf8_lossy(&o.stdout).to_string(); println!("PAID {}", said.replace(&tok, "<token>")); assert!(said.starts_with("200 "), "{said}");
    let from = len(&r.hub).await;
    if pilot {
        let ev = until(&r.hub, from, 30, |ev| posted(ev, &format!("o:{qid}:pilot")) == 1).await; assert!(ev.iter().any(|e| e["text"].as_str().map_or(false, |t| t.contains("your card was not charged"))), "{ev:?}");
        assert!(!ev.iter().any(|e| e["key"] == format!("alert.{T1}") && e["value"].as_str().unwrap_or("").starts_with("note")), "{ev:?}");
    } else {
        let ev = until(&r.hub, from, 30, |ev| posted(ev, &format!("o:{qid}:paid")) == 1).await; assert!(ev.iter().any(|e| e["key"] == format!("alert.{T1}") && e["value"].as_str().unwrap_or("").starts_with("note")), "{ev:?}");
    }
    r.ag.shutdown();
}

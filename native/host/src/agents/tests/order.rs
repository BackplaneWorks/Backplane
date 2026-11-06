//! The order tools through the agent's own MCP path: what an agent asked to review for manufacturing calls, and what a buy thread's questions do.
use super::*;
use super::lost::rig;

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

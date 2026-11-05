//! The core MCP loop a Claude thread drives, with a stand-in agent (its turns echo "re: <text>"): launch a subthread, send it a message,
//! read the reply, receive its reports, and wait on it with wake_on. Same hub, same tool entry points the MCP server calls.
use super::*;
use serde_json::json;

fn tool(ag: &Arc<Agents>, name: &str) -> Arc<dyn mcp::ToolSet> { ag.tool_sets().into_iter().find(|s| s.owns(name)).expect("a tool set serves it") }
fn reports(ev: &[Value], to: &str) -> Vec<String> { ev.iter().filter(|e| is(e, "MessagePosted") && e["thread"] == to && e["msg"].as_str().map_or(false, |m| m.contains(":lr-"))).map(|e| e["text"].as_str().unwrap_or("").to_string()).collect() }

#[tokio::test(flavor = "multi_thread")] async fn launch_send_read_and_the_reports_come_back() {
    let (hub, ag, _sd) = codex_start("ce1").await; let from = len(&hub).await;
    let (ok, r) = ag.tool_launch(T1, &json!({"prompt":"first job","provider":"codex"})).await; assert!(ok, "{r}"); let child = r["threadId"].as_str().unwrap().to_string();
    let ev = until(&hub, from, 30, |ev| !reports(ev, T1).is_empty()).await;
    assert!(reports(&ev, T1)[0].contains("completed:\n\nre: first job"), "{:?}", reports(&ev, T1));
    let (ok, r) = ag.tool_send(T1, &json!({"threadId":child,"text":"second job"})).await; assert!(ok, "{r}");
    let ev = until(&hub, from, 30, |ev| reports(ev, T1).len() >= 2).await;
    assert!(reports(&ev, T1)[1].contains("completed:\n\nre: second job"), "{:?}", reports(&ev, T1));
    let (ok, r) = ag.tool_read(T1, &json!({"threadId":child})).await; assert!(ok, "{r}");
    let texts: Vec<String> = r["entries"].as_array().unwrap().iter().filter_map(|e| e["text"].as_str().map(String::from)).collect();
    assert!(texts.iter().any(|t| t.trim() == "re: first job") && texts.iter().any(|t| t.trim() == "re: second job"), "{texts:?}");
    let (ok, r) = ag.tool_list(T1).await; assert!(ok && r.to_string().contains(&child), "{r}"); ag.shutdown();
}

#[tokio::test(flavor = "multi_thread")] async fn wake_on_a_subthread_wakes_its_watcher_once() {
    let (hub, ag, _sd) = codex_start("ce2").await; let from = len(&hub).await;
    let (ok, r) = ag.tool_launch(T1, &json!({"prompt":"watched job","provider":"codex","report":false})).await; assert!(ok, "{r}"); let child = r["threadId"].as_str().unwrap().to_string();
    let ts = tool(&ag, "wake_on");
    let (ok, w) = ts.call(T1, "wake_on", &json!({"kind":"thread","target":child,"events":["done"],"note":"check it"})).await; assert!(ok, "{w}"); let id = w["id"].as_str().unwrap().to_string();
    let (ok, again) = ts.call(T1, "wake_on", &json!({"kind":"thread","target":child,"events":["done"]})).await; assert!(ok && again["existing"] == true && again["id"] == id.as_str(), "{again}");
    assert!(!ts.call(T1, "wake_on", &json!({"kind":"thread","target":T1})).await.0, "watching yourself is refused");
    assert!(!ts.call(T1, "wake_on", &json!({"kind":"thread","target":"nope"})).await.0);
    let ev = until(&hub, from, 30, |ev| ev.iter().any(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["text"].as_str().map_or(false, |t| t.starts_with("Wake ")))).await;
    let m = ev.iter().find(|e| is(e, "MessagePosted") && e["thread"] == T1 && e["text"].as_str().map_or(false, |t| t.starts_with("Wake "))).expect("the wake message");
    let t = m["text"].as_str().unwrap(); assert!(t.starts_with(&format!("Wake {id}: thread ")) && t.contains("ended its turn (completed). Its last reply: re: watched job") && t.contains("Events: done.") && t.contains("Note: check it."), "{t}");
    let (ok, l) = ts.call(T1, "wake_list", &json!({})).await; assert!(ok && l["wakes"].as_array().unwrap().is_empty(), "a once-wake drops after its message: {l}");
    // a recurring wake stays and can be cancelled; another thread cannot cancel it
    let (_, w2) = ts.call(T1, "wake_on", &json!({"kind":"thread","target":child,"events":["end"],"once":false})).await; let id2 = w2["id"].as_str().unwrap().to_string();
    assert!(!ts.call("someone-else", "wake_cancel", &json!({"id":id2})).await.0);
    assert_eq!(ts.call(T1, "wake_cancel", &json!({"id":id2})).await.1["cancelled"], true);
    assert!(ts.call(T1, "wake_list", &json!({})).await.1["wakes"].as_array().unwrap().is_empty()); ag.shutdown();
}

// ---- a hub on a copy of a log: BACKPLANE_COPY=1 is read once per process, so the probe runs in a child ----
#[tokio::test(flavor = "multi_thread")] async fn wake_copy_child_probe() {
    if std::env::var("BP_WAKE_COPY_PROBE").is_err() { return }
    assert!(crate::copy::on());
    let sd = dir("wkc-bin"); perf_standin(&sd, 1); const T2: &str = "t1700001001-1";
    let (hub, ag, _h, _w) = start("wkc", |w| base_log(w, 2, &format!("{{\"$\":\"WakeSet\",\"id\":\"w-open\",\"by\":\"{T1}\",\"kind\":\"thread\",\"target\":\"{T2}\",\"events\":[\"end\",\"ask\"],\"once\":false,\"note\":\"\",\"at\":1700000005}}\n")), |_| Cli::standin(&sd)).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    hub.commit_many(&[json!({"$":"TurnChanged","thread":T2,"state":"running","at":1700000010}), json!({"$":"AskOpened","id":"q1","thread":T2,"kind":"question","tool":"AskUserQuestion","detail":"which?","options":"","at":1700000011}), json!({"$":"TurnChanged","thread":T2,"state":"completed","at":1700000012})]).await.unwrap();
    let n = len(&hub).await; tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(len(&hub).await, n, "nothing is written on a copy: no wake message, no turn, no drop");
    assert!(!log_from(&hub, 0).await.iter().any(|e| e["thread"] == T1 && is(e, "MessagePosted")), "the subscriber was told nothing");
    let ts = tool(&ag, "wake_on");
    let (ok, l) = ts.call(T1, "wake_list", &json!({})).await; assert!(ok && l["wakes"].as_array().unwrap().len() == 1, "{l}");
    for (name, a) in [("wake_on", json!({"kind":"thread","target":T2})), ("wake_cancel", json!({"id":"w-open"}))] { let (ok, r) = ts.call(T1, name, &a).await; assert!(!ok && r["error"].as_str().unwrap().contains("off on a copy"), "{name}: {r}"); }
    assert_eq!(len(&hub).await, n, "a refused tool writes nothing"); ag.shutdown();
}
#[test] fn a_hub_on_a_copy_wakes_nobody_and_the_writing_tools_are_refused() {
    let me = std::env::current_exe().unwrap();
    let o = crate::proc::std_cmd(crate::proc::Kind::Local, me).args(["agents::tests::core_e2e::wake_copy_child_probe", "--exact", "--nocapture", "--test-threads=1"]).env("BACKPLANE_COPY", "1").env("BP_WAKE_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)); assert!(String::from_utf8_lossy(&o.stdout).contains("wake_copy_child_probe ... ok"));
}

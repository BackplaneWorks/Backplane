//! Two throwaway hubs over loopback: A owns a thread "t" and answers /far/rpc; B is the machine whose client asks about "<link>~t". No real peers.
use super::*;
use crate::api::{Caller, RpcHandler};
use crate::net::peering::tests::{know, link_by_code, live_ids, node_extra, settle, url, Extra, Node};
use crate::net::{botnet, farstore::FarStore, feed::Feed};
use std::sync::Mutex;

/// A side records what it was asked (params as its handler saw them) and answers with a thread.
struct Rec(&'static [&'static str], Mutex<Vec<(String, Value)>>);
impl Rec { fn new(ms: &'static [&'static str]) -> Arc<Rec> { Arc::new(Rec(ms, Default::default())) } fn seen(&self) -> Vec<(String, Value)> { self.1.lock().unwrap().clone() } }
impl RpcHandler for Rec {
    fn methods(&self) -> &'static [&'static str] { self.0 }
    fn call<'a>(&'a self, _h: &'a Arc<Hub>, _w: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move { self.1.lock().unwrap().push((m.to_string(), p.clone())); vec![json!({"t":"reply","id":id,"ok":true,"result":{"thread":p["thread"]}, "thread": p["thread"]})] })
    }
}
const ALL: &[&str] = &["thread.rename", "thread.delete", "project.remove", "fs.list", "turn.start", "setting.set"];
/// A hub with a FarSync, its /far/rpc served (after `hang`, a peer that never answers in time), and a router installed.
async fn side(tag: &str, with_feed: bool, hang: bool) -> (Node, Arc<FarSync>) { side_with(tag, with_feed, hang, false).await }
async fn side_with(tag: &str, with_feed: bool, hang: bool, copy: bool) -> (Node, Arc<FarSync>) {
    let slot: Arc<Mutex<Option<Arc<FarSync>>>> = Arc::new(Mutex::new(None)); let s2 = slot.clone();
    let extra: Extra = Arc::new(move |hub, req| { let s = s2.lock().unwrap().clone(); Box::pin(async move {
        let s = s?; if req.route() != "/far/rpc" || req.method != "POST" { return None }
        if hang { tokio::time::sleep(std::time::Duration::from_secs(30)).await }
        Some(serve_with(false, &s, &hub, &req).await) }) });
    let n = node_extra(tag, false, false, Some(extra)).await;
    if with_feed { n.hub.commit(&json!({"$":"ThreadCreated","id":"t","project":"","title":"T","env":"","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap(); }
    let fs = FarSync::new(FarStore::open(&n.home).unwrap(), n.p.clone());
    if with_feed { let feed = Feed::new(); feed.start(&n.hub).await; let _ = fs.feed.set(feed); }
    n.hub.set_far_router(Arc::new(Router::with_copy(fs.clone(), copy))); // (a router is installed once: a copy is chosen here)
    *slot.lock().unwrap() = Some(fs.clone());
    (n, fs)
}
async fn link(a: &Node, b: &Node, own: bool) -> String {
    know(a, b); know(b, a);
    let id = link_by_code(b, a).await; assert_eq!(live_ids(b), vec![id.clone()]);
    if !own { for n in [a, b] { n.hub.commit(&json!({"$":"SettingSet","key":format!("peer.own.{id}"),"value":"","at":3})).await.unwrap(); } settle().await; }
    id
}
/// A client of `b` asks; the reply is the frame returned or, for a request sent on, the one pushed to its connection.
async fn ask(b: &Node, id: u64, m: &str, p: Value) -> Value {
    let client = b.hub.fresh_client(); let mut ob = b.hub.attach(client);
    let mut c = Conn { sent: vec![], client, gen: "t".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    if let Some(v) = b.hub.request(&mut c, &json!({"id": id, "m": m, "p": p})).await.into_iter().find(|v| v["t"] == "reply") { return v }
    match tokio::time::timeout(std::time::Duration::from_secs(10), ob.recv()).await { Ok(Some(crate::hub::Out::Value(v))) => v, o => panic!("no reply (timeout or other frame): {}", o.is_ok()) }
}
async fn signed(from: &Node, to: &Node, link: &str, body: Value) -> botnet::Resp {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    botnet::to_peer(&from.home, &url(to), link, now, "/far/rpc", true, &body.to_string()).await
}

#[tokio::test] async fn far_route_goes_home_and_the_reply_comes_back_as_ours() {
    let (a, _fa) = side("ra1", true, false).await; let (b, _fb) = side("rb1", false, false).await;
    let rec = Rec::new(ALL); a.hub.register_rpc(rec.clone());
    let l = link(&a, &b, true).await;
    let r = ask(&b, 7, "thread.rename", json!({"thread": format!("{l}~t"), "title": "New"})).await;
    assert_eq!(r["ok"], true, "{r}"); assert_eq!(r["id"], 7, "the caller's request id");
    assert_eq!(r["thread"], format!("{l}~t"), "the thread a reply names is the id here");
    let got = rec.seen();
    assert_eq!(got.len(), 1, "{got:?}"); assert_eq!(got[0].1["thread"], "t", "the owner takes its own ids"); assert_eq!(got[0].1["title"], "New");
}
#[tokio::test] async fn far_route_here_stays() {
    // the law's case: turn.start on thread t1 with a text that looks like an id elsewhere (`a~b`) is not about a thing elsewhere: it runs here, params as sent
    let (b, _fb) = side("rb2", false, false).await;
    let rec = Rec::new(ALL); b.hub.register_rpc(rec.clone());
    let p = json!({"thread": "t1", "text": "a~b"});
    let r = ask(&b, 3, "turn.start", p.clone()).await;
    assert_eq!(r["ok"], true, "{r}"); assert_eq!(r["id"], 3);
    assert_eq!(rec.seen(), vec![("turn.start".to_string(), p)], "a local handler ran with the params unchanged");
}
#[tokio::test] async fn far_allowed_only_methods_and_far_no_delete() {
    let (a, _fa) = side("ra3", true, false).await; let (b, _fb) = side("rb3", false, false).await;
    let rec = Rec::new(ALL); a.hub.register_rpc(rec.clone());
    let l = link(&a, &b, true).await;
    // the asker refuses what Fr.methods lacks, and nothing is sent
    let r = ask(&b, 1, "thread.delete", json!({"thread": format!("{l}~t")})).await;
    assert_eq!(r["ok"], false); assert_eq!(r["error"], "that is done on its own machine");
    assert!(rec.seen().is_empty(), "never sent: a linked machine cannot delete");
    // the owner's own check, straight from an own machine's signed request (the asker's refusal is not what stops it)
    for (m, p) in [("setting.set", json!({"key":"k","value":"v"})), ("thread.delete", json!({"thread":"t"}))] {
        let r = signed(&b, &a, &l, json!({"id":0,"m":m,"p":p})).await;
        assert_eq!(r.code, 403, "{m}: {}", r.body);
    }
    // a thread it does not have, and a project that is not its own
    let r = ask(&b, 2, "thread.rename", json!({"thread": format!("{l}~nope"), "title": "x"})).await;
    assert_eq!(r["ok"], false); assert!(r["error"].as_str().unwrap().starts_with("that machine did not take it"), "{r}");
    assert_eq!(signed(&b, &a, &l, json!({"id":0,"m":"project.remove","p":{"project":"p"}})).await.code, 403);
    assert!(rec.seen().is_empty(), "nothing ran on the owner");
}
#[tokio::test] async fn far_a_request_run_for_a_linked_machine_is_never_sent_onward() {
    // both hubs route; `on` is not renamed on the way, so a router on the owner would send fs.list {on:L} straight back
    let (a, _fa) = side("ra6", true, false).await; let (b, _fb) = side("rb6", false, false).await;
    let (ra, rb) = (Rec::new(ALL), Rec::new(ALL)); a.hub.register_rpc(ra.clone()); b.hub.register_rpc(rb.clone());
    let l = link(&a, &b, true).await;
    let r = ask(&b, 5, "fs.list", json!({"on": l, "path": "/"})).await;
    assert_eq!(r["ok"], true, "{r}"); assert_eq!(r["id"], 5);
    assert_eq!(ra.seen().len(), 1, "it ran once on the owner"); assert!(rb.seen().is_empty(), "and never on the asker");
    // an own machine's request that names an id elsewhere is run here or refused, never relayed
    let r = signed(&b, &a, &l, json!({"id":0,"m":"thread.rename","p":{"thread":"t","project":"x~p","title":"y"}})).await;
    assert_eq!(r.code, 200, "{}", r.body);
    assert_eq!(ra.seen().len(), 2); assert_eq!(ra.seen()[1].1["project"], "x~p", "run here with the params as they came");
    assert!(rb.seen().is_empty());
}
#[tokio::test] async fn far_a_request_to_a_machine_that_does_not_answer_holds_no_connection() {
    let (a, _fa) = side("ra7", true, true).await; let (b, _fb) = side("rb7", false, false).await;
    let l = link(&a, &b, true).await;
    let client = b.hub.fresh_client(); let _ob = b.hub.attach(client);
    let mut c = Conn { sent: vec![], client, gen: "t".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    let t = std::time::Instant::now();
    let out = b.hub.request(&mut c, &json!({"id": 1, "m": "thread.rename", "p": {"thread": format!("{l}~t"), "title": "x"}})).await;
    assert!(out.is_empty() && t.elapsed() < std::time::Duration::from_secs(2), "sent on, nothing to wait for: {out:?} {:?}", t.elapsed());
    // the same connection's next request is answered at once
    let out = b.hub.request(&mut c, &json!({"id": 2, "m": "nope.nothing", "p": {}})).await;
    assert_eq!(out.len(), 1); assert_eq!(out[0]["id"], 2); assert!(t.elapsed() < std::time::Duration::from_secs(3));
    // and only so many wait at once
    let mut last = Value::Null;
    for i in 10..30u64 { let o = b.hub.request(&mut c, &json!({"id": i, "m": "thread.rename", "p": {"thread": format!("{l}~t"), "title": "x"}})).await; if let Some(v) = o.into_iter().next() { last = v; break } }
    assert_eq!(last["error"], "too many requests to other machines are waiting");
}
#[tokio::test] async fn far_rpc_is_for_the_owners_own_signed_machines_only() {
    let (a, _fa) = side("ra4", true, false).await; let (b, _fb) = side("rb4", false, false).await;
    let l = link(&a, &b, false).await; // linked, but not the owner's
    let body = json!({"id":0,"m":"thread.rename","p":{"thread":"t","title":"x"}});
    let un = botnet::plain(&format!("{}/far/rpc", url(&a))).await; assert_eq!(un.code, 401, "{}", un.body);
    let r = signed(&b, &a, &l, body).await;
    assert_eq!(r.code, 403, "{}", r.body);
}
#[tokio::test] async fn far_a_hub_on_a_copy_takes_no_far_rpc() {
    let (a, fa) = side("ra8", true, false).await;
    let p = PReq { method: "POST".into(), target: "/far/rpc".into(), headers: vec![], body: b"{}".to_vec(), trusted: false, own_device: false , peer: String::new()};
    let r = serve_with(true, &fa, &a.hub, &p).await;
    assert_eq!(r.status, "403 Forbidden"); assert_eq!(String::from_utf8_lossy(&r.body), "a hub on a copied log does not take /far/rpc");
    // (not left to the missing secret: the same request without the gate is refused for another reason)
    assert_eq!(serve_with(false, &fa, &a.hub, &p).await.status, "401 Unauthorized");
}
#[tokio::test] async fn far_owner_link_posts_only_to_an_own_machine_and_reaches_owner_mail() {
    let (a, _fa) = side("ra5", true, false).await; let (b, fb) = side("rb5", false, false).await;
    let l = link(&a, &b, false).await;
    let lk = Link { sync: fb.clone(), hub: Arc::downgrade(&b.hub), copy: false };
    assert_eq!(lk.post(&l, json!({"m":"owner.mail","p":{}}).to_string()).await, None, "not the owner's machine: nothing is sent");
    for n in [&a, &b] { n.hub.commit(&json!({"$":"SettingSet","key":format!("peer.own.{l}"),"value":"approved","at":1})).await.unwrap(); }
    // (A has no bots here: the request reached its owner.mail branch, which says so)
    let r = lk.post(&l, json!({"m":"owner.mail","p":{}}).to_string()).await.expect("sent");
    assert_eq!(r, (503, "bots are not running".to_string()), "signed, owner-only, and it reached the owner.mail branch");
}
#[test] fn far_params_name_the_machine_and_come_back_local() {
    let p = json!({"thread":"m1~t7","title":"x","project":"m1~p","other":"m1~z"});
    assert_eq!(far::params_link(&p), "m1");
    assert_eq!(far::params_local(&p, "m1"), json!({"thread":"t7","title":"x","project":"p","other":"m1~z"}), "only the naming keys are renamed");
    assert_eq!(far::params_link(&json!({"on":"box","thread":"t"})), "box");
    assert_eq!(far::params_link(&json!({"thread":"t","on":"m1~x"})), "");
    assert_eq!(far::params_link(&json!({"thread":"t"})), "");
    assert_eq!(far::reply_on(json!({"t":"reply","id":0,"ok":true,"thread":"n9"}), &json!(5), "m1"), json!({"t":"reply","id":5,"ok":true,"thread":"m1~n9"}));
    assert_eq!(far::reply_on(json!({"t":"reply","id":0,"ok":true,"result":{"diff":"x","thread":"n9"}}), &json!(5), "m1")["result"]["thread"], "m1~n9", "a Diff of a thread there names it as this hub shows it");
    assert!(far::may("thread.rename") && !far::may("thread.delete") && !far::may("turn.stop"));
}
#[tokio::test] async fn far_a_hub_on_a_copy_reaches_no_other_machine() {
    let (a, _fa) = side("ra9", true, false).await; let (b, fb) = side_with("rb9", false, false, true).await;
    let rec = Rec::new(ALL); a.hub.register_rpc(rec.clone());
    let l = link(&a, &b, true).await;
    // B is a copy: a request about a thing elsewhere is answered with an error here, at once, and nothing reaches A
    let t = std::time::Instant::now();
    let client = b.hub.fresh_client(); let _ob = b.hub.attach(client);
    let mut c = Conn { sent: vec![], client, gen: "t".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    let out = b.hub.request(&mut c, &json!({"id": 4, "m": "thread.rename", "p": {"thread": format!("{l}~t"), "title": "x"}})).await;
    assert_eq!(out.len(), 1); assert_eq!(out[0]["ok"], false); assert_eq!(out[0]["id"], 4);
    assert_eq!(out[0]["error"], "a hub on a copied log does not reach other machines");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(rec.seen().is_empty() && t.elapsed() < std::time::Duration::from_secs(3), "no connection was made");
    // and the owner.mail transport sends nothing either, even to an own machine
    let lk = Link { sync: fb, hub: Arc::downgrade(&b.hub), copy: true };
    assert_eq!(lk.post(&l, json!({"m":"owner.mail","p":{}}).to_string()).await, None);
}

/// A side's agents, as far as /far/rpc sees them: records what it was asked to run and answers
struct Srv(Mutex<Vec<(String, String, String, Value)>>);
impl crate::api::FarMcp for Srv {
    fn serve<'a>(&'a self, link: &'a str, from: &'a str, tool: &'a str, args: &'a Value) -> Fut<'a, (bool, Value)> {
        Box::pin(async move { self.0.lock().unwrap().push((link.into(), from.into(), tool.into(), args.clone())); (true, json!({"threadId":"t9","state":"started"})) })
    }
}
#[tokio::test] async fn far_mcp_a_tool_call_for_a_thread_runs_on_the_other_machine_for_its_caller_and_only_over_an_own_link() {
    let (a, _fa) = side("rm1", true, false).await; let (b, _fb) = side("rm2", false, false).await;
    let srv = Arc::new(Srv(Default::default())); a.hub.set_far_mcp(srv.clone());
    let l = link(&a, &b, false).await; // linked, not the owner's: nothing is asked and nothing is taken
    assert_eq!(b.hub.far_machines(), vec![], "a link that is not the owner's is no machine");
    assert!(b.hub.far_call(&l, "tb", "thread_launch", &json!({"prompt":"x"})).await.unwrap_err().contains("no machine of yours is linked"));
    let r = signed(&b, &a, &l, json!({"id":0,"m":"mcp.call","p":{"tool":"thread_launch","from":"tb","args":{"prompt":"x"}}})).await; assert_eq!(r.code, 403, "{}", r.body);
    let r = signed(&b, &a, &l, json!({"id":0,"m":"bots.room_post","p":{"room":"r","msg":"m","from":"x","text":"y"}})).await; assert_eq!(r.code, 403, "{}", r.body);
    assert!(srv.0.lock().unwrap().is_empty());
    // the owner's own: B's thread tb launches on A; A runs it for "<link>~tb"
    for n in [&a, &b] { n.hub.commit(&json!({"$":"SettingSet","key":format!("peer.own.{l}"),"value":"pair","at":1})).await.unwrap(); }
    assert_eq!(b.hub.far_machines().len(), 1);
    let (ok, body) = b.hub.far_call(&l, "tb", "thread_launch", &json!({"prompt":"build it","project":"p1"})).await.unwrap();
    assert!(ok); assert_eq!(body["threadId"], "t9");
    assert_eq!(srv.0.lock().unwrap().clone(), vec![(l.clone(), format!("{l}~tb"), "thread_launch".to_string(), json!({"prompt":"build it","project":"p1"}))]);
    // only the tools the owner's machines may call, and only on behalf of a thread there (not one that is itself elsewhere)
    let r = signed(&b, &a, &l, json!({"id":0,"m":"mcp.call","p":{"tool":"secret_run","from":"tb","args":{}}})).await; assert_eq!(r.code, 403, "{}", r.body);
    let r = signed(&b, &a, &l, json!({"id":0,"m":"mcp.call","p":{"tool":"thread_send","from":"x~tb","args":{}}})).await; assert_eq!(r.code, 400, "{}", r.body);
    assert_eq!(srv.0.lock().unwrap().len(), 1);
}

/// The owner's viewer method: it pushes a frame to whoever asked (as the plots do), and answers.
struct Viewer(Mutex<Vec<(String, u64)>>);
impl RpcHandler for Viewer {
    fn methods(&self) -> &'static [&'static str] { &["kicad.watch", "kicad.info"] }
    fn call<'a>(&'a self, h: &'a Arc<Hub>, w: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            self.0.lock().unwrap().push((m.to_string(), w.client));
            if m == "kicad.watch" && p["kind"] != "" { h.push(w.client, json!({"t":"plot","key":"0|/r|","none":"drawn on the owner"})); h.push_raw(w.client, vec![9, 8, 7]); }
            vec![json!({"t":"reply","id":id,"ok":true,"result":{"keys":["0|/r|"],"root":p["root"],"on":p["on"]}})]
        })
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn far_a_window_watches_a_project_of_another_machine_and_the_frames_it_draws_come_here() {
    let (a, _fa) = side("va1", false, false).await; let (b, _fb) = side("vb1", false, false).await;
    let v = Arc::new(Viewer(Default::default())); a.hub.register_rpc(v.clone());
    let l = link(&a, &b, true).await;
    let client = b.hub.fresh_client(); let mut ob = b.hub.attach(client);
    let mut c = Conn { sent: vec![], client, gen: "t".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    // the watch names the machine; the reply is the owner's, and what the owner draws arrives as pushes to this window
    let sent = b.hub.request(&mut c, &json!({"id": 5, "m": "kicad.watch", "p": {"kind":"board","root":"/r","path":"","tree":"","on": l}})).await;
    assert!(sent.is_empty(), "the answer comes later: the connection never waits on another machine");
    let (mut reply, mut plot, mut raw) = (None, None, None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline && (reply.is_none() || plot.is_none() || raw.is_none()) {
        match tokio::time::timeout(std::time::Duration::from_secs(3), ob.recv()).await {
            Ok(Some(crate::hub::Out::Value(f))) => if f["t"] == "reply" { reply = Some(f) } else if f["t"] == "plot" { plot = Some(f) },
            Ok(Some(crate::hub::Out::Raw(r))) => raw = Some(r), _ => {} }
    }
    let r = reply.expect("a reply"); assert_eq!((r["id"].clone(), r["ok"].clone(), r["result"]["root"].clone()), (json!(5), json!(true), json!("/r")));
    assert!(r["result"]["on"].is_null(), "the owner is not told to send it on: {r}");
    assert_eq!(plot.expect("the owner's plot frame")["none"], "drawn on the owner"); assert_eq!(raw.expect("a binary frame as it was"), vec![9, 8, 7]);
    // a read about a piece names only the plot's key: it goes where the watch is
    let sent = b.hub.request(&mut c, &json!({"id": 6, "m": "kicad.info", "p": {"key":"0|/r|","at":1,"idx":2}})).await;
    assert!(sent.is_empty(), "{sent:?}");
    let got = loop { match tokio::time::timeout(std::time::Duration::from_secs(5), ob.recv()).await { Ok(Some(crate::hub::Out::Value(f))) if f["id"] == 6 => break f, Ok(Some(_)) => {}, _ => panic!("no answer to the read") } };
    assert_eq!(got["ok"], true);
    let seen = v.0.lock().unwrap().clone(); assert_eq!(seen.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["kicad.watch", "kicad.info"], "both ran on the owner");
    assert_eq!(seen[0].1, seen[1].1, "for the same window of its own");
    // the owner takes viewer reads only from its own machines, and only the viewers'
    let bad = signed(&b, &a, &l, json!({"id":0,"m":"viewer.call","p":{"sub":1,"m":"turn.start","p":{}}})).await; assert_eq!(bad.code, 200);
    assert_eq!(serde_json::from_str::<Value>(&bad.body).unwrap()["ok"], false, "{}", bad.body);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn far_a_drawing_an_agent_shows_is_shown_in_the_windows_of_the_other_machine() {
    let (a, _fa) = side("va2", false, false).await; let (b, _fb) = side("vb2", false, false).await;
    let l = link(&a, &b, true).await;
    let client = b.hub.fresh_client(); let mut ob = b.hub.attach(client);
    a.hub.far_announce("viewer.show", &json!({"thread": "t9", "path": "/r/x.kicad_pcb", "kind": "board"})).await;
    let f = loop { match tokio::time::timeout(std::time::Duration::from_secs(5), ob.recv()).await { Ok(Some(crate::hub::Out::Value(f))) if f["t"] == "view" => break f, Ok(Some(_)) => {}, _ => panic!("nothing shown") } };
    assert_eq!((f["thread"].clone(), f["kind"].clone(), f["path"].clone()), (json!(format!("{l}~t9")), json!("board"), json!("/r/x.kicad_pcb")), "the thread is the owner's, named through the link");
}

/// A link that moved (or whose stored address was never reachable) says where it is, signed by the link's own secret; the other machine keeps the new address only
/// when the link is approved and the address answers.
#[tokio::test] async fn a_link_that_moved_says_so_signed_and_the_other_machine_keeps_the_new_address() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (a, _fa) = side("addr-a", false, false).await; let (b, _fb) = side("addr-b", false, false).await;
    let id = link(&a, &b, true).await;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let moved = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { loop { let Ok((mut c, _)) = l.accept().await else { return }; tokio::spawn(async move { let mut x = [0u8; 512]; let _ = c.read(&mut x).await; let _ = c.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").await; }); } });
    // b holds a wrong address for a
    b.hub.commit(&json!({"$":"PeerSet","id":id,"name":"","url":"http://127.0.0.1:1","revoked":false,"at":4})).await.unwrap(); settle().await;
    assert_eq!(b.p.table.with(|t| t.url(&id)), "http://127.0.0.1:1");
    let say = |u: &str| { let body = nonced(&json!({"id":0,"m":"peer.address","p":{"url":u}}).to_string()); let (home, to, id) = (a.home.clone(), url(&b), id.clone()); async move { botnet::to_peer(&home, &to, &id, now(), "/far/rpc", true, &body).await } };
    let r = say("http://127.0.0.1:2").await; assert_eq!(r.code, 409, "an address that does not answer is not kept: {}", r.body);
    assert_eq!(b.p.table.with(|t| t.url(&id)), "http://127.0.0.1:1");
    let r = say(&moved).await; assert_eq!(r.code, 200, "{}", r.body);
    assert_eq!(b.p.table.with(|t| t.url(&id)), moved, "the new address is kept");
    assert_eq!(say(&moved).await.code, 200, "the same again changes nothing");
    assert_eq!(say("ftp://x").await.code, 400);
    // a link nobody approved is not believed
    for n in [&a, &b] { n.hub.commit(&json!({"$":"SettingSet","key":format!("peer.own.{id}"),"value":"","at":9})).await.unwrap(); } settle().await;
    let r = say("http://127.0.0.1:3").await; assert_eq!(r.code, 403, "{}", r.body);
    // and the secret is the link's: another machine's request with no signature is not heard
    let r = botnet::plain(&format!("{}/far/rpc", url(&b))).await; assert_eq!(r.code, 401);
}

/// A message to a thread elsewhere is held while its machine is down (an Unsent row in the strip with Edit and Remove), one removed never goes, and the rest go,
/// oldest first, once the machine is back; the owner takes a message it took already (a lost answer posted again) once.
#[tokio::test] async fn far_messages_wait_for_their_machine_and_go_when_it_is_back() {
    let down = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let slot: Arc<Mutex<Option<Arc<FarSync>>>> = Arc::new(Mutex::new(None)); let (s2, d2) = (slot.clone(), down.clone());
    let extra: Extra = Arc::new(move |hub, req| { let (s, down) = (s2.lock().unwrap().clone(), d2.load(std::sync::atomic::Ordering::SeqCst)); Box::pin(async move {
        let s = s?; if req.route() != "/far/rpc" || req.method != "POST" { return None }
        if down && !String::from_utf8_lossy(&req.body).contains("peer.") { return Some(HttpReply::text("503 Service Unavailable", "down")) }
        Some(serve_with(false, &s, &hub, &req).await) }) });
    let a = node_extra("rfo-a", false, false, Some(extra)).await;
    a.hub.commit(&json!({"$":"ThreadCreated","id":"t","project":"","title":"T","env":"","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
    let fa = FarSync::new(FarStore::open(&a.home).unwrap(), a.p.clone()); let feed = Feed::new(); feed.start(&a.hub).await; let _ = fa.feed.set(feed);
    a.hub.set_far_router(Arc::new(Router::with_copy(fa.clone(), false))); *slot.lock().unwrap() = Some(fa);
    let rec = Rec::new(ALL); a.hub.register_rpc(rec.clone());
    let (b, fb) = side("rfo-b", false, false).await; let _ = fb;
    let out = Outbox::open(None); b.hub.register_rpc(Arc::new(farsend::Rpc(out.clone())));
    let rb = Arc::new(Router::with_outbox(fb.clone(), false, out.clone()));
    let l = link(&a, &b, true).await;
    // (side() installed a router already; this one with the outbox to look at takes b's requests through `call` directly)
    let th = format!("{l}~t");
    let call = |id: u64, text: &str, msg: &str| { let (rb, hub, th) = (rb.clone(), b.hub.clone(), th.clone()); let p = json!({"thread": th, "text": text, "msg": msg}); async move { rb.call(&hub, 1, &json!(id), "turn.start", &p).await } };
    let r = call(1, "one", "m1").await; assert_eq!((r[0]["ok"].as_bool(), r[0]["result"]["held"].as_bool()), (Some(true), Some(true)), "answered at once: {r:?}");
    call(2, "two", "m2").await;
    // the phones' path (data/1 asks and waits): held the same way, answered at once
    let r = rb.ask(&b.hub, 1, &json!(3), "turn.start", &json!({"thread": th, "text": "three", "msg": "m3"})).await.expect("a thread elsewhere is the router's");
    assert_eq!(r["result"]["held"], true, "{r}");
    let mut tag = String::new();
    for _ in 0..100 { tag = out.rows(&th, farsend::now_ms()).first().map(|r| r["tag"].as_str().unwrap_or("").to_string()).unwrap_or_default(); if tag == "Unsent" { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
    assert_eq!(tag, "Unsent", "its machine is down: the strip says so");
    let rows = out.rows(&th, farsend::now_ms()); assert_eq!(rows.len(), 3); assert_eq!(rows[1]["buttons"][1], json!({"label":"Remove","action":"o-remove","value":"m2"}));
    assert!(rec.seen().is_empty(), "nothing reached it");
    // Remove on the second: hub-local (`of` is not renamed, so it is not sent on), and it answers the text (Edit puts it in the draft)
    let r = ask(&b, 9, "outbox.remove", json!({"of": th, "msg": "m2"})).await; assert_eq!((r["ok"].as_bool(), r["result"]["text"].as_str()), (Some(true), Some("two")), "{r}");
    down.store(false, std::sync::atomic::Ordering::SeqCst);
    for _ in 0..200 { if out.items().is_empty() { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
    assert!(out.items().is_empty(), "delivered once the machine is back: {:?}", out.items());
    let seen: Vec<(String, String)> = rec.seen().into_iter().map(|(m, p)| (m, format!("{}/{}", p["thread"].as_str().unwrap_or(""), p["msg"].as_str().unwrap_or("")))).collect();
    assert_eq!(seen, vec![("turn.start".to_string(), "t/m1".to_string()), ("turn.start".to_string(), "t/m3".to_string())], "oldest first, the removed one never");
    // the same message posted again (its answer was lost): taken once
    let again = signed(&b, &a, &l, json!({"id":0,"m":"turn.start","p":{"thread":"t","text":"one","msg":"m1"}})).await; assert_eq!(again.code, 200);
    assert_eq!(rec.seen().len(), 2, "not run twice");
}

/// An agent's thread_send to a thread on a machine that is down waits here (an mcp.call in the far outbox) and goes the moment the machine is back.
#[tokio::test] async fn an_agents_send_to_a_machine_that_is_down_goes_when_it_is_back() {
    struct Mcp(Mutex<Vec<(String, String, Value)>>);
    impl crate::api::FarMcp for Mcp { fn serve<'a>(&'a self, _l: &'a str, from: &'a str, tool: &'a str, args: &'a Value) -> Fut<'a, (bool, Value)> { Box::pin(async move { self.0.lock().unwrap().push((from.to_string(), tool.to_string(), args.clone())); (true, json!({"state":"sent"})) }) } }
    let down = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let slot: Arc<Mutex<Option<Arc<FarSync>>>> = Arc::new(Mutex::new(None)); let (s2, d2) = (slot.clone(), down.clone());
    let extra: Extra = Arc::new(move |hub, req| { let (s, down) = (s2.lock().unwrap().clone(), d2.load(std::sync::atomic::Ordering::SeqCst)); Box::pin(async move {
        let s = s?; if req.route() != "/far/rpc" || req.method != "POST" { return None }
        if down && !String::from_utf8_lossy(&req.body).contains("peer.") { return Some(HttpReply::text("503 Service Unavailable", "down")) }
        Some(serve_with(false, &s, &hub, &req).await) }) });
    let a = node_extra("rfm-a", false, false, Some(extra)).await;
    let fa = FarSync::new(FarStore::open(&a.home).unwrap(), a.p.clone()); *slot.lock().unwrap() = Some(fa);
    let mcp = Arc::new(Mcp(Default::default())); a.hub.set_far_mcp(mcp.clone());
    let (b, fb) = side("rfm-b", false, false).await;
    let out = Outbox::open(None); let rb = Arc::new(Router::with_outbox(fb.clone(), false, out.clone()));
    let l = link(&a, &b, true).await; rb.work(&b.hub);
    out.hold_m(&format!("{l}~t"), &l, "mcp.call", "fs1", json!({"tool":"thread_send","from":"t9","args":{"threadId":"t","text":"look again","_msg":"fs1"}}), farsend::now_ms());
    for _ in 0..60 { if out.items().first().map_or(false, |h| h.state == "away") { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
    assert_eq!(out.rows(&format!("{l}~t"), farsend::now_ms())[0]["tag"], "Unsent"); assert!(mcp.0.lock().unwrap().is_empty());
    down.store(false, std::sync::atomic::Ordering::SeqCst); let back = std::time::Instant::now();
    for _ in 0..200 { if out.items().is_empty() { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
    assert!(out.items().is_empty(), "delivered: {:?}", out.items()); assert!(back.elapsed() < std::time::Duration::from_secs(7), "within a retry of the machine coming back");
    let got = mcp.0.lock().unwrap().clone(); assert_eq!(got.len(), 1); assert!(got[0].0.ends_with("~t9"), "the sender, named as the owner names it: {}", got[0].0);
    assert_eq!((got[0].1.as_str(), got[0].2["text"].as_str(), got[0].2["_msg"].as_str()), ("thread_send", Some("look again"), Some("fs1")));
}

/// Copy.turns: a message held for a thread elsewhere (a far-send a copied home's `far-sends.json` still names) never leaves a copy: its worker does not start, so the
/// owner's machine (the person's real one) starts no turn with it. The same router without the copy switch delivers it, so the switch is the only difference.
#[tokio::test] async fn a_held_far_send_never_leaves_a_copy() {
    for copy in [true, false] {
        let slot: Arc<Mutex<Option<Arc<FarSync>>>> = Arc::new(Mutex::new(None)); let s2 = slot.clone();
        let extra: Extra = Arc::new(move |hub, req| { let s = s2.lock().unwrap().clone(); Box::pin(async move {
            let s = s?; if req.route() != "/far/rpc" || req.method != "POST" { return None }
            Some(serve_with(false, &s, &hub, &req).await) }) });
        let a = node_extra(&format!("rfc-a-{copy}"), false, false, Some(extra)).await;
        a.hub.commit(&json!({"$":"ThreadCreated","id":"t","project":"","title":"T","env":"","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
        let fa = FarSync::new(FarStore::open(&a.home).unwrap(), a.p.clone()); let feed = Feed::new(); feed.start(&a.hub).await; let _ = fa.feed.set(feed);
        a.hub.set_far_router(Arc::new(Router::with_copy(fa.clone(), false))); *slot.lock().unwrap() = Some(fa);
        let rec = Rec::new(ALL); a.hub.register_rpc(rec.clone());
        let (b, fb) = side(&format!("rfc-b-{copy}"), false, false).await;
        let l = link(&a, &b, true).await;
        let out = Outbox::open(None); out.hold(&format!("{l}~t"), &l, json!({"thread":"t","text":"go on","msg":"fc1"}), farsend::now_ms());
        let rb = Arc::new(Router::with_outbox(fb.clone(), copy, out.clone())); rb.work(&b.hub);
        for _ in 0..80 { if !copy && out.items().is_empty() { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
        if copy { assert!(rec.seen().is_empty(), "a copy posts no held message: {:?}", rec.seen()); assert_eq!(out.items().len(), 1, "it stays held") }
        else { assert!(out.items().is_empty(), "delivered without the switch: {:?}", out.items()); assert_eq!(rec.seen().len(), 1, "the owner took it once") }
    }
}

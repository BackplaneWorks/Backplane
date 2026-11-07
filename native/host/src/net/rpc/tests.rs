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
    let p = PReq { method: "POST".into(), target: "/far/rpc".into(), headers: vec![], body: b"{}".to_vec(), trusted: false, own_device: false };
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

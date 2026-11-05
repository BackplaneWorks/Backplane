//! Two throwaway hubs: A is the owner (it signs pushes and answers pulls with a canned log), B mirrors. Real curl over
//! loopback; no real peers. The routes B serves are `FarRoutes` behind the same front the peering tests use.
use super::*;
use crate::net::peering::tests::{know, live_ids, node, node_extra, settle, url, Extra, Node};
use crate::net::{botnet, farstore::FarStore};
use std::sync::atomic::{AtomicUsize, Ordering};

fn it(n: u64, c: Value) -> Value { json!({"n": n, "c": c}) }
fn thread(n: u64, id: &str) -> Value { it(n, json!({"$":"ThreadCreated","id":id,"project":"","title":"T","env":"","branch":"","worktree":"","provider":"claude","model":"","at":n})) }
fn msg(n: u64, th: &str, text: &str) -> Value { it(n, json!({"$":"MessagePosted","thread":th,"msg":format!("m{n}"),"role":if n % 2 == 0 { "user" } else { "assistant" },"text":text,"at":n})) }

/// B's front: /far/push goes to its FarSync.
async fn mirror(tag: &str) -> (Node, Arc<FarSync>) {
    let slot: Arc<Mutex<Option<Arc<FarSync>>>> = Arc::new(Mutex::new(None));
    let s2 = slot.clone();
    let extra: Extra = Arc::new(move |hub, req| { let s = s2.lock().unwrap().clone(); Box::pin(async move {
        let s = s?; if req.route() != "/far/push" || req.method != "POST" { return None }
        Some(s.push(&hub, &req).await) }) });
    let b = node_extra(tag, false, false, Some(extra)).await;
    let fs = FarSync::new(FarStore::open(&b.home).unwrap(), b.p.clone());
    *slot.lock().unwrap() = Some(fs.clone());
    (b, fs)
}
/// A links with B (through the proof path) and both know each other.
async fn linked(a: &Node, b: &Node) -> String {
    know(a, b); know(b, a);
    a.p.ask(&a.hub, &url(b), &url(a), false).await; settle().await;
    let l = live_ids(b); assert_eq!(l.len(), 1, "linked: {l:?}");
    // the link is verified as the owner's (as a pairing over the tailnet would record it)
    for n in [a, b] { n.hub.commit(&json!({"$":"SettingSet","key":format!("peer.own.{}", l[0]),"value":"pair","at":1})).await.unwrap(); }
    l[0].clone()
}
async fn push(a: &Node, b: &Node, link: &str, body: Value) -> botnet::Resp {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    botnet::to_peer(&a.home, &url(b), link, now, "/far/push", true, &body.to_string()).await
}

#[tokio::test] async fn far_push_from_the_owner_lands_in_the_mirror_not_the_log() {
    let a = node("fa1", false).await; let (b, fs) = mirror("fb1").await;
    let link = linked(&a, &b).await;
    fs.store.take(&link, "e1", 0, 0, &[]).unwrap(); // (a mirror with no epoch yet pulls on its first push, as Bend: Fr.epoch.changed)
    let n0 = b.hub.store.len();
    let r = push(&a, &b, &link, json!({"epoch":"e1","since":0,"head":2,"items":[thread(1, "t9"), msg(2, "t9", "hello from A")]})).await;
    assert_eq!((r.code, r.body.as_str()), (200, r#"{"ok":true}"#));
    let lid = format!("{link}~t9");
    assert_eq!(fs.store.head(&link), 2);
    use crate::api::FarView;
    assert!(fs.store.has(&lid)); assert_eq!(fs.store.page(&lid, 0, 32).unwrap().rows[0].text, "hello from A");
    assert_eq!(b.hub.store.len(), n0, "B's log is untouched: a mirror is not in it");
}

#[tokio::test] async fn far_push_is_refused_from_a_stranger_a_replay_and_a_machine_sync_is_off_for() {
    let a = node("fa2", false).await; let (b, fs) = mirror("fb2").await;
    let link = linked(&a, &b).await;
    fs.store.take(&link, "e", 0, 0, &[]).unwrap();
    let body = json!({"epoch":"e","since":0,"head":1,"items":[thread(1, "t")]});
    let nth = |n: u64| { let mut b = body.clone(); b["note_n"] = json!(n); b }; // (each signed body distinct: a signature is accepted once)
    // unsigned: 401 (the intake)
    let r = botnet::plain(&format!("{}/far/push", url(&b))).await; assert_eq!(r.code, 401, "{}", r.body);
    // not an owner's machine: a link that exists but is not verified
    let c = node("fc2", false).await; know(&c, &b); know(&b, &c);
    c.p.ask(&c.hub, &url(&b), &url(&c), false).await; settle().await;
    let other: Vec<String> = live_ids(&b).into_iter().filter(|x| *x != link).collect(); assert_eq!(other.len(), 1);
    know(&b, &a); // (know replaces the list: B finds A again)
    let r = push(&c, &b, &other[0], body.clone()).await;
    assert_eq!((r.code, r.body.as_str()), (403, "threads are shared with the owner's own machines"), "linked but unverified");
    // sync off for A's name: 403
    let name = b.p.table.with(|p| p.peer(&link).unwrap().name.clone());
    b.hub.commit(&json!({"$":"SettingSet","key":format!("sync.{}", name.to_lowercase()),"value":"off","at":2})).await.unwrap();
    assert_eq!(push(&a, &b, &link, nth(1)).await.code, 403);
    b.hub.commit(&json!({"$":"SettingSet","key":format!("sync.{}", name.to_lowercase()),"value":"","at":3})).await.unwrap();
    assert_eq!(push(&a, &b, &link, nth(2)).await.code, 200);
    // the same signed bytes again are a replay
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let s = botnet::peer_secret(&a.home, &link); let raw = body.to_string();
    let first = botnet::signed(&a.home, &url(&b), &link, &s, now, "/far/push", true, &raw).await; let again = botnet::signed(&a.home, &url(&b), &link, &s, now, "/far/push", true, &raw).await;
    assert_eq!((first.code, again.code), (200, 401));
    assert_eq!(fs.store.head(&link), 1);
}

#[tokio::test] async fn far_a_gap_or_another_epoch_makes_the_mirror_pull_from_the_owner() {
    // A answers POST /far/log with a canned history in two pages
    let calls = Arc::new(AtomicUsize::new(0)); let c2 = calls.clone();
    let log: Extra = Arc::new(move |_hub, req| { let c = c2.clone(); Box::pin(async move {
        if req.route() != "/far/log" { return None }
        let o: Value = serde_json::from_slice(&req.body).unwrap(); let since = o["since"].as_u64().unwrap();
        c.fetch_add(1, Ordering::SeqCst);
        let body = if since == 0 { json!({"since":0,"catalog":[{"id":"proj","title":"P","at":3,"n":3}],"head":2,"items":[thread(1, "t"), msg(2, "t", "one")],"more":true,"epoch":"E"}) }
            else { json!({"since":since,"catalog":[{"id":"proj","title":"P","at":3,"n":3}],"head":3,"items":[msg(3, "t", "two")],"more":false,"epoch":"E"}) };
        Some(HttpReply { status: "200 OK", ctype: "application/json".into(), extra: String::new(), body: body.to_string().into_bytes() }) }) });
    let a = node_extra("fa3", false, false, Some(log)).await; let (b, fs) = mirror("fb3").await;
    let link = linked(&a, &b).await;
    // a push that starts past B's head (0) is a gap: the answer is ok, then B pulls both pages
    push(&a, &b, &link, json!({"epoch":"E","since":2,"head":3,"items":[msg(3, "t", "two")]})).await;
    for _ in 0..100 { if fs.store.head(&link) == 3 { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert_eq!(fs.store.head(&link), 3, "pulled to the end");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "two pages");
    use crate::api::FarView;
    assert_eq!(fs.store.page(&format!("{link}~t"), 0, 32).unwrap().rows.len(), 2);
    assert_eq!(fs.catalog(&link)[0]["id"], "proj", "the catalog of the first page is kept");
    // a push in another epoch is only a hint: nothing is taken from it, the mirror pulls
    let before = calls.load(Ordering::SeqCst);
    push(&a, &b, &link, json!({"epoch":"OTHER","since":3,"head":4,"items":[msg(4, "t", "late old push")]})).await;
    for _ in 0..100 { if calls.load(Ordering::SeqCst) > before { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert!(calls.load(Ordering::SeqCst) > before, "it pulled");
    assert!(!fs.store.page(&format!("{link}~t"), 0, 32).map_or(false, |p| p.rows.iter().any(|r| r.text == "late old push")), "the push itself was not trusted");
}
#[test] fn far_own_link_needs_live_verified_not_off_and_found() {
    let mut p = Peers::default();
    let ps = |id: &str, name: &str, rev: bool| json!({"$":"PeerSet","id":id,"name":name,"url":"http://100.2.0.1:3787","revoked":rev,"at":1});
    p.fold(&ps("p1", "desk", false));
    let j = json!({"self":{"name":"me","url":"http://100.1.0.1:3787","domain":""},"peers":[{"name":"desk","url":"http://100.2.0.1:3787"}]});
    assert!(!own_link(&p, &j, "p1"), "unverified");
    p.fold(&json!({"$":"SettingSet","key":"peer.own.p1","value":"pair","at":1})); assert!(own_link(&p, &j, "p1"));
    assert!(!own_link(&p, &j, "p2"), "unknown"); assert!(!own_link(&p, &json!({"self":{},"peers":[]}), "p1"), "not found by this hub");
    p.fold(&json!({"$":"SettingSet","key":"sync.desk","value":"off","at":2})); assert!(!own_link(&p, &j, "p1"));
    p.fold(&json!({"$":"SettingSet","key":"sync.desk","value":"","at":3})); p.fold(&json!({"$":"SettingSet","key":"peer.own.p1","value":"no","at":3})); assert!(!own_link(&p, &j, "p1"), "the owner said it is not theirs");
    p.fold(&json!({"$":"SettingSet","key":"peer.own.p1","value":"tailnet","at":4})); p.fold(&ps("p1", "", true)); assert!(!own_link(&p, &j, "p1"), "revoked");
}

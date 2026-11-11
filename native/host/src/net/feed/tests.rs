use super::*;
use crate::{hub::Cfg, presenter::Canned};

fn ev(j: Value) -> Value { j }
fn created(id: &str, project: &str, at: u64) -> Value { ev(json!({"$":"ThreadCreated","id":id,"project":project,"title":"T","env":"","branch":"","worktree":"","provider":"claude","model":"","at":at})) }
fn msg(th: &str, n: u64) -> Value { ev(json!({"$":"MessagePosted","thread":th,"msg":format!("m{n}"),"role":"user","text":"hi","at":n})) }

#[test] fn far_feed_goes_by_the_state_after_the_line() {
    let mut ix = Index::default();
    let lines = [json!({"$":"ProjectCreated","id":"p1","title":"P","root":"/p","at":1}), created("t1", "p1", 2), msg("t1", 3),
        json!({"$":"SettingSet","key":"k","value":"v","at":4}), msg("ghost", 5), json!({"$":"BotSet","id":"b","name":"B","thread":"","look":1,"persona":"","at":6}),
        json!({"$":"AskOpened","id":"a1","thread":"t1","kind":"approval","tool":"x","detail":"","options":"","at":7}), json!({"$":"AskClosed","id":"a1","answer":"y","at":8}),
        json!({"$":"PeerSet","id":"p","name":"n","url":"http://u","revoked":false,"at":9})];
    for (i, l) in lines.iter().enumerate() { ix.fold(i as u32 + 1, l) }
    assert_eq!(ix.shared_after(0), &[1, 2, 3, 6, 7, 8], "project, thread, message, bot, ask opened and closed; not a setting, a ghost thread or a peer");
    assert_eq!(ix.shared_after(3), &[6, 7, 8]); assert_eq!(ix.shared_after(8), &[] as &[u32]);
    assert_eq!((ix.last("p1"), ix.last("")), (8, 6), "an ask's close belongs to its thread's project; a bot to none");
}
#[test] fn far_a_mirrored_thread_is_never_re_exported() {
    let mut ix = Index::default();
    ix.fold(1, &created("p9~t", "", 1)); ix.fold(2, &msg("p9~t", 2));
    assert!(ix.shared_after(0).is_empty(), "ids with a ~ are mirrors");
}
#[test] fn far_epoch_is_the_signature_of_the_first_line_and_the_view() {
    // Far.history: SG.Sign.header("00", 0, first ++ "|view:4")
    let first = r#"{"$":"ProjectCreated","id":"p","title":"t","root":"/","at":1}"#;
    let e = epoch_of(first, 5, 5);
    assert!(e.starts_with("sha256=") && e.len() == 7 + 64);
    assert_eq!(e, sign::header("00", 0, &format!("{first}|view:4")));
    assert_ne!(epoch_of(first, 6, 5), e, "lines this build cannot decode give the history a new identity");
    assert_eq!(epoch_of(first, 6, 5), sign::header("00", 0, &format!("{first}|unknown:1|view:4")));
    assert_ne!(epoch_of("{\"$\":\"other\"}", 5, 5), e);
}
#[test] fn far_a_commit_during_the_build_is_applied_once() {
    let f = Feed::new();
    f.observe(&[created("t", "", 5)], 5); f.observe(&[msg("t", 6)], 6); // arrive while the build runs
    let mut ix = Index::default();
    for (i, l) in [json!({"$":"ProjectCreated","id":"p","title":"P","root":"/","at":1}), json!({"$":"ProjectCreated","id":"q","title":"Q","root":"/","at":2}), json!({"$":"ProjectCreated","id":"r","title":"R","root":"/","at":3}), json!({"$":"ProjectCreated","id":"s","title":"S","root":"/","at":4}), created("t", "", 5)].iter().enumerate() { ix.fold(i as u32 + 1, l) }
    ix.applied = 5; f.finish(ix);
    f.with(|ix| assert_eq!(ix.shared_after(0), &[1, 2, 3, 4, 5, 6], "line 5 was in the build and in the buffer: once"));
    f.observe(&[msg("t", 7)], 7); f.with(|ix| assert_eq!(ix.shared_after(5), &[6, 7]));
}
async fn hub(tag: &str) -> (Arc<Hub>, std::path::PathBuf) {
    let h = std::env::temp_dir().join(format!("bp-feed-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap();
    (Hub::start(Cfg { home: h.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap(), h)
}
#[tokio::test] async fn far_log_answers_a_pull_with_wire_lines_a_page_at_a_time() {
    let (hub, h) = hub("log").await;
    let n0 = hub.store.len() as u64; // (a started hub writes housekeeping lines of its own first)
    let big = "x".repeat(250_000);
    hub.commit_many(&[json!({"$":"ProjectCreated","id":"p1","title":"P","root":"/p","at":1}), created("t1", "p1", 2), msg("t1", 3),
        json!({"$":"DeviceRegistered","token":"SECRETTOKEN","platform":"ios","kind":"alert","thread":"","env":"production","bundle":"b","at":4}),
        json!({"$":"MessagePosted","thread":"t1","msg":"b1","role":"user","text":big,"at":5}), json!({"$":"MessagePosted","thread":"t1","msg":"b2","role":"user","text":big,"at":6}),
        json!({"$":"MessagePosted","thread":"t1","msg":"b3","role":"user","text":"tail","at":7})]).await.unwrap();
    let f = Feed::new(); assert!(f.answer(&hub, 0, "", json!([])).await.is_none(), "not ready: the asker pulls again");
    f.start(&hub).await;
    let cat = f.catalog(&hub).await;
    assert_eq!((cat[0]["id"].as_str(), cat[0]["title"].as_str(), cat[0]["n"].as_u64()), (Some("p1"), Some("P"), Some(n0 + 7)), "{cat}");
    let p1 = f.answer(&hub, 0, "", cat.clone()).await.unwrap();
    assert_eq!(p1["epoch"], epoch(&hub)); assert_eq!(p1["since"], 0);
    let ns: Vec<u64> = p1["items"].as_array().unwrap().iter().map(|i| i["n"].as_u64().unwrap()).collect();
    assert_eq!(ns, vec![n0 + 1, n0 + 2, n0 + 3, n0 + 5], "the device registration is not shared; the first big message fits, the second starts the next page");
    assert_eq!((p1["more"].as_bool(), p1["head"].as_u64()), (Some(true), Some(n0 + 5)));
    let p2 = f.answer(&hub, n0 + 5, &epoch(&hub), cat.clone()).await.unwrap();
    assert_eq!(p2["items"].as_array().unwrap().iter().map(|i| i["n"].as_u64().unwrap()).collect::<Vec<_>>(), vec![n0 + 6, n0 + 7]);
    assert_eq!((p2["more"].as_bool(), p2["head"].as_u64()), (Some(false), Some(hub.store.len() as u64)));
    // another history's epoch is asked from the start
    assert_eq!(f.answer(&hub, n0 + 5, "sha256=other", cat).await.unwrap()["since"], 0);
    assert!(!p1.to_string().contains("SECRETTOKEN"));
    let _ = std::fs::remove_dir_all(h);
}

#[test] fn far_epoch_on_unknown_lines() {
    let body = |first: &str, len: usize, seq: usize| epoch_of(first, len, seq);
    assert_eq!(body("x", 3, 3), sign::header("00", 0, "x|view:4")); assert_eq!(body("x", 5, 3), sign::header("00", 0, "x|unknown:2|view:4"));
}
#[test] fn far_epoch_view_bumped() { assert_ne!(epoch_of("x", 3, 3), sign::header("00", 0, "x|view:2")); }
#[tokio::test] async fn far_a_commit_that_shares_goes_to_the_push_machine_in_wire_form() {
    let (hub, h) = hub("push").await;
    let f = Feed::new(); f.start(&hub).await; let mut rx = f.pushes();
    let n0 = hub.store.len() as u64;
    f.observe(&[json!({"$":"SettingSet","key":"k","value":"v","at":1})], n0 as u32 + 1);
    assert!(rx.try_recv().is_err(), "a setting stays home");
    f.observe(&[created("t", "", 2), msg("t", 3)], n0 as u32 + 2);
    let (from, to, items) = rx.try_recv().unwrap();
    assert_eq!((from, to, items.iter().map(|i| i.n).collect::<Vec<_>>()), (n0 + 1, n0 + 3, vec![n0 + 2, n0 + 3]));
    assert_eq!(items[1].c["text"], "hi");
    let _ = std::fs::remove_dir_all(h);
}

#[tokio::test] async fn far_a_pull_across_many_slices_and_a_commit_while_it_reads() {
    let (hub, h) = hub("slices").await;
    let n0 = hub.store.len() as u64;
    let evs: Vec<Value> = (0..9000).map(|i| msg("t", i + 10)).collect();
    hub.commit_many(&[json!({"$":"ProjectCreated","id":"p1","title":"P","root":"/p","at":1}), created("t", "p1", 2)]).await.unwrap();
    for c in evs.chunks(1000) { hub.commit_many(c).await.unwrap(); }
    let f = Feed::new(); f.start(&hub).await;
    // the observer is not held up while a pull runs (the lock is taken only to copy numbers)
    let f2 = f.clone(); let hub2 = hub.clone();
    let pull = tokio::spawn(async move { f2.answer(&hub2, 0, "", json!([])).await.unwrap() });
    for i in 0..50u32 { f.observe(&[msg("t", 100_000 + i as u64)], hub.store.len() as u32 + 1 + i) }
    let a = pull.await.unwrap();
    let ns: Vec<u64> = a["items"].as_array().unwrap().iter().map(|i| i["n"].as_u64().unwrap()).collect();
    assert!(ns.windows(2).all(|w| w[0] < w[1]) && ns[0] == n0 + 1, "in order, from the start");
    assert!(a["more"].as_bool().unwrap() || ns.len() >= 9000, "{} items, more={}", ns.len(), a["more"]);
    let _ = std::fs::remove_dir_all(h);
}

#[tokio::test] async fn far_a_shared_commit_landing_mid_answer_reaches_the_mirror_by_the_next_pull() {
    let (hub, h) = hub("mid").await;
    hub.commit_many(&[json!({"$":"ProjectCreated","id":"p1","title":"P","root":"/p","at":1}), created("t1", "p1", 2), msg("t1", 3)]).await.unwrap();
    let f = Feed::new(); f.start(&hub).await;
    let top = hub.store.len() as u64;                       // the answer read the end here ...
    hub.commit(&msg("t1", 4)).await.unwrap();               // ... and a shared line lands before it finished
    f.observe(&[msg("t1", 4)], top as u32 + 1);             // (the writer's observer; the test hub has none of ours registered)
    let a = f.answer_at(&hub, top, 0, "", json!([])).await.unwrap();
    assert!(a["items"].as_array().unwrap().iter().all(|i| i["n"].as_u64().unwrap() <= top), "no line above the end that was read");
    assert_eq!((a["head"].as_u64(), a["more"].as_bool()), (Some(top), Some(false)), "the head is that end, not the log's end now");
    // the mirror takes it: head = top. The push of the new line (n = top + 1, since = top) then lands, not filtered out
    let mirror = crate::net::farstore::FarStore::open(&h).unwrap();
    let items: Vec<far::Item> = a["items"].as_array().unwrap().iter().map(far::Item::of).collect();
    mirror.take("L", "e", 0, top, &items).unwrap();
    let pushed = mirror.take("L", "e", top, top + 1, &[far::Item { n: top + 1, c: msg("t1", 4) }]).unwrap();
    assert_eq!(pushed.events.len(), 1, "the line the pull did not carry arrives with its push");
    // and the next pull carries it too
    let next = f.answer_at(&hub, top + 1, top, &epoch(&hub), json!([])).await.unwrap();
    assert_eq!(next["items"].as_array().unwrap().iter().map(|i| i["n"].as_u64().unwrap()).collect::<Vec<_>>(), vec![top + 1]);
    let _ = std::fs::remove_dir_all(h);
}
#[tokio::test] async fn far_a_big_line_opening_a_new_slice_still_waits_for_a_page_with_room() {
    let (hub, h) = hub("slice-big").await;
    // 4096 small shared lines fill one slice; the big one is the first of the next
    let small: Vec<Value> = (0..4096).map(|i| msg("t", i + 10)).collect();
    hub.commit_many(&[json!({"$":"ProjectCreated","id":"p1","title":"P","root":"/p","at":1}), created("t", "p1", 2)]).await.unwrap();
    for c in small.chunks(1000) { hub.commit_many(c).await.unwrap(); }
    hub.commit(&json!({"$":"MessagePosted","thread":"t","msg":"big","role":"user","text":"x".repeat(700_000),"at":9})).await.unwrap();
    let f = Feed::new(); f.start(&hub).await;
    let a = f.answer(&hub, 0, "", json!([])).await.unwrap();
    let texts: usize = a["items"].as_array().unwrap().iter().map(|i| i["c"]["text"].as_str().map_or(0, |t| t.len())).sum();
    assert!(texts < 700_000 && a["more"] == true, "the 700k line did not join a page that already holds thousands of lines ({} chars, more={})", texts, a["more"]);
    let _ = std::fs::remove_dir_all(h);
}

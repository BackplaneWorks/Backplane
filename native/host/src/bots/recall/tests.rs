//! The memory tree against a real Hub on a temp home, with a stand-in compactor (no model, no network): building and merging, the
//! view's bound, zoom down to a message, the view's prefix between turns (the provider's cache), and a restart.
use super::*;
use crate::{api::Fut, hub::{Cfg, Hub}, presenter::Canned};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

const BOT: &str = "b1"; const TH: &str = "tb1";

/// The stand-in compactor: a line per task, "S<l/i>" and the start of its input, so a line says what it was made from.
/// `skip`: the calls it answers with nothing (an outage); `drop_last`: it leaves out the last task of each answer.
#[derive(Default)] struct StandIn { calls: AtomicUsize, skip: AtomicUsize, drop_last: bool }
impl Writer for StandIn {
    fn write<'a>(&'a self, _thread: &'a str, system: &'a str, input: &'a str) -> Fut<'a, Result<String, String>> {
        Box::pin(async move {
            assert_eq!(system, SYSTEM);
            self.calls.fetch_add(1, Relaxed);
            if self.skip.load(Relaxed) > 0 { self.skip.fetch_sub(1, Relaxed); return Err("no login can take it".into()) }
            let mut out = vec![];
            for t in input.split("<task id=\"").skip(1) {
                let (id, rest) = t.split_once('"').unwrap();
                let body = rest.split_once("\">\n").unwrap().1.split("\n</task>").next().unwrap();
                out.push(format!("{id}|S{id}: {}", tree::cut(&tree::flat(body), 60)));
            }
            if self.drop_last { out.pop(); }
            Ok(out.join("\n"))
        })
    }
}
struct NoInbox;
impl super::super::rpc::Inbox for NoInbox {
    fn deliver<'a>(&'a self, _: &'a str, _: &'a str, _: &'a str, _: Option<(String, String, String)>) -> Fut<'a, Result<(), String>> { Box::pin(async { Ok(()) }) }
    fn stop<'a>(&'a self, _: &'a str) -> Fut<'a, ()> { Box::pin(async {}) }
    fn ride<'a>(&'a self, _: &'a str, _: &'a str, _: &'a str) -> Fut<'a, Result<(), String>> { Box::pin(async { Ok(()) }) }
    fn note<'a>(&'a self, _: &'a str, _: &'a str, _: &'a str) -> Fut<'a, Result<(), String>> { Box::pin(async { Ok(()) }) }
}

/// Message i of the stand-in conversation: the person and the bot in turn, a tool call now and then, every 7th long.
fn text(i: usize) -> String { if i % 7 == 3 { format!("long{i} ") .repeat(200) } else { format!("message {i} about item-{i}") } }
fn msg(i: usize) -> String {
    if i % 5 == 4 { return json!({"$":"ActivityLogged","thread":TH,"act":format!("a{i}"),"tone":"tool","kind":"tool","summary":format!("Bash: ls dir-{i}"),"at":1700000000 + i}).to_string() }
    json!({"$":"MessagePosted","thread":TH,"msg":format!("m{i}"),"role":if i % 2 == 0 { "user" } else { "assistant" },"text":text(i),"at":1700000000 + i}).to_string()
}
fn base(n: usize) -> String {
    let mut s = format!("{}\n{}\n", json!({"$":"ThreadCreated","id":TH,"project":"","title":"Bub","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000000}), json!({"$":"BotSet","id":BOT,"name":"Bub","thread":TH,"look":0,"persona":"","at":1700000000}));
    // the flag (off by default) on
    s += &json!({"$":"SettingSet","key":"recall.bots","value":"on","at":1700000000}).to_string(); s.push('\n');
    // rows that are no message of the conversation: never in the tree
    s += &json!({"$":"ActivityLogged","thread":TH,"act":"x","tone":"info","kind":"note","summary":"not a message","at":1700000000}).to_string(); s.push('\n');
    for i in 0..n { s += &msg(i); s.push('\n') }
    s
}
struct Rig { hub: Arc<Hub>, bots: Arc<Bots>, rc: Arc<Recall>, w: Arc<StandIn>, home: crate::plat::tmp::TempDir }
async fn rig_in(home: crate::plat::tmp::TempDir, w: StandIn) -> Rig {
    let hub = Hub::start(Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let bots = Bots::start_with(hub.clone(), Arc::new(NoInbox)).await;
    let w = Arc::new(w); let rc = Recall::new(&bots, w.clone());
    Rig { hub, bots, rc, w, home }
}
async fn rig(name: &str, n: usize, w: StandIn) -> Rig {
    let home = crate::plat::tmp::dir(&format!("recall-{name}")); std::fs::write(home.join("events.jsonl"), base(n)).unwrap();
    rig_in(home, w).await
}
impl Rig {
    async fn build(&self) -> usize { let mut n = 0; loop { let k = self.rc.round().await; if k == 0 { return n } n += k } }
    async fn add(&self, from: usize, to: usize) { let ev: Vec<Value> = (from..to).map(|i| serde_json::from_str(&msg(i)).unwrap()).collect(); self.hub.commit_many(&ev).await.unwrap(); }
    async fn kept(&self) -> Kept { self.bots.world().await.recall.get(TH).cloned().unwrap_or_default() }
    fn log(&self) -> Vec<Value> { self.hub.store.range(0, usize::MAX).unwrap().iter().filter_map(|l| serde_json::from_str(l).ok()).collect() }
}

#[tokio::test(flavor = "multi_thread")] async fn with_the_flag_off_by_default_nothing_runs() {
    let home = crate::plat::tmp::dir("recall-flag"); std::fs::write(home.join("events.jsonl"), base(20).replace("\"value\":\"on\"", "\"value\":\"x\"")).unwrap();
    let r = rig_in(home, StandIn::default()).await;
    assert!(!r.rc.on(TH)); assert_eq!(r.rc.round().await, 0); assert_eq!(r.w.calls.load(Relaxed), 0, "no compactor call");
    use crate::agents::mcp::ToolSet; assert!(Tools(r.rc.clone()).list(TH).is_empty());
    r.hub.commit_many(&[json!({"$":"SettingSet","key":"recall.thread.t9","value":"on","at":2})]).await.unwrap();
    let _ = r.bots.world().await; assert!(!r.rc.on("t9"), "a thread's own switch needs the flag too");
    r.hub.commit_many(&[json!({"$":"SettingSet","key":"recall.bots","value":"on","at":3})]).await.unwrap();
    let _ = r.bots.world().await; assert!(r.rc.on(TH) && r.rc.on("t9"));
}

#[tokio::test(flavor = "multi_thread")] async fn with_the_flag_on_every_bot_thread_is_on_unless_its_settings_turn_it_off() {
    let r = rig("on", 3, StandIn::default()).await;
    assert!(r.rc.on(TH));
    r.hub.commit_many(&[json!({"$":"SettingSet","key":"recall.bot.b1","value":"off","at":1})]).await.unwrap();
    let _ = r.bots.world().await; assert!(!r.rc.on(TH));
    r.hub.commit_many(&[json!({"$":"SettingSet","key":"recall.bot.b1","value":"on","at":2}), json!({"$":"SettingSet","key":"recall.thread.t9","value":"on","at":2})]).await.unwrap();
    let _ = r.bots.world().await; assert!(r.rc.on(TH) && r.rc.on("t9") && !r.rc.on("t8"));
    use crate::agents::mcp::ToolSet; let tools = Tools(r.rc.clone());
    assert_eq!(tools.list(TH)[0]["name"], "memory_zoom"); assert!(tools.list("t8").is_empty());
}

#[tokio::test(flavor = "multi_thread")] async fn the_compactor_builds_every_leaf_and_merge_and_nothing_twice() {
    let r = rig("build", 40, StandIn::default()).await;
    let leaves = r.rc.leaves(TH).await; assert_eq!(leaves.len(), 40, "the info row is no message");
    let written = r.build().await;
    let kept = r.kept().await;
    // every node of the tree over 40 messages: long leaves (every 7th from 3) and all 8+4+2+1 merges under the roots 32 and 8
    let long = (0..40).filter(|i| i % 7 == 3 && i % 5 != 4).count();
    assert_eq!(written, long + (16 + 8 + 4 + 2 + 1) + (4 + 2 + 1), "{written}");
    assert_eq!(tree::roots(40), vec![(5, 0), (3, 4)]);
    for p in tree::roots(40) { assert!(kept.at(p).is_some(), "root {p:?}") }
    let noted: Vec<Value> = r.log().into_iter().filter(|e| e["$"] == "RecallNoted").flat_map(|e| e["nodes"].as_array().unwrap().clone()).collect();
    let mut ids: Vec<String> = noted.iter().map(|n| format!("{}/{}", n[0], n[1])).collect(); let k = ids.len(); ids.sort(); ids.dedup(); assert_eq!(ids.len(), k, "no node written twice");
    // a merge reads its two halves, oldest first; a long leaf its message
    let reader = Reader::new(&r.hub.store, &kept, &leaves);
    assert!(reader.line((1, 0)).unwrap().starts_with("S1/0: 0+1|user: message 0"), "{:?}", reader.line((1, 0)));
    assert!(reader.line((0, 3)).unwrap().starts_with("S0/3: you: long3"));
    assert!(r.w.calls.load(Relaxed) < written, "several lines a call");
}

#[tokio::test(flavor = "multi_thread")] async fn a_call_that_fails_holds_the_thread_and_an_unanswered_node_is_asked_again_then_cut() {
    let r = rig("fail", 8, StandIn { skip: AtomicUsize::new(1), ..Default::default() }).await;
    assert_eq!(r.rc.round().await, 0, "the outage writes nothing");
    assert_eq!(r.rc.round().await, 0, "and the thread waits");
    assert!(r.kept().await.written() == 0);
    // a compactor that never answers the last task: that node is asked TRIES times, then its source cut is its line
    let r = rig("drop", 2, StandIn { drop_last: true, ..Default::default() }).await;
    r.add(2, 4).await; // 4 messages, leaf 3 long: one call has leaf 3 and nothing else ready
    r.build().await;
    let kept = r.kept().await; let leaves = r.rc.leaves(TH).await; let reader = Reader::new(&r.hub.store, &kept, &leaves);
    // each call's last task goes unanswered: leaf 3 and (1,0) are written in later calls, (1,1) and the root, alone in theirs, end cut
    assert!(reader.line((0, 3)).unwrap().starts_with("S0/3: you: long3"), "{:?}", reader.line((0, 3)));
    let cut = reader.line((1, 1)).unwrap(); assert!(cut.starts_with("2+1|user: message 2 about item-2 3+1|S0/3") && cut.len() <= NODE, "{cut}");
    assert!(reader.line((2, 0)).unwrap().starts_with("0+2|S1/0"));
}

#[tokio::test(flavor = "multi_thread")] async fn the_view_stays_under_128_kb_and_keeps_the_newest_messages_whole() {
    let r = rig("big", 5000, StandIn::default()).await;
    // before the compactor: the backlog is cut to the newest lines, said once
    let v0 = r.rc.view(TH).await;
    assert!(v0.len() <= HIGH + tree::DOC.len() + 200, "{}", v0.len());
    assert!(v0.contains("older lines are left out") && v0.contains("4997+1|you: message 4997") && v0.contains("4999+1|tool: Bash: ls dir-4999"), "{}", &v0[v0.len() - 300..]);
    r.build().await;
    let v = r.rc.view(TH).await;
    assert!(v.len() <= HIGH + tree::DOC.len() + 200, "{}", v.len());
    assert!(!v.contains("left out"), "merged lines cover all of it");
    assert!(v.contains("\n0+") , "the view starts at the first message");
    assert!(v.contains("4997+1|you: message 4997") && v.contains("4999+1|tool: Bash: ls dir-4999"), "the newest are their own lines");
    // a batch merged it to about half and kept that
    assert!(r.log().iter().any(|e| e["$"] == "RecallView" && e["thread"] == TH));
    assert!(v.len() <= tree::DOC.len() + HIGH / 2 + 64, "merged to half: {}", v.len());
    // the oldest lines cover the most
    let first: u64 = v.split("<chat>\n").nth(1).unwrap().split('|').next().unwrap().split('+').nth(1).unwrap().parse().unwrap();
    assert!(first >= 16, "{first}");
}

#[tokio::test(flavor = "multi_thread")] async fn zoom_opens_lines_down_to_the_original_message() {
    let r = rig("zoom", 300, StandIn::default()).await; r.build().await;
    let target = 136usize; // a long one (136 % 7 == 3)
    // from the root over it down, always into the half that holds it
    let mut p = *tree::roots(300).iter().find(|p| { let (a, b) = tree::span(**p); a <= target as u64 && target as u64 <= b }).unwrap();
    while p.0 > 0 {
        let z = r.rc.zoom(TH, p.1 << p.0, 1 << p.0, 0).await;
        let ls: Vec<&str> = z.lines().collect(); assert_eq!(ls.len(), 2, "{z}");
        let hs = tree::halves(p); assert!(ls[0].starts_with(&tree::name(hs[0])) && ls[1].starts_with(&tree::name(hs[1])), "{z}");
        p = *hs.iter().find(|h| { let (a, b) = tree::span(**h); a <= target as u64 && target as u64 <= b }).unwrap();
    }
    let whole = r.rc.zoom(TH, target as u64, 1, 0).await;
    assert!(whole.starts_with("136+1|user (2023-11-1") && whole.contains(":36") && whole.contains("): long136 long136"), "with when, in the person's zone: {}", &whole[..80]);
    assert!(whole.ends_with(&text(136)), "the message whole");
    assert!(r.rc.zoom(TH, 3, 2, 0).await.starts_with("No line 3+2"), "id is a multiple of n");
    assert!(r.rc.zoom(TH, 0, 0, 0).await.contains("roots: 0+256, 256+32, 288+8, 296+4"));
}

#[tokio::test(flavor = "multi_thread")] async fn between_batches_each_view_starts_with_the_last_one_so_the_cache_holds() {
    let r = rig("cache", 2500, StandIn::default()).await; r.build().await;
    let (mut prev, mut broke, mut n) = (r.rc.view(TH).await, 0usize, 2500usize);
    let views0 = r.log().iter().filter(|e| e["$"] == "RecallView").count();
    for _ in 0..150 {
        r.add(n, n + 4).await; n += 4; r.build().await;
        let v = r.rc.view(TH).await;
        let head = prev.strip_suffix("</chat>").unwrap();
        if !v.starts_with(head) { broke += 1 }
        prev = v;
    }
    let views = r.log().iter().filter(|e| e["$"] == "RecallView").count() - views0;
    assert!(views >= 1, "600 messages more pass 128 KB again");
    assert_eq!(broke, views, "the view changes before its end only when a batch merges it");
    assert!(views <= 3, "{views} batches");
}

#[tokio::test(flavor = "multi_thread")] async fn the_turns_own_messages_are_left_to_the_turn() {
    let r = rig("own", 5, StandIn::default()).await; // 0 user, 1 you, 2 user, 3 you (long), 4 tool
    r.add(5, 7).await; // 5 you, 6 user: the turn's message
    let v = r.rc.view(TH).await;
    assert!(v.contains("5+1|you: message 5") && !v.contains("6+1|"), "{v}");
    assert!(v.contains("3+1|you: long3") && v.contains("(zoom it)"), "a long message not written yet shows cut");
    assert!(v.starts_with(tree::DOC) && v.ends_with("</chat>"));
    let r = rig("none", 0, StandIn::default()).await; assert_eq!(r.rc.view(TH).await, "");
}

#[tokio::test(flavor = "multi_thread")] async fn a_restart_finds_the_tree_and_goes_on_where_it_stopped() {
    let r = rig("restart", 500, StandIn::default()).await;
    // half a round: a few calls' lines, then the hub goes
    { let mut n = 0; while n < 80 { let b = r.rc.batch(TH).await.unwrap(); n += r.rc.write(TH, b).await } }
    let before = r.kept().await; assert!(before.written() >= 80);
    let v1 = r.rc.view(TH).await;
    let log = std::fs::read_to_string(r.home.join("events.jsonl")).unwrap();
    let home = crate::plat::tmp::dir("recall-restart-2"); std::fs::write(home.join("events.jsonl"), &log).unwrap();
    let r2 = rig_in(home, StandIn::default()).await;
    assert_eq!(r2.kept().await, before, "the tree and the kept view fold from the log");
    assert_eq!(r2.rc.view(TH).await, v1, "the same view, so the cache holds across the restart");
    r2.build().await;
    let noted: Vec<String> = r2.log().into_iter().filter(|e| e["$"] == "RecallNoted").flat_map(|e| e["nodes"].as_array().unwrap().iter().map(|n| format!("{}/{}", n[0], n[1])).collect::<Vec<_>>()).collect();
    let mut u = noted.clone(); u.sort(); u.dedup(); assert_eq!(u.len(), noted.len(), "nothing written before the restart is written again");
    let kept = r2.kept().await; for p in tree::roots(500) { assert!(kept.at(p).is_some()) }
}


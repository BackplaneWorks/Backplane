//! Infinite context (agents/recall.rs) with a stand-in claude: a thread that has it starts every new turn in a fresh session, the
//! view ahead of its message and no hand-over; a thread that has not resumes its session as before.
use super::*;
use super::lost::rig;

struct Fake(&'static str);
impl crate::agents::recall::Recall for Fake {
    fn on(&self, thread: &str) -> bool { thread == self.0 }
    fn view<'a>(&'a self, thread: &'a str) -> crate::api::Fut<'a, String> { Box::pin(async move { format!("<chat>\nVIEW-OF-{thread}\n</chat>") }) }
}
const LIVE: &str = "{\"$\":\"SessionSet\",\"id\":\"t1700000001-2\",\"session\":\"fresh-old\",\"at\":1700000060}\n";

async fn turn(r: &super::lost::Rig, text: &str) {
    let from = len(&r.hub).await; rpc(&r.hub, "turn.start", json!({"thread":T1,"text":text})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
    assert!(ev.iter().any(|e| turn_is(e, "completed")), "{ev:?}");
}

#[tokio::test(flavor = "multi_thread")] async fn each_new_turn_starts_fresh_with_the_view_and_no_hand_over() {
    let r = rig("rc-on", "result", LIVE).await; r.ag.set_recall(Arc::new(Fake(T1)));
    turn(&r, "zqshort first").await; turn(&r, "zqshort second").await;
    assert_eq!(r.starts(), vec!["START resume=".to_string(), "START resume=".to_string()], "never resumed: a fresh session each turn");
    let u = r.users(); assert_eq!(u.len(), 2, "{u:?}");
    for (line, text) in u.iter().zip(["zqshort first", "zqshort second"]) {
        assert!(line.contains(&format!("VIEW-OF-{T1}")) && line.contains(text), "{line}");
        assert!(line.find("VIEW-OF").unwrap() < line.find(text).unwrap(), "the view comes first: {line}");
        assert!(!line.contains("earlier question"), "no hand-over: the view has the conversation: {line}");
    }
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_thread_without_it_resumes_its_session_as_before() {
    let r = rig("rc-off", "result", LIVE).await; r.ag.set_recall(Arc::new(Fake("another-thread")));
    turn(&r, "zqshort first").await; turn(&r, "zqshort second").await;
    assert_eq!(r.starts(), vec!["START resume=fresh-old".to_string()], "one process, resumed, kept for the next turn");
    assert!(r.users().iter().all(|l| !l.contains("VIEW-OF")));
    r.ag.shutdown();
}

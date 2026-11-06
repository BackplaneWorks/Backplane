//! A told note (Bend's `Ride.note`, core/hub.bend `Private.tell.fresh`) joins the thread's ride and starts no turn; the next turn takes it (Ride.post).
use super::*;
use super::lost::rig;

const SESSION_OK: &str = "{\"$\":\"SessionSet\",\"id\":\"t1700000001-2\",\"session\":\"fresh-old\",\"at\":1700000060}\n";

#[tokio::test(flavor = "multi_thread")] async fn a_note_on_an_idle_thread_starts_no_turn_and_the_next_message_takes_it() {
    let r = rig("nt-idle", "result", SESSION_OK).await; let from = len(&r.hub).await;
    assert!(r.ag.ride_note(T1, "told:tell1", "[Backplane] note-words-zq the person was told").await.is_ok());
    tokio::time::sleep(Duration::from_millis(600)).await;
    let ev = log_from(&r.hub, from).await;
    assert!(!ev.iter().any(|e| is(e, "TurnChanged")), "no turn: {ev:?}");
    assert!(!ev.iter().any(|e| e["kind"] == "ride" && e["$"] == "ActivityLogged"), "nothing is said to be delivered: {ev:?}");
    assert!(r.starts().is_empty(), "no process: {:?}", r.starts());
    assert!(r.ag.world().await.setting(&format!("ride.{T1}")).contains("note-words-zq"), "the ride holds it");
    assert!(r.ag.ride_note(T1, "told:tell1", "again").await.is_ok());
    assert_eq!(r.ag.world().await.setting(&format!("ride.{T1}")).matches("told:tell1").count(), 1, "once per id");
    // the person's next message runs with it, and the ride is emptied
    let from = len(&r.hub).await; rpc(&r.hub, "turn.start", json!({"thread":T1,"text":"zqshort and now?"})).await;
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
    assert!(ev.iter().any(|e| turn_is(e, "completed")), "{ev:?}");
    let u = r.users(); assert_eq!(u.len(), 1, "{u:?}");
    assert!(u[0].contains("note-words-zq") && u[0].contains("zqshort and now?"), "{}", u[0]);
    assert!(u[0].find("note-words-zq") < u[0].find("zqshort and now?"), "the note goes ahead of the message");
    assert_eq!(r.ag.world().await.setting(&format!("ride.{T1}")), "");
    r.ag.shutdown();
}
#[tokio::test(flavor = "multi_thread")] async fn a_wake_still_starts_an_idle_thread() { // Wake.ride keeps Ride.start: the triage and lost-lane escalations
    let r = rig("nt-wake", "result", SESSION_OK).await; let from = len(&r.hub).await;
    assert!(r.ag.wake_ride(T1, "triage:x", "[Backplane] wake-words-zq").await.is_ok());
    let ev = until(&r.hub, from, 20, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
    assert!(ev.iter().any(|e| turn_is(e, "completed")), "{ev:?}"); assert!(r.users()[0].contains("wake-words-zq"));
    r.ag.shutdown();
}

use super::*;
use serde_json::Value;
fn it(n: u64) -> Item { Item { n, c: json!(n) } }
fn out(from: u64, to: u64, xs: &[u64]) -> Out { let mut o = Out::default(); o.add(from, to, xs.iter().map(|n| it(*n)).collect()); o }

#[test] fn far_fail_keeps_items() { let mut o = out(0, 2, &[1, 2]); o.next(); o.done(false, 2); assert_eq!(o.items, vec![it(1), it(2)]); }
#[test] fn far_ack_removes_batch() { let mut o = out(0, 1, &[1]); o.next(); o.done(true, 1); assert!(o.flight.is_empty()); }
#[test] fn far_ack_keeps_waiting() {
    let mut o = out(0, 1, &[1]); o.next(); o.add(1, 2, vec![it(2)]); o.done(true, 1);
    assert_eq!(o.items, vec![it(2)]);
}
#[test] fn far_retry_order() {
    let mut o = out(4, 5, &[5]); o.next(); o.add(5, 6, vec![it(6)]); o.done(false, 5);
    assert_eq!(o.items, vec![it(5), it(6)], "what came meanwhile follows the failed batch, in order");
}
#[test] fn far_retry_begins() {
    let mut o = out(4, 5, &[5]); o.next(); o.add(5, 6, vec![it(6)]); o.done(false, 5);
    assert_eq!(o.from, 4, "the batch is sent from where it began");
}
#[test] fn far_hold_waits() { let mut o = out(0, 1, &[1]); o.next(); o.done(false, 1); assert!(!o.due()); }
#[test] fn far_release_resends() { let mut o = out(0, 1, &[1]); o.next(); o.done(false, 1); o.release(); assert!(o.due()); }
#[test] fn far_backoff() { assert_eq!([1, 2, 3, 6, 9].map(delay), [2, 4, 8, 60, 60]); assert_eq!(delay(5), 32); assert_eq!(delay(0), 2); }
#[test] fn far_a_batch_follows_what_got_through_only_while_every_push_did() {
    let mut o = Out::default();
    o.add(0, 2, vec![it(1), it(2)]);
    let s = o.next().unwrap(); assert_eq!(s, Send { since: 0, to: 2, items: vec![it(1), it(2)] }, "nothing is known after a start: it follows only its own start");
    assert!(o.next().is_none(), "one at a time");
    o.add(2, 3, vec![it(3)]); assert!(o.next().is_none(), "what comes while one is out waits");
    o.done(true, 2);
    assert_eq!(o.next().unwrap(), Send { since: 2, to: 3, items: vec![it(3)] }, "now it follows the last push through");
    o.add(3, 5, vec![it(4), it(5)]); o.done(false, 3); o.release();
    assert_eq!(o.next().unwrap(), Send { since: 2, to: 5, items: vec![it(3), it(4), it(5)] }, "after a failure it follows only its own start (where the failed batch began)");
}
#[test] fn far_a_push_too_big_says_only_where_it_ends() {
    let big = Send { since: 5, to: 9, items: vec![Item { n: 9, c: json!("x".repeat(210_000)) }] };
    let b: Value = serde_json::from_str(&body("E", "me", &big)).unwrap();
    assert_eq!((b["since"].as_u64(), b["head"].as_u64(), b["items"].as_array().unwrap().len(), b["epoch"].as_str()), (Some(9), Some(9), 0, Some("E")), "a gap: the other end pulls in pages");
    let small = body("E", "me", &Send { since: 1, to: 2, items: vec![it(2)] });
    let v: Value = serde_json::from_str(&small).unwrap(); assert_eq!((v["since"].as_u64(), v["name"].as_str(), v["items"][0]["n"].as_u64()), (Some(1), Some("me"), Some(2)));
}

#[test] fn far_a_link_that_takes_nothing_holds_a_bounded_queue_and_is_told_the_gap() {
    let mut o = Out::default();
    for n in 1..=(MAX_WAITING as u64 + 1) { o.add(n - 1, n, vec![it(n)]); }
    assert!(o.items.is_empty() && o.lost, "past the limit the items are dropped");
    let s = o.next().unwrap();
    let last = MAX_WAITING as u64 + 1;
    assert_eq!(s, Send { since: last, to: last, items: vec![] }, "the gap form: only where the log ends");
    o.done(false, last); assert!(o.lost && o.items.is_empty(), "a failed gap is owed again, not forgotten"); o.release();
    o.add(last, last + 1, vec![it(last + 1)]);
    let s = o.next().unwrap(); assert_eq!(s.items.len(), 1, "what comes after the loss is sent; the gap before it is why the other end pulls");
    o.done(true, last + 1); assert!(!o.lost && !o.flight_lost);
    // a failing flight plus what waited cannot grow past the limit either
    let mut o = Out::default();
    o.add(0, 5000, (1..=5000).map(it).collect()); o.next().unwrap(); o.add(5000, 10_000, (5001..=10_000).map(it).collect());
    o.done(false, 5000); o.release(); assert_eq!(o.items.len(), 10_000);
    o.add(10_000, 10_001, vec![it(10_001)]); assert!(o.lost && o.items.is_empty());
}

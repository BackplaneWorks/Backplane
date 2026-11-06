//! The turn boundaries of the time stamps under messages: src/core/stamp's `Stamp.ends` and `Stamp.mid` (the part of
//! `Stamps` that needs no time zone) in Rust. The clock text (`Stamp.clock`, `Stamp.text`) needs tz and stays unported.
//! Each law it guards has a test here with the law's name.
use std::collections::HashMap;

/// the part of an entry the rule reads: a message (the person's or the agent's) with its id and time, or anything else
#[derive(Clone, Debug, PartialEq)]
pub enum E { User { id: String, at: u64 }, Assistant { id: String, at: u64 }, Other }

/// Steer.mark: a reply posted ahead of a message sent while it streamed
const AHEAD: &str = ":before";
fn ahead(id: &str) -> bool { id.ends_with(AHEAD) }

fn close(ends: &mut HashMap<String, u64>, held: &str, at: u64) { if !held.is_empty() { ends.insert(held.to_string(), at); } }

/// Stamp.ends.go over the entries newest first
fn ends_go(es: &[E], open0: bool) -> HashMap<String, u64> {
    let (mut open, mut held, mut ends) = (open0, String::new(), HashMap::new());
    for e in es.iter().rev() {
        match e {
            E::User { at, .. } => { close(&mut ends, &held, *at); open = true; held.clear(); }
            E::Assistant { id, .. } => {
                let a = ahead(id);
                if !a && open { held = id.clone(); }
                open = open && a;
            }
            E::Other => {}
        }
    }
    close(&mut ends, &held, 1);
    ends
}

/// Stamp.newest over the entries newest first: the newest reply after the person's newest message ("" for none)
fn newest(es_oldest_first: &[E]) -> &str {
    for e in es_oldest_first.iter().rev() {
        match e { E::User { .. } => return "", E::Assistant { id, .. } => return id, E::Other => {} }
    }
    ""
}

/// Stamp.ends: each finished turn's last reply (its id) to when the turn started (1 when the page does not hold its start);
/// while the thread's turn runs its newest reply is kept with 0 (it ends no turn, but stays in sight)
pub fn ends(es: &[E], running: bool) -> HashMap<String, u64> {
    let mut m = ends_go(es, !running);
    if running { let id = newest(es); close(&mut m, id, 0); }
    m
}

/// Stamp.mid: an agent reply that does not end a turn (its stamp is a time that shows only on hover); the turn's own stamps always show
pub fn mid(ends: &HashMap<String, u64>, e: &E) -> bool { match e { E::Assistant { id, .. } => !ends.contains_key(id), _ => false } }

#[cfg(test)]
mod tests {
    use super::*;
    fn turn() -> Vec<E> { vec![E::User { id: "u".into(), at: 1 }, E::Assistant { id: "a1".into(), at: 2 }, E::Assistant { id: "a2".into(), at: 3 }] }
    fn a(id: &str) -> E { E::Assistant { id: id.into(), at: 0 } }
    #[test] fn stamp_mid_never_user() {
        for at in [0u64, 1, 99] { assert!(!mid(&ends(&turn(), false), &E::User { id: "x".into(), at })); assert!(!mid(&HashMap::new(), &E::User { id: "".into(), at })); }
        // a user entry whose id IS in ends (and one that is not): an impl reading ends.contains_key(id) for users must fail here
        let e = ends(&turn(), false); assert!(e.contains_key("a2"));
        assert!(!mid(&e, &E::User { id: "a2".into(), at: 3 })); assert!(!mid(&ends(&turn(), true), &E::User { id: "a2".into(), at: 3 }));
        let e2 = ends(&turn(), false); assert!(mid(&e2, &a("a1")) && !mid(&e2, &E::User { id: "a1".into(), at: 2 }));
    }
    #[test] fn stamp_mid_inside_turn() { assert!(mid(&ends(&turn(), false), &a("a1"))); }
    #[test] fn stamp_mid_turn_end_shows() { assert!(!mid(&ends(&turn(), false), &a("a2"))); }
    #[test] fn stamp_mid_running_newest_shows() { assert!(!mid(&ends(&turn(), true), &a("a2"))); }
    // not named by a law but fixes the table: ends map a turn's last reply to its start, 1 when the page lacks the start, 0 while running
    #[test] fn ends_table() {
        let m = ends(&turn(), false); assert_eq!(m.get("a2"), Some(&1)); assert_eq!(m.len(), 1);
        let mut two = turn(); two.push(E::User { id: "u2".into(), at: 10 }); two.push(a("a3"));
        let m = ends(&two, false); assert_eq!(m.get("a2"), Some(&1)); assert_eq!(m.get("a3"), Some(&10));
        assert_eq!(ends(&turn(), true).get("a2"), Some(&0));
        let m = ends(&[E::User { id: "u".into(), at: 5 }, a("r"), a("x:before")], false); assert_eq!(m.get("r"), Some(&5));
    }
}

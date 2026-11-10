//! Settle, snooze and the shelf a thread sits on: src/core/model "Settle rule", "Snooze" and `Shelf.of`/`Shelf.at`
//! in Rust. code stays as the differential oracle; each law it guards has a test here with the law's name.
use crate::state::{Override, State, Thread, Turn};

/// Settle.window: three days
pub const SETTLE_WINDOW: u64 = 259_200;

/// Thread.busy: a turn runs, or something waits on the user
pub fn busy(turn: Turn, pending: u64) -> bool { turn == Turn::Running || pending > 0 }

/// Settle.stale: inactivity alone settles a thread whose last activity was at `last`
pub fn stale(now: u64, last: u64, window: u64) -> bool { now >= last.saturating_add(window) }

/// Settle.auto.k
pub fn auto_k(busy: bool, archived: bool, o: Override, stale: bool) -> bool { !busy && !archived && o == Override::None && stale }

/// Settle.auto: whether the server's settle tick settles this thread now
pub fn auto(now: u64, window: u64, t: &Thread) -> bool { auto_k(busy(t.turn, t.pending), t.archived, t.override_, stale(now, t.activity, window)) }

/// Snooze.on: a snooze lasts while now < until (until 0: never snoozed)
pub fn snooze_on(now: u64, until: u64) -> bool { !(now >= until) }

/// Settle.auto.at: the settle tick's rule with the thread's snooze; a snoozed thread never auto-settles, and its window starts over from the wake time
pub fn auto_at(now: u64, window: u64, t: &Thread, until: u64) -> bool {
    if snooze_on(now, until) { false } else { stale(now, until, window) && auto(now, window, t) }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shelf { Pinned, Active, Settled, Hidden, Snoozed }

/// Shelf.of.k
pub fn shelf_of_k(deleted: bool, archived: bool, pinned: bool, o: Override, settled: bool) -> Shelf {
    if deleted || archived { return Shelf::Hidden }
    match o {
        Override::ForcedSettled => Shelf::Settled,
        _ if pinned => Shelf::Pinned,
        Override::ForcedActive => Shelf::Active,
        Override::None => if settled { Shelf::Settled } else { Shelf::Active },
    }
}
/// Shelf.of: settled_at > unsettled_at marks a settle the tick or the person recorded
pub fn shelf_of(t: &Thread) -> Shelf { shelf_of_k(t.deleted, t.archived, t.pinned, t.override_, t.settled_at > t.unsettled_at) }

/// Thread.hidden
pub fn hidden(t: &Thread) -> bool { t.archived || t.deleted }

/// Shelf.at: a snoozed thread sits on Snoozed (above settled, whatever its pin or settle state) until the clock ends the snooze
pub fn shelf_at(t: &Thread, until: u64, now: u64) -> Shelf { let sh = shelf_of(t); if !hidden(t) && snooze_on(now, until) { Shelf::Snoozed } else { sh } }

/// Sidebar.split.at: every thread lands on exactly one shelf, snoozed ones included. Threads keep their input order within a shelf
/// reversed, as cons-list build leaves them (Shelves.put prepends while recursing from the tail, so each shelf keeps input order).
pub struct Shelves<'a> { pub pinned: Vec<&'a Thread>, pub active: Vec<&'a Thread>, pub settled: Vec<&'a Thread>, pub hidden: Vec<&'a Thread>, pub snoozed: Vec<&'a Thread> }
impl<'a> Shelves<'a> {
    pub fn count(&self) -> usize { self.pinned.len() + self.active.len() + self.settled.len() + self.hidden.len() + self.snoozed.len() }
}
pub fn split_at<'a>(ts: &[(&str, &'a Thread)], snoozes: &std::collections::HashMap<String, u64>, now: u64) -> Shelves<'a> {
    let mut s = Shelves { pinned: vec![], active: vec![], settled: vec![], hidden: vec![], snoozed: vec![] };
    for (id, t) in ts {
        let until = snoozes.get(*id).copied().unwrap_or(0);
        match shelf_at(t, until, now) { Shelf::Pinned => s.pinned.push(t), Shelf::Active => s.active.push(t), Shelf::Settled => s.settled.push(t), Shelf::Hidden => s.hidden.push(t), Shelf::Snoozed => s.snoozed.push(t) }
    }
    s
}
/// Sidebar.split: no snoozes
pub fn split<'a>(ts: &[(&str, &'a Thread)]) -> Shelves<'a> { split_at(ts, &std::collections::HashMap::new(), 0) }

impl State {
    /// the threads of a project as (id, thread), in id order, for the sidebar
    pub fn project_threads(&self, project: &str) -> Vec<(&str, &Thread)> {
        let mut v: Vec<(&str, &Thread)> = self.threads.iter().filter(|(_, t)| t.project == project).map(|(i, t)| (i.as_str(), t)).collect();
        v.sort_by(|a, b| a.0.cmp(b.0)); v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // a tiny deterministic generator: the laws are universal over Nat/Bool/enums, so these sweep a grid and a pseudo-random sample
    struct Rng(u64);
    impl Rng { fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 } fn below(&mut self, n: u64) -> u64 { self.next() % n } fn bool(&mut self) -> bool { self.next() & 1 == 1 } }
    const TURNS: [Turn; 5] = [Turn::Idle, Turn::Running, Turn::Completed, Turn::Interrupted, Turn::Failed];
    const OVERRIDES: [Override; 3] = [Override::None, Override::ForcedSettled, Override::ForcedActive];
    fn thread(r: &mut Rng) -> Thread {
        Thread { project: "p".into(), title: String::new(), created: r.below(1000), activity: r.below(1000), turn: TURNS[r.below(5) as usize], pending: r.below(3), archived: r.bool(),
                 override_: OVERRIDES[r.below(3) as usize], settled_at: r.below(50), unsettled_at: r.below(50), pinned: r.bool(), pinned_at: 0, worktree_mode: false, branch: String::new(),
                 worktree: String::new(), provider: String::new(), model: String::new(), session: String::new(), deleted: r.bool() }
    }

    #[test] fn settle_at_window() { let mut r = Rng(1); for _ in 0..2000 { let (l, w) = (r.below(1 << 40), r.below(1 << 30)); assert!(stale(l + w, l, w)); } }
    #[test] fn settle_fresh() { let mut r = Rng(2); for _ in 0..2000 { let (n, w) = (r.below(1 << 40), r.below(1 << 30)); assert!(!stale(n, n, 1 + w)); } }
    #[test] fn settle_monotone() {
        let mut r = Rng(3);
        for _ in 0..5000 { let (n, l, w, d) = (r.below(1000), r.below(1000), r.below(500), r.below(500)); if stale(n, l, w) { assert!(stale(n + d, l, w)); } }
    }
    #[test] fn settle_never_running() { let mut r = Rng(4); for _ in 0..2000 { let (n, w, p) = (r.below(1000), r.below(500), r.below(5)); for a in [false, true] { for o in OVERRIDES { assert!(!auto_k(busy(Turn::Running, p), a, o, stale(n, n, w))); } } } }
    #[test] fn settle_never_pending() { for t in TURNS { for p in 1..4 { for a in [false, true] { for o in OVERRIDES { for s in [false, true] { assert!(!auto_k(busy(t, p), a, o, s)); } } } } } }
    #[test] fn settle_respects_unsettle() { for b in [false, true] { for a in [false, true] { for s in [false, true] { assert!(!auto_k(b, a, Override::ForcedActive, s)); } } } }
    #[test] fn settle_never_snoozed() {
        let mut r = Rng(5);
        for _ in 0..5000 { let t = thread(&mut r); let (now, w) = (r.below(1000), r.below(500)); let until = now + 1 + r.below(100); assert!(!(now >= until)); assert!(!auto_at(now, w, &t, until)); }
    }
    #[test] fn snooze_wakes() { let mut r = Rng(6); for _ in 0..2000 { let (n, u) = (r.below(1000), r.below(1000)); if n >= u { assert!(!snooze_on(n, u)); } } }
    #[test] fn sidebar_split_count() {
        let mut r = Rng(7);
        for _ in 0..200 { let n = r.below(30) as usize; let ts: Vec<Thread> = (0..n).map(|_| thread(&mut r)).collect(); let ids: Vec<String> = (0..n).map(|i| format!("t{i}")).collect();
            let pairs: Vec<(&str, &Thread)> = ids.iter().map(|s| s.as_str()).zip(ts.iter()).collect(); assert_eq!(split(&pairs).count(), n); }
    }
    #[test] fn sidebar_split_at_count() {
        let mut r = Rng(8);
        for _ in 0..200 { let n = r.below(30) as usize; let ts: Vec<Thread> = (0..n).map(|_| thread(&mut r)).collect(); let ids: Vec<String> = (0..n).map(|i| format!("t{i}")).collect();
            let mut snoozes: HashMap<String, u64> = HashMap::new(); for i in &ids { if r.bool() { snoozes.insert(i.clone(), r.below(2000)); } } let now = r.below(2000);
            let pairs: Vec<(&str, &Thread)> = ids.iter().map(|s| s.as_str()).zip(ts.iter()).collect(); assert_eq!(split_at(&pairs, &snoozes, now).count(), n); }
    }
    // Shelf.of / Shelf.at are not named by a law but fix the table the sidebar rests on
    #[test] fn shelf_of_table() {
        use Override::*;
        assert_eq!(shelf_of_k(true, false, true, ForcedActive, false), Shelf::Hidden);
        assert_eq!(shelf_of_k(false, true, false, None, false), Shelf::Hidden);
        assert_eq!(shelf_of_k(false, false, true, ForcedSettled, false), Shelf::Settled);
        assert_eq!(shelf_of_k(false, false, true, None, true), Shelf::Pinned);
        assert_eq!(shelf_of_k(false, false, false, ForcedActive, true), Shelf::Active);
        assert_eq!(shelf_of_k(false, false, false, None, true), Shelf::Settled);
        assert_eq!(shelf_of_k(false, false, false, None, false), Shelf::Active);
    }
    #[test] fn a_snooze_puts_a_visible_thread_on_its_own_shelf_and_the_clock_ends_it() {
        let mut r = Rng(9); let mut t = thread(&mut r); t.archived = false; t.deleted = false;
        assert_eq!(shelf_at(&t, 100, 50), Shelf::Snoozed); assert_eq!(shelf_at(&t, 100, 100), shelf_of(&t)); assert_eq!(shelf_at(&t, 0, 0), shelf_of(&t));
        t.archived = true; assert_eq!(shelf_at(&t, 100, 50), Shelf::Hidden);
    }
}

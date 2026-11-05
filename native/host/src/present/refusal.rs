//! Content-free diagnostics. One line per fixed refusal label for this process, never per request/thread.
use std::{collections::HashSet,sync::{LazyLock,Mutex}};
#[derive(Default)]
struct Once { seen: HashSet<&'static str> }
impl Once { fn first(&mut self,reason: &'static str) -> bool { self.seen.insert(reason) } }
static LOGGED: LazyLock<Mutex<Once>>=LazyLock::new(||Mutex::new(Once::default()));
pub fn report(reason: &'static str) {
    if LOGGED.lock().unwrap_or_else(|e|e.into_inner()).first(reason) { eprintln!("thread screen: not ported ({reason})"); }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_distinct_refusal_logs_once_without_a_per_thread_ledger() {
        let mut once=Once::default();
        assert!(once.first("thread.model"));
        for _ in 0..1000 { assert!(!once.first("thread.model")); }
        assert!(once.first("row.attachment")); assert_eq!(once.seen.len(),2);
    }
}

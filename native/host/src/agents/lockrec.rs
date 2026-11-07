//! A named lock's record (docs/locks.md): pure, shared with the presenter (present/panels.rs). The hub's operations are locks.rs.
use serde_json::{json, Value};

const WAITING: &str = "waiting for lock";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Party { pub id: String, pub kind: String, pub thread: String, pub note: String, pub since: u64, pub lease: u64 }
impl Party {
    fn read(o: &Value) -> Party { let t = |k: &str| o[k].as_str().unwrap_or("").to_string(); Party { id: t("id"), kind: o["kind"].as_str().unwrap_or("thread").to_string(), thread: t("thread"), note: t("note"), since: o["since"].as_u64().unwrap_or(0), lease: o["lease"].as_u64().unwrap_or(0) } }
    fn json(&self) -> Value { json!({"id":self.id,"kind":self.kind,"thread":self.thread,"note":self.note,"since":self.since,"lease":self.lease}) }
}
/// A single holder slot makes exclusivity structural; waiters keep FIFO order. `ready`: the holder has its flock and has been granted. `outside`: who holds the flock outside the hub.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lock { pub holder: Option<Party>, pub waiters: Vec<Party>, pub ready: bool, pub outside: Option<Value> }
impl Lock {
    pub fn read(text: &str) -> Lock {
        let o: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        Lock { holder: Some(&o["holder"]).filter(|h| h.is_object()).map(Party::read), waiters: o["waiters"].as_array().map(|a| a.iter().map(Party::read).collect()).unwrap_or_default(), ready: o["ready"].as_bool().unwrap_or(false), outside: Some(o["outside"].clone()).filter(Value::is_object) }
    }
    pub fn json(&self) -> Value {
        let mut v = json!({"holder":self.holder.as_ref().map_or(Value::Null, Party::json),"waiters":self.waiters.iter().map(Party::json).collect::<Vec<_>>(),"ready":self.ready});
        if let Some(o) = &self.outside { v["outside"] = o.clone() } v
    }
    pub fn owner(&self) -> &str { self.holder.as_ref().map_or("", |p| p.id.as_str()) }
    /// One-based place in the queue, 0 for none.
    pub fn position(&self, id: &str) -> usize { self.waiters.iter().position(|p| p.id == id).map_or(0, |i| i + 1) }
    pub fn has(&self, id: &str) -> bool { self.owner() == id || self.position(id) > 0 }
    pub fn idle(&self) -> bool { self.holder.is_none() && self.waiters.is_empty() }
    /// A party already in keeps its place, timestamp and note. An empty lock's holder is not ready until its grant.
    pub fn acquire(&mut self, p: Party) { if self.has(&p.id) { return } if self.idle() { self.holder = Some(p); self.ready = false; self.outside = None } else { self.waiters.push(p) } }
    /// The next in line takes the slot, not ready: its grant is the caller's next step.
    pub fn promote(&mut self) { self.ready = false; self.outside = None; self.holder = if self.waiters.is_empty() { None } else { Some(self.waiters.remove(0)) } }
    pub fn release(&mut self, id: &str) -> bool { if id.is_empty() || self.owner() != id { return false } self.promote(); true }
    pub fn cancel(&mut self, id: &str) -> bool { let n = self.waiters.len(); self.waiters.retain(|p| p.id != id); self.waiters.len() != n }
    pub fn answer(&self, id: &str) -> Value { json!({"held":self.owner() == id && self.ready,"position":self.position(id),"holder":self.owner()}) }
}
/// Names contain 1 to 64 letters, digits, hyphens or underscores: one is a file name too.
pub fn name_ok(n: &str) -> bool { !n.is_empty() && n.len() <= 64 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') }
/// Lock.ordinal
pub fn ordinal(n: usize) -> String { let (l, t) = (n % 10, n % 100); format!("{n}{}", if (11..=13).contains(&t) { "th" } else if l == 1 { "st" } else if l == 2 { "nd" } else if l == 3 { "rd" } else { "th" }) }
/// Lock.wait.word: what a queued job's row says; position 0 is the head, waiting for a flock held outside.
pub fn wait_word(name: &str, position: usize) -> String { format!("{WAITING} {name} ({})", if position == 0 { "OS flock".to_string() } else { ordinal(position) }) }
pub fn queued_row(doing: &str) -> bool { doing.starts_with(WAITING) }
/// Lock.lease.expired
pub fn lease_expired(lease: u64, now: u64) -> bool { lease > 0 && now >= lease }
pub fn age(now: u64, then: u64) -> String { let d = now.saturating_sub(then); if d < 60 { format!("{d}s") } else if d < 3600 { format!("{}m", d / 60) } else { format!("{}h", d / 3600) } }

#[cfg(test)]
mod tests {
    use super::*;
    fn p(id: &str) -> Party { Party { id: id.into(), kind: "thread".into(), thread: id.into(), ..Default::default() } }
    #[test] fn one_holder_and_a_fifo_queue() {
        let mut l = Lock::default(); l.acquire(p("a")); l.acquire(p("b")); l.acquire(p("c")); l.acquire(p("b"));
        assert_eq!((l.owner(), l.position("b"), l.position("c"), l.waiters.len()), ("a", 1, 2, 2), "a repeat keeps its place");
        assert!(!l.release("b") && l.owner() == "a", "only the holder releases"); assert!(!l.release(""));
        l.ready = true; assert!(l.release("a")); assert_eq!((l.owner(), l.ready, l.position("c")), ("b", false, 1), "the next is holder, not yet granted");
        assert!(l.cancel("c") && !l.cancel("c")); assert!(l.release("b")); assert!(l.idle());
    }
    #[test] fn a_record_reads_back_as_written() {
        let mut l = Lock::default(); l.acquire(Party { id: "w-1".into(), kind: "work".into(), thread: "t1".into(), note: "x".into(), since: 5, lease: 9 }); l.acquire(p("b")); l.ready = true; l.outside = Some(json!({"pid":7,"cmd":"flock","since":3}));
        assert_eq!(Lock::read(&l.json().to_string()), l); assert_eq!(Lock::read(""), Lock::default());
        assert_eq!(Lock::read(&l.json().to_string()).answer("b"), json!({"held":false,"position":1,"holder":"w-1"}));
    }
    #[test] fn names_and_words() {
        assert!(name_ok("build-1_x") && name_ok(&"a".repeat(64))); assert!(!name_ok("") && !name_ok(&"a".repeat(65)) && !name_ok("a/b") && !name_ok("a b") && !name_ok("../x"));
        assert_eq!([1, 2, 3, 4, 11, 12, 13, 21, 22, 101, 111].map(ordinal).join(" "), "1st 2nd 3rd 4th 11th 12th 13th 21st 22nd 101st 111th");
        assert_eq!(wait_word("b", 2), "waiting for lock b (2nd)"); assert_eq!(wait_word("b", 0), "waiting for lock b (OS flock)"); assert!(queued_row(&wait_word("b", 1)));
        assert!(lease_expired(5, 5) && !lease_expired(6, 5) && !lease_expired(0, 5));
    }
}

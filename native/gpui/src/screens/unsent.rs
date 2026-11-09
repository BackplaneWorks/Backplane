//! The window's own outbox: messages sent while this window has no connection to its hub. The hub decides everything it can, but it cannot
//! answer while it is out of reach, so this is the one queue the client keeps: each message waits here (on disk, beside the drafts), shows in the
//! queue strip as an Unsent row with Edit and Remove, and goes as a `turn.start` (the same message id every try, which the hub posts once) one
//! after another, oldest first, once the window is connected again. Pure, apart from the file.
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Item { pub thread: String, pub text: String, pub msg: String, /** on its way: its answer is awaited */ #[serde(default)] pub out: bool, /** the hub said no */ #[serde(default)] pub why: String }

#[derive(Default)]
pub struct Unsent { pub items: Vec<Item>, path: Option<PathBuf>, seq: u64, /** the connection has its hub's greeting (requests go out) */ pub live: bool }
/// The tag an Ask for an unsent message carries (above every other screen's).
pub const TAG: u64 = 0xF0_0000_0000;
impl Unsent {
    pub fn open(path: PathBuf) -> Self {
        let mut items: Vec<Item> = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        for i in &mut items { i.out = false } // what was on its way when the window closed goes again (the hub posts one id once)
        Self { items, path: Some(path), seq: 0, live: false }
    }
    fn save(&self) { if let Some(p) = &self.path { let _ = std::fs::write(p, serde_json::to_vec(&self.items).unwrap_or_default()); } }
    /// Keep a message for `thread`; its id is made here and kept for every try.
    pub fn push(&mut self, thread: &str, text: &str, now_ms: u64) -> String {
        self.seq += 1; let msg = format!("u{now_ms}-{}", self.seq);
        self.items.push(Item { thread: thread.into(), text: text.into(), msg: msg.clone(), out: false, why: String::new() }); self.save(); msg
    }
    /// Edit or Remove: out of the outbox; its text.
    pub fn remove(&mut self, msg: &str) -> Option<String> { let i = self.items.iter().position(|x| x.msg == msg)?; let t = self.items.remove(i).text; self.save(); Some(t) }
    /// The next to send now (connected): the oldest not refused, when none is on its way. It is marked on its way.
    pub fn next(&mut self) -> Option<Item> {
        if self.items.iter().any(|x| x.out) { return None }
        let i = self.items.iter_mut().find(|x| x.why.is_empty())?; i.out = true; let r = i.clone(); self.save(); Some(r)
    }
    /// The hub's answer to `msg`: taken (it goes), or refused (it stays with why, for the person to edit or remove).
    pub fn answered(&mut self, msg: &str, ok: bool, why: &str) {
        let Some(i) = self.items.iter().position(|x| x.msg == msg) else { return };
        if ok { self.items.remove(i); } else { self.items[i].out = false; self.items[i].why = if why.is_empty() { "refused".into() } else { why.into() }; }
        self.save();
    }
    /// The answer to the one on its way.
    pub fn answered_out(&mut self, ok: bool, why: &str) { if let Some(m) = self.items.iter().find(|x| x.out).map(|x| x.msg.clone()) { self.answered(&m, ok, why) } }
    /// The connection went: what was on its way goes again.
    pub fn lost(&mut self) { self.live = false; for i in &mut self.items { i.out = false } }
    /// The strip's rows for a thread, in the hub's row shape (screens/queue.rs): tag Unsent (or Refused, with why), Edit and Remove of this window's own (`u-edit`, `u-remove`).
    pub fn rows(&self, thread: &str) -> Vec<Value> {
        let first = |t: &str| t.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(200).collect::<String>();
        self.items.iter().filter(|x| x.thread == thread).map(|x| json!({"msg": x.msg, "tag": if x.why.is_empty() { "Unsent" /* i18n-ok: a tag value the unsent card translates */ } else { "Refused" },
            "text": if x.why.is_empty() { first(&x.text) } else { format!("{} ({})", first(&x.text), x.why) },
            /* i18n-ok: button labels the strip draws through trs */ "buttons": [{"label": "Edit", "action": "u-edit", "value": x.msg}, {"label": "Remove", "action": "u-remove", "value": x.msg}]})).collect()
    }
    /// The request that sends one (as the composer's Send does: mode steer, into a running turn at once).
    /// (`thread` is the window's key, `<hub>|<id>`: the hub is asked about its own id.)
    pub fn request(i: &Item) -> Value { json!({"thread": i.thread.rsplit('|').next().unwrap_or(""), "text": i.text, "msg": i.msg, "mode": "steer"}) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn unsent_messages_wait_show_and_go_one_at_a_time_in_order() {
        let dir = std::env::temp_dir().join(format!("bp-unsent-{}", std::process::id())); let _ = std::fs::create_dir_all(&dir); let f = dir.join("u.json"); let _ = std::fs::remove_file(&f);
        let mut u = Unsent::open(f.clone());
        let a = u.push("h|t1", "first\nmore", 5); let b = u.push("h|t1", "second", 5); let c = u.push("t2", "other", 5);
        let r = u.rows("h|t1"); assert_eq!(r.len(), 2); assert_eq!((r[0]["tag"].as_str(), r[0]["text"].as_str()), (Some("Unsent"), Some("first")));
        assert_eq!(r[1]["buttons"], json!([{"label":"Edit","action":"u-edit","value":b},{"label":"Remove","action":"u-remove","value":b}]));
        assert_eq!(u.remove(&b).as_deref(), Some("second"), "Edit and Remove take it out; Edit puts this back in the draft");
        let n = u.next().unwrap(); assert_eq!(n.msg, a); assert_eq!(Unsent::request(&n), json!({"thread":"t1","text":"first\nmore","msg":a,"mode":"steer"}));
        assert!(u.next().is_none(), "one at a time");
        u.lost(); assert_eq!(u.next().unwrap().msg, a, "a send cut off goes again, with the same id");
        u.answered(&a, true, ""); assert_eq!(u.next().unwrap().msg, c);
        u.answered(&c, false, "unknown thread"); assert_eq!(u.rows("t2")[0]["text"], "other (unknown thread)"); assert!(u.next().is_none(), "a refused one is not sent again");
        let again = Unsent::open(f); assert_eq!(again.items.len(), 1, "kept on disk"); assert_eq!(again.items[0].why, "unknown thread");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

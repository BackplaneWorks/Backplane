//! Alert delivery: when an `alert.<thread>` record with a kind is committed,
//! the webhook (settings::webhook) is posted, and the desk used last gets `{"t":"notify"}` as a desktop notification, unless a focused desk in use
//! is showing the thread and the kind is one a watching desk holds (ask, done, fail, merged: never a note or a reminder).
//! A desk (the GPUI window, the web page) says where it is with `desk.look {thread, focused}` on connect, on a change and every `BEAT_S` seconds
//! while in use. The same record is a phone push of its own unless a focused desk in use shows the thread (0.14's rule; push/mod.rs `alert` holds the push policy);
//! the uberbot's own orders reach the phones as before. Not ported here: the Live Activity, and the owner's other machines
//! (`Far.note.*`, the net lane). The body of a done/fail alert with no text of its own is "Finished"/"Failed".
use crate::{api::{Caller, Fut, Observer, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex}};

/// Desk.window: how long a focused desk counts as watching with no input (seconds).
pub const WINDOW_S: u64 = 180;
/// Desk.beat: how often a desk in use says its look again.
pub const BEAT_S: u64 = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Look { pub thread: String, pub focused: bool, pub at: u64 }
impl Look {
    /// Look.sees: a focused desk showing the thread, heard from inside the window.
    pub fn sees(&self, thread: &str, now: u64) -> bool { self.focused && self.thread == thread && now < self.at + WINDOW_S }
}
/// Desk.seen over every desk.
pub fn seen(desks: &[(u64, Look)], thread: &str, now: u64) -> bool { desks.iter().any(|(_, l)| l.sees(thread, now)) }
/// What a phone says of an alert (0.14's Push.alert.body): the record's own text (an ask, a merge, a note), else the last reply's first line, "Finished" when there is
/// none, "Failed" for a failure.
pub fn phone_body(kind: &str, text: &str, last: &str) -> String {
    if matches!(kind, "ask" | "merged") || !text.is_empty() { return body(kind, text) }
    if kind == "fail" { return "Failed".into() }
    let line: String = last.trim().lines().next().unwrap_or("").trim().chars().take(180).collect();
    if line.is_empty() { "Finished".into() } else { line }
}
/// Desk.lead: the desk used last.
pub fn lead(desks: &[(u64, Look)]) -> Option<u64> { desks.iter().fold(None::<(u64, u64)>, |best, (c, l)| match best { Some((_, at)) if l.at <= at => best, _ => Some((*c, l.at)) }).map(|(c, _)| c) }
/// Alert.holds: kinds a watching desk holds.
pub fn holds(kind: &str) -> bool { matches!(kind, "ask" | "done" | "fail" | "merged") }
/// The record `kind\tkey\tsettled\tstamp\ttext`.
pub fn parse(v: &str) -> (String, String, String) { let mut f = v.splitn(5, '\t'); let kind = f.next().unwrap_or("").to_string(); let key = f.next().unwrap_or("").to_string(); let text = f.nth(2).unwrap_or("").to_string(); (kind, key, text) }
/// Push.alert.body.k without the last reply's line: a note, an ask or a merge says its own text.
pub fn body(kind: &str, text: &str) -> String {
    match kind {
        "merged" => format!("Merged: {}", if text.is_empty() { "its branch" } else { text }),
        "ask" => if text.is_empty() { "Needs you".into() } else { format!("Needs you: {text}") },
        "fail" => if text.is_empty() { "Failed".into() } else { text.into() },
        _ => if text.is_empty() { "Finished".into() } else { text.lines().next().unwrap_or("").chars().take(180).collect() },
    }
}
/// Desk.notify
pub fn frame(thread: &str, title: &str, body: &str, kind: &str, key: &str) -> Value { json!({"t":"notify","thread":thread,"title":title,"body":body,"kind":kind,"key":key}) }

/// The clear frame a desk gets when the need it was told about is answered: it closes that notification.
pub fn clear_frame(thread: &str, key: &str) -> Value { json!({"t":"notify.clear","thread":thread,"key":key}) }

#[derive(Default)]
pub struct Desks { looks: Mutex<HashMap<u64, Look>>, needs: Mutex<Needs> }
/// What a desk was told about asks: the open asks' threads, and per thread the desk and key of its ask notification (bounded).
#[derive(Default)]
pub struct Needs { open: HashMap<String, String>, told: HashMap<String, (u64, String)> }
impl Needs {
    pub fn opened(&mut self, id: &str, thread: &str) { if self.open.len() > 4096 { self.open.clear() } self.open.insert(id.into(), thread.into()); }
    pub fn told(&mut self, thread: &str, client: u64, key: &str) { if self.told.len() > 4096 { self.told.clear() } self.told.insert(thread.into(), (client, key.into())); }
    /// An ask closed: the desk to clear and the key, once its thread has no other ask open.
    pub fn closed(&mut self, id: &str) -> Option<(String, u64, String)> {
        let thread = self.open.remove(id)?;
        if self.open.values().any(|t| *t == thread) { return None }
        self.told.remove(&thread).map(|(c, k)| (thread, c, k))
    }
    /// An open ask of the thread, if one is known (the phone's alert is cleared when it closes).
    pub fn ask_of(&self, thread: &str) -> String { self.open.iter().filter(|(_, t)| *t == thread).map(|(id, _)| id.clone()).max().unwrap_or_default() }
}
impl Desks {
    pub fn new() -> Arc<Desks> { Arc::new(Desks::default()) }
    /// The looks of desks still connected.
    pub fn live(&self, hub: &Hub) -> Vec<(u64, Look)> { let mut m = self.looks.lock().unwrap(); m.retain(|c, _| hub.is_attached(*c)); let mut v: Vec<_> = m.iter().map(|(c, l)| (*c, l.clone())).collect(); v.sort_by_key(|(c, _)| *c); v }
    pub fn say(&self, client: u64, thread: &str, focused: bool, at: u64) { self.looks.lock().unwrap().insert(client, Look { thread: thread.chars().take(200).collect(), focused, at }); }
}

pub fn start(hub: &Arc<Hub>) -> Arc<Desks> {
    let d = Desks::new();
    hub.register_rpc(Arc::new(DeskRpc(d.clone())));
    hub.register_observer(Arc::new(Alerts(d.clone())));
    d
}

struct DeskRpc(Arc<Desks>);
impl RpcHandler for DeskRpc {
    fn methods(&self) -> &'static [&'static str] { &["desk.look"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            self.0.say(who.client, p.get("thread").and_then(Value::as_str).unwrap_or(""), p.get("focused").and_then(Value::as_bool).unwrap_or(false), now_ms() / 1000);
            vec![json!({"t":"reply","id":id,"ok":true})]
        })
    }
}

struct Alerts(Arc<Desks>);
impl Observer for Alerts {
    fn committed(&self, hub: &Arc<Hub>, events: &[Value], _first: u32) {
        for e in events {
            // a need answered: the desk that was told about it closes its notification
            if e["$"] == "AskOpened" { if let (Some(id), Some(th)) = (e["id"].as_str(), e["thread"].as_str()) { self.0.needs.lock().unwrap().opened(id, th); } continue }
            if e["$"] == "AskClosed" { if let Some((th, c, key)) = e["id"].as_str().and_then(|id| self.0.needs.lock().unwrap().closed(id)) { hub.push(c, clear_frame(&th, &key)); } continue }
            if e["$"] != "SettingSet" { continue }
            let Some(thread) = e["key"].as_str().and_then(|k| k.strip_prefix("alert.")) else { continue };
            let Some(v) = e["value"].as_str() else { continue };
            let (kind, key, text) = parse(v);
            if kind.is_empty() { continue }
            let (hub, desks, thread) = (hub.clone(), self.0.clone(), thread.to_string());
            if tokio::runtime::Handle::try_current().is_err() { continue }
            tokio::spawn(async move {
                let title = { let m = hub.model.lock().await; m.threads.get(&thread).map(|t| t.title.clone()).unwrap_or_default() };
                let title = if title.is_empty() { "Backplane".to_string() } else { title };
                let text_body = body(&kind, &text);
                let now = now_ms() / 1000;
                let looks = desks.live(&hub);
                let watched = holds(&kind) && seen(&looks, &thread, now);
                if !watched { if let Some(c) = lead(&looks) { hub.push(c, frame(&thread, &title, &text_body, &kind, &key)); if kind == "ask" { desks.needs.lock().unwrap().told(&thread, c, &key); } } }
                // the phones: the same record is a push of its own unless a focused desk in use shows the thread (0.14's rule; push/mod.rs `alert`)
                if !watched {
                    let ask = if kind == "ask" { desks.needs.lock().unwrap().ask_of(&thread) } else { String::new() };
                    let last = if kind == "done" && text.is_empty() { let m = hub.model().await; m.page(&hub.store, &thread, 0, 8).ok().and_then(|p| p.rows.iter().find(|r| r.kind == "msg" && r.role == "assistant" && !r.text.trim().is_empty()).map(|r| r.text.clone())).unwrap_or_default() } else { String::new() };
                    crate::push::alert(&hub, &thread, &kind, &key, &phone_body(&kind, &text, &last), &ask, now).await;
                }
                let state = match kind.as_str() { "fail" => "failed", "done" => "completed", _ => "" };
                let _ = crate::settings::webhook::deliver(&hub, &thread, &kind, state, "", "", &text_body, &title).await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn l(t: &str, f: bool, at: u64) -> Look { Look { thread: t.into(), focused: f, at } }
    #[test] fn a_focused_desk_in_use_watches_its_thread_only() {
        assert!(l("t1", true, 100).sees("t1", 200));
        assert!(!l("t1", true, 100).sees("t1", 280), "stale after the window");
        assert!(!l("t1", true, 100).sees("t2", 110));
        assert!(!l("t1", false, 100).sees("t1", 110), "no focus");
    }
    #[test] fn an_answered_need_clears_its_notification_once_the_thread_has_no_other_ask() {
        let mut n = Needs::default();
        n.opened("q1", "t1"); n.opened("q2", "t1"); n.opened("q3", "t2"); n.told("t1", 7, "ask-t1");
        assert_eq!(n.closed("q1"), None, "t1 still has q2 open: the notification stays");
        assert_eq!(n.closed("q2"), Some(("t1".into(), 7, "ask-t1".into())), "the last ask of t1 closed: desk 7 clears ask-t1");
        assert_eq!(n.closed("q3"), None, "no desk was told about t2");
        assert_eq!(n.closed("q2"), None, "cleared once");
        assert_eq!(clear_frame("t1", "ask-t1"), json!({"t":"notify.clear","thread":"t1","key":"ask-t1"}));
    }
    #[test] fn the_desk_used_last_leads() {
        assert_eq!(lead(&[(1, l("a", true, 10)), (2, l("b", false, 30)), (3, l("c", true, 20))]), Some(2));
        assert_eq!(lead(&[]), None);
    }
    #[test] fn a_phone_says_the_records_text_else_the_last_reply_as_014_did() {
        assert_eq!(phone_body("done", "", "Done: the board is routed.\nMore below."), "Done: the board is routed.");
        assert_eq!(phone_body("done", "", " "), "Finished"); assert_eq!(phone_body("fail", "", "half a reply"), "Failed");
        assert_eq!(phone_body("ask", "Bash rm x", "ignored"), "Needs you: Bash rm x"); assert_eq!(phone_body("note", "call mum", "x"), "call mum");
        assert_eq!(phone_body("merged", "", "x"), "Merged: its branch");
    }
    #[test] fn only_ask_done_fail_merged_are_held() { for k in ["ask", "done", "fail", "merged"] { assert!(holds(k)) } for k in ["note", "reminder", ""] { assert!(!holds(k)) } }
    #[test] fn the_record_and_its_body() {
        assert_eq!(parse("note\tk1\tm\t1.2\tcall mum"), ("note".into(), "k1".into(), "call mum".into()));
        assert_eq!(body("ask", "ok?"), "Needs you: ok?"); assert_eq!(body("done", ""), "Finished"); assert_eq!(body("fail", ""), "Failed"); assert_eq!(body("note", "call mum"), "call mum");
    }

    async fn recv_notify(o: &mut crate::hub::Outbox) -> Option<Value> {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(1500);
        while std::time::Instant::now() < end {
            match o.try_recv() { Ok(crate::hub::Out::Value(v)) if v["t"] == "notify" => return Some(v), Ok(_) => {}, Err(_) => tokio::time::sleep(std::time::Duration::from_millis(20)).await }
        }
        None
    }
    // end to end on a hub: the alert record reaches the desk used last, and a watched thread's done reaches nobody (a note always does)
    #[tokio::test(flavor = "multi_thread")] async fn an_alert_record_goes_to_the_desk_used_last_unless_a_desk_watches_the_thread() {
        let n = crate::net::peering::tests::node_extra("alerts", false, false, None).await;
        let hub = n.hub.clone(); let _d = start(&hub);
        hub.commit(&json!({"$":"ThreadCreated","id":"t1","project":"","title":"Bub","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
        let (mut c1, _) = hub.connect(false); let mut o1 = hub.attach(c1.client);
        let (mut c2, _) = hub.connect(false); let mut o2 = hub.attach(c2.client);
        let r = hub.request(&mut c1, &json!({"id":1,"m":"desk.look","p":{"thread":"t1","focused":true}})).await; assert_eq!(r[0]["ok"], true, "{r:?}");
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        hub.request(&mut c2, &json!({"id":1,"m":"desk.look","p":{"thread":"t9","focused":false}})).await;
        // t1 is watched by desk 1: a done alerts nobody
        hub.commit(&json!({"$":"SettingSet","key":"alert.t1","value":"done\tturn-t1\tm1\t1.1\t","at":2})).await.unwrap();
        assert!(recv_notify(&mut o2).await.is_none() && recv_notify(&mut o1).await.is_none(), "a watched thread's done is held");
        // a note is never held: it goes to desk 2, used last
        hub.commit(&json!({"$":"SettingSet","key":"alert.t1","value":"note\tk7\tm1\t1.2\tcall mum","at":3})).await.unwrap();
        let f = recv_notify(&mut o2).await.expect("the desk used last gets the note");
        assert_eq!((f["thread"].as_str(), f["title"].as_str(), f["body"].as_str(), f["kind"].as_str(), f["key"].as_str()), (Some("t1"), Some("Bub"), Some("call mum"), Some("note"), Some("k7")));
        assert!(recv_notify(&mut o1).await.is_none(), "only one desk is told");
        // a record with no kind (the quiet memory) tells nobody
        hub.commit(&json!({"$":"SettingSet","key":"alert.t1","value":"\t\tm2\t1.3\t","at":4})).await.unwrap();
        assert!(recv_notify(&mut o2).await.is_none());
        let _ = (&mut c1, &mut c2);
    }
}

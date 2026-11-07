//! wake_on / wake_list / wake_cancel.
//! A subscription is the log's `WakeSet`/`WakeDropped` (the state stream folds them as `wake.<id>`); what a firing has already
//! told is `wake.seen.<id>.<event>`. This module keeps its own fold of those three, tails the log, and on a thread's turn end or
//! opened ask tells the subscriber through `Agents::deliver_as`, like a launcher's report. Kinds served: `thread`. `work`, `pr` and `file`
//! are refused by name until their pollers exist (work waits for work.rs's rows to settle; PARITY lists the rest).
use super::{lines::hub_id, now_s, Agents};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex}};

/// A copy of a log wakes nothing: a thread ending there must not start or feed another (the same gate as work.rs).
pub const COPY_LINE: &str = "wake subscriptions are off on a copy of a log (BACKPLANE_COPY=1): nothing is subscribed, cancelled or woken here";
const KINDS: [&str; 4] = ["thread", "work", "pr", "file"];
const SERVED: [&str; 1] = ["thread"];

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Wake { pub id: String, pub by: String, pub kind: String, pub target: String, pub events: Vec<String>, pub once: bool, pub note: String, pub created: u64 }
impl Wake {
    fn json(&self) -> Value { json!({"id":self.id,"by":self.by,"kind":self.kind,"target":self.target,"events":self.events,"once":self.once,"note":self.note,"created":self.created}) }
}

/// Wake.defaults
pub fn defaults(kind: &str) -> Vec<String> { match kind { "pr" => vec!["merged".into(), "closed".into()], "file" => vec!["changed".into()], _ => vec!["end".into()] } }
/// Wake.allowed
pub fn allowed(kind: &str) -> &'static [&'static str] { match kind { "thread" => &["end", "done", "fail", "ask", "idle"], "work" => &["end"], "pr" => &["merged", "closed", "ci", "ci_fail"], "file" => &["exists", "changed"], _ => &[] } }

/// Wake.error; "" when the request is fine.
pub fn error(by: &str, kind: &str, target: &str, events: &[String]) -> String {
    if !KINDS.contains(&kind) { return "kind must be thread, work, pr or file".into() }
    if target.is_empty() || events.is_empty() || !events.iter().all(|e| allowed(kind).contains(&e.as_str())) { return "target and valid nonempty events are required".into() }
    if kind == "thread" && target.contains('~') { return "wake on a thread elsewhere is not supported yet".into() }
    if kind == "thread" && target == by { return "a subscriber cannot wake itself".into() }
    if kind == "file" && !crate::plat::fs::is_abs(target) { return "file target must be an absolute path".into() }
    if kind == "pr" && !pr_valid(target) { return "PR target must be owner/repo#N; bots must give owner/repo".into() }
    String::new()
}
fn pr_valid(t: &str) -> bool {
    let p: Vec<&str> = t.split('#').collect(); if p.len() != 2 { return false }
    let word = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'));
    let r: Vec<&str> = p[0].split('/').collect();
    r.len() == 2 && word(r[0]) && word(r[1]) && !p[1].is_empty() && p[1].chars().all(|c| c.is_ascii_digit()) && p[1] != "0"
}

/// Wake.note: one paragraph, whatever line breaks the reply held.
pub fn note(w: &Wake, text: &str, events: &[String]) -> String {
    let raw = format!("Wake {}: {} Events: {}.{}{}", w.id, text, events.join(", "),
        if w.note.is_empty() { String::new() } else { format!(" Note: {}.", w.note) },
        if w.once { String::new() } else { format!(" Cancel with wake_cancel {{id: {}}}.", w.id) });
    raw.chars().map(|c| if matches!(c, '\n' | '\r' | '\t') { ' ' } else { c }).collect()
}

/// What a turn end or an opened ask means for a subscription: (event, key) pairs, in order (end, done, fail, ask, idle).
/// `end`: 1 completed, 2 failed, 3 interrupted (0 none). `ask`: the ask's id and time when one opened.
pub fn signals(end: u8, at: u64, thread: &str, ask: Option<(&str, u64)>, idle: bool) -> Vec<(&'static str, String)> {
    let ek = format!("{thread}:{at}:{end}"); let mut v = vec![];
    if end > 0 { v.push(("end", ek.clone())) }
    if end == 1 { v.push(("done", ek.clone())) }
    if end == 2 { v.push(("fail", ek.clone())) }
    if let Some((id, t)) = ask { v.push(("ask", format!("{thread}:ask:{id}:{t}"))) }
    if end > 0 && idle { v.push(("idle", ek)) }
    v
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Book { wakes: HashMap<String, Wake>, seen: HashMap<String, String>, at: usize }
impl Book {
    fn path(store: &crate::store::Store) -> std::path::PathBuf { crate::ckpt::dir(store.path()).join("wakes.json") }
    fn load(store: &crate::store::Store) -> Option<Book> { let b = crate::ckpt::load_json(&Self::path(store), store, crate::ckpt::stamps::WAKES, |b: &Book| b.at); if let Some(b) = &b { crate::ckpt::saved("wakes", b.at);
        if crate::ckpt::check_on() { let (s, n, a) = (store.clone(), b.at, serde_json::to_value(b).unwrap_or_default()); std::thread::spawn(move || { let mut f = Book::default(); while f.at < n { let e = (f.at + 4096).min(n); if let Ok(ls) = s.range(f.at, e) { for l in ls { f.apply(&l) } } f.at = e; } crate::ckpt::check("wakes book", a, serde_json::to_value(&f).unwrap_or_default()) }); } } b }
    fn save(&self, store: &crate::store::Store) { if crate::ckpt::due("wakes", self.at) { crate::ckpt::save_json(&Self::path(store), store, self.at, crate::ckpt::stamps::WAKES, self); crate::ckpt::saved("wakes", self.at) } }
    fn apply(&mut self, line: &str) {
        let Some(t) = crate::session::event_type(line) else { return };
        if !matches!(t, "WakeSet" | "WakeDropped" | "SettingSet" | "ThreadDeleted") { return }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        match t {
            "WakeSet" => { let events = v.get("events").and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
                self.wakes.insert(g("id"), Wake { id: g("id"), by: g("by"), kind: g("kind"), target: g("target"), events, once: v.get("once").and_then(Value::as_bool).unwrap_or(true), note: g("note"), created: v.get("at").and_then(Value::as_u64).unwrap_or(0) }); }
            "WakeDropped" => { self.wakes.remove(&g("id")); }
            "SettingSet" => { let k = g("key"); if k.starts_with("wake.seen.") { self.seen.insert(k, g("value")); } }
            _ => {}
        }
    }
    pub fn on(&self, kind: &str, target: &str) -> Vec<&Wake> { let mut v: Vec<&Wake> = self.wakes.values().filter(|w| w.kind == kind && w.target == target).collect(); v.sort_by(|a, b| a.created.cmp(&b.created).then(a.id.cmp(&b.id))); v }
    pub fn by(&self, by: &str) -> Vec<&Wake> { let mut v: Vec<&Wake> = self.wakes.values().filter(|w| w.by == by).collect(); v.sort_by(|a, b| a.created.cmp(&b.created).then(a.id.cmp(&b.id))); v }
}

pub struct Wakes { ag: Arc<Agents>, book: Mutex<Book>, tailing: tokio::sync::Mutex<()> }

pub fn defs(names: &[&str]) -> Vec<Value> {
    let all: Value = serde_json::from_str(super::mcp::TOOLS_JSON).unwrap_or_default();
    all["result"]["tools"].as_array().cloned().unwrap_or_default().into_iter().filter(|t| names.contains(&t["name"].as_str().unwrap_or(""))).collect()
}

impl Wakes {
    /// Catch up with the log (no firing for what is already in it), register the tools and follow the log.
    pub async fn start(ag: Arc<Agents>) -> Arc<Wakes> {
        let w = Arc::new(Wakes { ag: ag.clone(), book: Mutex::new(Book::load(&ag.hub.store).unwrap_or_default()), tailing: tokio::sync::Mutex::new(()) });
        w.catch_up();
        { let me = w.clone(); crate::ckpt::register(move |store| me.book.lock().unwrap().save(store)); }
        ag.add_tools(Arc::new(WakeTools(w.clone())));
        if crate::copy::on() { return w } // the book is read for wake_list; nothing follows the log, nothing fires
        let me = w.clone(); let mut rx = ag.hub.subscribe();
        tokio::spawn(async move { loop {
            match rx.recv().await { Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => me.tail().await, Err(_) => return }
        } });
        w
    }
    fn catch_up(&self) {
        let store = &self.ag.hub.store; let total = store.len(); let mut b = self.book.lock().unwrap();
        while b.at < total { let e = (b.at + 4096).min(total); match store.range(b.at, e) { Ok(ls) => { let from = b.at; for (i, l) in ls.iter().enumerate() { store.fold(from + i, l, |l| b.apply(l)) } } Err(_) => break } b.at = e; }
    }
    /// New log lines: fold them, and fire for the turn ends and asks among them.
    async fn tail(&self) {
        if crate::copy::on() { return }
        let _one = self.tailing.lock().await; // one reader at a time: two would fire the same lines twice
        let store = &self.ag.hub.store; let total = store.len();
        let from = self.book.lock().unwrap().at; if from >= total { return }
        let lines = match store.range(from, total) { Ok(l) => l, Err(_) => return };
        let mut ends: Vec<(String, u8, u64, Option<(String, u64)>)> = vec![]; let mut gone: Vec<String> = vec![];
        for l in &lines {
            self.book.lock().unwrap().apply(l);
            let Some(t) = crate::session::event_type(l) else { continue };
            if !matches!(t, "TurnChanged" | "AskOpened" | "ThreadDeleted") { continue }
            let Ok(v) = serde_json::from_str::<Value>(l) else { continue };
            let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string(); let at = v.get("at").and_then(Value::as_u64).unwrap_or(0);
            match t {
                "TurnChanged" => { let e = match g("state").as_str() { "completed" => 1, "failed" => 2, "interrupted" => 3, _ => 0 }; if e > 0 { ends.push((g("thread"), e, at, None)) } }
                "AskOpened" => ends.push((g("thread"), 0, at, Some((g("id"), at)))),
                _ => gone.push(g("id")),
            }
        }
        self.book.lock().unwrap().at = total;
        for th in gone { let ids: Vec<String> = self.book.lock().unwrap().by(&th).iter().map(|w| w.id.clone()).collect(); if !ids.is_empty() { let at = now_s(); self.ag.commit_events(ids.into_iter().map(|id| json!({"$":"WakeDropped","id":id,"at":at})).collect()).await } }
        for (th, end, at, ask) in ends { self.fire(&th, end, at, ask).await }
    }
    async fn fire(&self, th: &str, end: u8, at: u64, ask: Option<(String, u64)>) {
        if crate::guard::safe() { return } // the wake stays in the book for the normal start that follows
        if crate::copy::on() { return }
        let subs: Vec<Wake> = self.book.lock().unwrap().on("thread", th).into_iter().cloned().collect(); if subs.is_empty() { return }
        let (title, reply, idle, live, asked) = { let w = self.ag.world().await;
            let t = w.threads.get(th); let idle = end > 0 && w.ready(th) && !w.running(th);
            let asked = ask.as_ref().map(|(id, _)| w.asks.get(id).map(|a| { let d: String = a.detail.chars().take(600).collect(); let h = if a.tool.is_empty() { a.kind.clone() } else { format!("{} ({})", a.kind, a.tool) }; if d.is_empty() { h } else { format!("{h}: {d}") } }).unwrap_or_default());
            (t.map(|t| t.title.clone()).unwrap_or_default(), t.map(|t| t.last_assistant.chars().take(1500).collect::<String>()).unwrap_or_default(), idle, subs.iter().map(|s| (s.id.clone(), w.threads.get(&s.by).map_or(false, |t| !t.deleted))).collect::<HashMap<_, _>>(), asked) };
        let sig = signals(end, at, th, ask.as_ref().map(|(i, t)| (i.as_str(), *t)), idle);
        for s in subs {
            if !live.get(&s.id).copied().unwrap_or(false) { continue }
            let matches: Vec<&(&'static str, String)> = sig.iter().filter(|(e, k)| s.events.iter().any(|x| x == e) && self.book.lock().unwrap().seen.get(&format!("wake.seen.{}.{e}", s.id)).map(String::as_str) != Some(k.as_str())).collect();
            if matches.is_empty() { continue }
            let evs: Vec<String> = matches.iter().map(|(e, _)| e.to_string()).collect();
            let text = format!("thread \"{title}\" ({th}) {}", if end > 0 { format!("ended its turn ({}). Its last reply: {reply}", super::launch::word(end, "")) } else { format!("opened an ask: {}", asked.clone().unwrap_or_default()) });
            let now = now_s(); let mut before: Vec<Value> = matches.iter().map(|(e, k)| json!({"$":"SettingSet","key":format!("wake.seen.{}.{e}", s.id),"value":k,"at":now})).collect();
            if s.once { before.push(json!({"$":"WakeDropped","id":s.id,"at":now})) }
            let room = self.ag.mailroom(); let to_uber = match &room { Some(m) => m.chat_is(&s.by).await, None => false };
            // Wake.thread.fire.covered: the uberbot already hears of a thread it launched through the report; the wake is only marked
            if to_uber && self.ag.world().await.launched.get(th).map_or(false, |(by, on)| *on && *by == s.by) { self.ag.commit_events(before).await; continue }
            let key = matches.iter().map(|(_, k)| k.as_str()).collect::<Vec<_>>().join("|");
            let msg = format!("t:{th}:lr-{}", hub_id("w", now, self.ag.seq_n()));
            let _ = self.ag.inbound_ref("user", &s.by, &msg, &note(&s, &text, &evs), (th.to_string(), "wake".into(), s.id.clone()), &format!("thread:{th}:{key}"), before).await;
        }
    }
}

pub struct WakeTools(Arc<Wakes>);
const NAMES: [&str; 3] = ["wake_on", "wake_list", "wake_cancel"];
fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }

impl super::mcp::ToolSet for WakeTools {
    fn list(&self, _thread: &str) -> Vec<Value> { defs(&NAMES) }
    fn owns(&self, name: &str) -> bool { NAMES.contains(&name) }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> crate::api::Fut<'a, (bool, Value)> { Box::pin(async move { self.0.call(thread, name, a).await }) }
}

impl Wakes {
    async fn call(&self, by: &str, name: &str, a: &Value) -> (bool, Value) {
        self.tail().await; // the book is the log as of now (a drop committed with a message is seen)
        match name {
            "wake_list" => { (true, json!({"wakes": self.book.lock().unwrap().by(by).iter().map(|w| w.json()).collect::<Vec<_>>()})) }
            "wake_cancel" => {
                if crate::copy::on() { return fail(COPY_LINE) }
                let id = a.get("id").and_then(Value::as_str).unwrap_or("");
                let own = !id.is_empty() && self.book.lock().unwrap().wakes.get(id).map_or(false, |w| w.by == by);
                if !own { return fail("no such wake subscription owned by this thread") }
                self.ag.commit_events(vec![json!({"$":"WakeDropped","id":id,"at":now_s()})]).await; self.book.lock().unwrap().wakes.remove(id);
                (true, json!({"id":id,"cancelled":true}))
            }
            _ => self.on(by, a).await,
        }
    }
    async fn on(&self, by: &str, a: &Value) -> (bool, Value) {
        if crate::copy::on() { return fail(COPY_LINE) }
        let (kind, target) = (a.get("kind").and_then(Value::as_str).unwrap_or(""), a.get("target").and_then(Value::as_str).unwrap_or(""));
        let events: Vec<String> = match a.get("events").and_then(Value::as_array) { Some(x) => x.iter().map(|e| e.as_str().unwrap_or("").to_string()).collect(), None => defaults(kind) };
        let e = error(by, kind, target, &events); if !e.is_empty() { return fail(&e) }
        if !SERVED.contains(&kind) { return fail(&format!("wake on a {kind} is not served by the Rust hub yet")) }
        { let w = self.ag.world().await; if kind == "thread" && !w.threads.get(target).map_or(false, |t| !t.deleted) { return fail("no such local target thread") }
          if kind == "thread" && w.launched.get(target).map_or(false, |(l, on)| *on && l == by) { return (true, json!({"id":format!("launch:{target}"),"existing":true,"delivery":"existing launched-thread report pipeline; no extra subscription"})) } }
        if let Some(w) = self.book.lock().unwrap().on(kind, target).into_iter().find(|w| w.by == by && w.events.join(",") == events.join(",")) { return (true, json!({"id":w.id,"existing":true})) }
        let now = now_s(); let id = hub_id("w", now, self.ag.seq_n());
        let once = a.get("once").and_then(Value::as_bool).unwrap_or(true); let note = a.get("note").and_then(Value::as_str).unwrap_or("");
        let ev = json!({"$":"WakeSet","id":id,"by":by,"kind":kind,"target":target,"events":events,"once":once,"note":note,"at":now});
        self.book.lock().unwrap().apply(&ev.to_string());
        self.ag.commit_events(vec![ev]).await;
        (true, Wake { id, by: by.into(), kind: kind.into(), target: target.into(), events, once, note: note.into(), created: now }.json())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ev(s: &str) -> Vec<String> { s.split(',').map(String::from).collect() }
    #[test] fn wake_error_follows_order() {
        assert_eq!(error("a", "x", "t", &ev("end")), "kind must be thread, work, pr or file");
        assert_eq!(error("a", "thread", "t", &ev("merged")), "target and valid nonempty events are required");
        assert_eq!(error("a", "thread", "l~t", &ev("end")), "wake on a thread elsewhere is not supported yet");
        assert_eq!(error("a", "thread", "a", &ev("end")), "a subscriber cannot wake itself");
        assert_eq!(error("a", "file", "rel", &ev("changed")), "file target must be an absolute path");
        assert_eq!(error("a", "pr", "o/r#0", &ev("merged")), "PR target must be owner/repo#N; bots must give owner/repo");
        assert_eq!(error("a", "pr", "o/r#12", &ev("merged")), ""); assert_eq!(error("a", "thread", "t", &ev("end,ask")), "");
    }
    #[test] fn defaults_by_kind() { assert_eq!(defaults("pr"), ev("merged,closed")); assert_eq!(defaults("file"), ev("changed")); assert_eq!(defaults("thread"), ev("end")); }
    #[test] fn the_note_is_one_paragraph_with_its_cancel_hint() {
        let w = Wake { id: "w1".into(), by: "b".into(), kind: "thread".into(), target: "t".into(), events: ev("end"), once: false, note: "n".into(), created: 1 };
        assert_eq!(note(&w, "thread \"x\" (t) ended.\nIts last reply: hi", &ev("end,done")), "Wake w1: thread \"x\" (t) ended. Its last reply: hi Events: end, done. Note: n. Cancel with wake_cancel {id: w1}.");
        let w2 = Wake { once: true, note: String::new(), ..w }; assert_eq!(note(&w2, "t", &ev("end")), "Wake w1: t Events: end.");
    }
    #[test] fn signals_in_order_with_their_own_keys() {
        let s = signals(1, 9, "t", Some(("a1", 9)), true);
        assert_eq!(s.iter().map(|(e, _)| *e).collect::<Vec<_>>(), vec!["end", "done", "ask", "idle"]);
        assert_eq!(s[0].1, "t:9:1"); assert_eq!(s[2].1, "t:ask:a1:9");
        assert_eq!(signals(2, 1, "t", None, false).iter().map(|(e, _)| *e).collect::<Vec<_>>(), vec!["end", "fail"]);
        assert!(signals(0, 1, "t", None, true).is_empty());
    }
    #[test] fn the_book_folds_sets_drops_and_seen() {
        let mut b = Book::default();
        b.apply(r#"{"$":"WakeSet","id":"w1","by":"a","kind":"thread","target":"t","events":["end"],"once":true,"note":"","at":5}"#);
        b.apply(r#"{"$":"SettingSet","key":"wake.seen.w1.end","value":"k","at":6}"#);
        assert_eq!(b.on("thread", "t").len(), 1); assert_eq!(b.by("a")[0].created, 5); assert_eq!(b.seen["wake.seen.w1.end"], "k");
        b.apply(r#"{"$":"WakeDropped","id":"w1","at":7}"#); assert!(b.on("thread", "t").is_empty());
    }
}

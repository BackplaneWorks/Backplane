//! A person's messages to a thread on another machine that have not reached it yet.
//!
//! A message sent to a local thread goes into the running turn at once (agents: steer). One to a thread elsewhere has to cross a link first. The
//! router takes it here (`hold`), answers the sender at once, and a worker posts each held message to its machine, oldest first per thread. While a
//! message is on its way for longer than `SHOW_AFTER_MS`, or its machine is away, it shows in the thread's queue strip (`rows`) with Edit and Remove;
//! the worker keeps trying an away machine (`backoff_ms`) and the row goes when the owner takes it. A message the owner refuses stays as a row that
//! says why, until the person edits or removes it. Kept in `<home>/far-sends.json`, so a restart of this hub loses none of them.
//! The decisions (`rows`, `outcome`, `due`) are pure and tested; `Outbox` only keeps them.
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

/// A message on its way shows in the strip once it has waited this long (a quick link shows nothing).
pub const SHOW_AFTER_MS: u64 = 1000;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Held {
    /// the message id (the sender's `msg`, the owner logs it as is)
    pub msg: String,
    /// the thread as this hub names it (`<link>~<id>`)
    pub thread: String,
    pub link: String,
    /// the request's params, ids made the owner's own
    pub p: Value,
    /// when it was held, the next try (ms)
    pub since: u64, pub next: u64,
    pub tries: u32,
    /// "" (not tried yet, or on its way), "away" (the machine could not be reached), "refused" (the owner said no: `why`)
    pub state: String, pub why: String,
    /// what is posted: `turn.start` (a person's message, `p` its params) or `mcp.call` (an agent's thread_send or thread_launch, or a launch report, `p` {tool, from, args})
    #[serde(default = "turn_start")] pub m: String,
}
fn turn_start() -> String { "turn.start".into() }
impl Held {
    pub fn text(&self) -> &str { self.p["text"].as_str().or_else(|| self.p["args"]["text"].as_str()).or_else(|| self.p["args"]["prompt"].as_str()).unwrap_or("") }
    /// the request body the owner's /far/rpc takes
    pub fn body(&self) -> Value { json!({"id": 0, "m": self.m, "p": self.p}) }
}

/// The wait before trying an away machine again: 1 s, doubling, 5 s at most, so a message goes within seconds of its machine coming back.
pub fn backoff_ms(tries: u32) -> u64 { (1000u64 << tries.saturating_sub(1).min(5)).min(5_000) }

/// What an answer from the owner does to the held message: None (taken: it goes), or the message as it stays.
pub fn outcome(h: &Held, code: u16, body: &str, now: u64) -> Option<Held> {
    let mut h = h.clone();
    let reply: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if code == 200 && reply["ok"] == true { return None }
    h.tries += 1;
    // could not be reached, or busy for now: tried again later. Anything else is the owner's answer, and trying again would get the same one.
    if code == 0 || code == 408 || code == 429 || code >= 500 {
        h.state = "away".into(); h.why = String::new(); h.next = now + backoff_ms(h.tries);
    } else {
        h.state = "refused".into();
        h.why = if code == 200 { reply["error"].as_str().or_else(|| reply["body"]["error"].as_str()).map(str::to_string).unwrap_or_else(|| reply["error"].to_string()) } else { body.chars().take(120).collect() };
        h.next = u64::MAX;
    }
    Some(h)
}

/// The messages to post now: per thread, its oldest one that is not refused, when its time has come and it is not on its way already.
pub fn due(items: &[Held], now: u64, busy: &[String]) -> Vec<Held> {
    let mut seen: Vec<&str> = vec![];
    let mut out = vec![];
    for h in items {
        if h.state == "refused" || seen.contains(&h.thread.as_str()) { continue }
        seen.push(&h.thread);
        if h.next <= now && !busy.contains(&h.msg) { out.push(h.clone()) }
    }
    out
}

/// The thread's queue-strip rows for what has not reached its machine (the shape of session/composer.rs `queue_json`): oldest first, a message on its way
/// only once it has waited `SHOW_AFTER_MS`. Tags: Sending (on its way), Unsent (its machine is away; it goes when it is back), Refused (with why).
/// Buttons: Edit (`o-edit`, `<msg>\u{1f}<text>`: out of here and back into the draft) and Remove (`o-remove`, `<msg>`).
pub fn rows(items: &[Held], thread: &str, now: u64) -> Vec<Value> {
    let first_line = |t: &str| t.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(200).collect::<String>();
    let b = |label: &str, action: &str, value: String| json!({"label": label, "action": action, "value": value});
    visible(items, now).into_iter().filter(|h| h.thread == thread).map(|h| {
        let (tag, text) = match h.state.as_str() {
            "away" => ("Unsent", first_line(h.text())),
            "refused" => ("Refused", format!("{} ({})", first_line(h.text()), h.why)),
            _ => ("Sending", first_line(h.text())),
        };
        json!({"msg": h.msg, "tag": tag, "text": text, "buttons": [b("Edit", "o-edit", format!("{}\u{1f}{}", h.msg, h.text())), b("Remove", "o-remove", h.msg.clone())]})
    }).collect()
}

/// What the strip shows: a message that has waited `SHOW_AFTER_MS` or could not go, and every later one of its thread (they wait behind it).
fn visible(items: &[Held], now: u64) -> Vec<&Held> {
    let mut behind: Vec<&str> = vec![];
    items.iter().filter(|h| {
        let show = behind.contains(&h.thread.as_str()) || h.state != "" || now.saturating_sub(h.since) >= SHOW_AFTER_MS;
        if show { behind.push(&h.thread) } show
    }).collect()
}

/// The held messages, kept on disk.
pub struct Outbox { path: Option<PathBuf>, items: Mutex<Vec<Held>>, pub kick: tokio::sync::Notify }
impl Outbox {
    pub fn open(path: Option<PathBuf>) -> Arc<Outbox> {
        let items = path.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<Vec<Held>>(&b).ok()).unwrap_or_default();
        Arc::new(Outbox { path, items: Mutex::new(items), kick: tokio::sync::Notify::new() })
    }
    fn save(&self, items: &[Held]) {
        let Some(p) = &self.path else { return };
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec(items).unwrap_or_default()).is_ok() { let _ = std::fs::rename(&tmp, p); }
    }
    /// Hold a message for `thread` (an id elsewhere) behind `link`; `p` the params as the owner takes them. A message held already (the same id) is held once.
    pub fn hold(&self, thread: &str, link: &str, p: Value, now: u64) { let msg = p["msg"].as_str().unwrap_or("").to_string(); self.hold_m(thread, link, "turn.start", &msg, p, now) }
    /// Hold any request for `link` under the id `msg` (an agent's thread_send, a launch report: `mcp.call`).
    pub fn hold_m(&self, thread: &str, link: &str, m: &str, msg: &str, p: Value, now: u64) {
        let msg = msg.to_string();
        { let mut v = self.items.lock().unwrap();
          if !v.iter().any(|h| h.msg == msg) { v.push(Held { msg, thread: thread.into(), link: link.into(), p, since: now, next: now, tries: 0, state: String::new(), why: String::new(), m: m.into() }); }
          self.save(&v); }
        self.kick.notify_one();
    }
    /// Take a held message out (Edit, Remove); its text, or None when it is not here (delivered meanwhile).
    pub fn remove(&self, thread: &str, msg: &str) -> Option<String> {
        let mut v = self.items.lock().unwrap();
        let i = v.iter().position(|h| h.thread == thread && h.msg == msg)?;
        let h = v.remove(i); self.save(&v); Some(h.text().to_string())
    }
    /// The owner's answer to a post of `msg`.
    pub fn answered(&self, msg: &str, code: u16, body: &str, now: u64) {
        let mut v = self.items.lock().unwrap();
        let Some(i) = v.iter().position(|h| h.msg == msg) else { return };
        match outcome(&v[i], code, body, now) { None => { v.remove(i); } Some(h) => v[i] = h }
        self.save(&v);
    }
    pub fn items(&self) -> Vec<Held> { self.items.lock().unwrap().clone() }
    pub fn rows(&self, thread: &str, now: u64) -> Vec<Value> { rows(&self.items.lock().unwrap(), thread, now) }
    /// The threads that have a row now (what the strip shows changes when this does).
    pub fn shown(&self, now: u64) -> Vec<(String, String, String)> {
        visible(&self.items.lock().unwrap(), now).into_iter().map(|h| (h.thread.clone(), h.msg.clone(), h.state.clone())).collect()
    }
}

/// The hub's outbox (one hub per process; the presenter and the phones' data read it from here).
static GLOBAL: OnceLock<Arc<Outbox>> = OnceLock::new();
pub fn global() -> Option<&'static Arc<Outbox>> { GLOBAL.get() }
pub fn set_global(o: Arc<Outbox>) -> Arc<Outbox> { GLOBAL.get_or_init(|| o).clone() }
/// The rows for a thread from the hub's outbox (none when no machine is linked).
pub fn rows_now(thread: &str) -> Vec<Value> { if !super::far::is(thread) { return vec![] } global().map(|o| o.rows(thread, now_ms())).unwrap_or_default() }
pub fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64) }

/// `outbox.remove {of, msg}` → `{text}`: Edit and Remove on a row of the strip. Hub-local: `of` (the thread) is not a key the far router renames, so this never
/// goes to the thread's machine, which has never seen the message.
pub struct Rpc(pub Arc<Outbox>);
impl crate::api::RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["outbox.remove"] }
    fn call<'a>(&'a self, _hub: &'a Arc<crate::hub::Hub>, _w: &'a crate::api::Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let r = self.0.remove(p["of"].as_str().unwrap_or(""), p["msg"].as_str().unwrap_or(""));
            self.0.kick.notify_one();
            vec![match r { Some(text) => json!({"t":"reply","id":id,"ok":true,"result":{"text":text}}), None => json!({"t":"reply","id":id,"ok":false,"error":"it reached its machine already"}) }]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(msg: &str, th: &str, since: u64) -> Held { Held { msg: msg.into(), thread: th.into(), link: "L".into(), p: json!({"thread": "t", "text": format!("hello {msg}\nmore"), "msg": msg}), since, next: since, tries: 0, state: String::new(), why: String::new(), m: "turn.start".into() } }
    #[test] fn a_message_on_its_way_shows_only_after_a_second_and_an_away_one_at_once() {
        let a = h("m1", "L~t", 1000);
        assert!(rows(&[a.clone()], "L~t", 1500).is_empty(), "a quick link shows nothing");
        let r = rows(&[a.clone()], "L~t", 2000); assert_eq!((r[0]["tag"].as_str(), r[0]["text"].as_str()), (Some("Sending"), Some("hello m1")));
        let away = outcome(&a, 0, "", 1100).unwrap(); assert_eq!((away.state.as_str(), away.next), ("away", 1100 + 1000));
        let r = rows(&[away], "L~t", 1100); assert_eq!(r[0]["tag"], "Unsent");
        assert_eq!(r[0]["buttons"], json!([{"label":"Edit","action":"o-edit","value":"m1\u{1f}hello m1\nmore"},{"label":"Remove","action":"o-remove","value":"m1"}]));
        assert!(rows(&[a.clone()], "L~other", 9000).is_empty(), "another thread's rows are its own");
        let (away, b) = (outcome(&a, 0, "", 1100).unwrap(), h("m2", "L~t", 1100));
        assert_eq!(rows(&[away, b.clone()], "L~t", 1100).len(), 2, "a new one behind an unsent one shows at once");
        assert_eq!(rows(&[h("m1", "L~u", 1100), b], "L~t", 1100).len(), 0, "behind another thread's, it does not");
    }
    #[test] fn an_agents_send_is_held_as_an_mcp_call_with_its_text_shown() {
        let o = Outbox::open(None);
        o.hold_m("L~t", "L", "mcp.call", "s1", json!({"tool": "thread_send", "from": "t9", "args": {"threadId": "t", "text": "stop and look", "_msg": "s1"}}), 0);
        let h = &o.items()[0]; assert_eq!((h.text(), h.body()["m"].as_str()), ("stop and look", Some("mcp.call")));
        let r = outcome(h, 200, r#"{"ok":false,"body":{"error":"not something this machine shares"}}"#, 1).unwrap(); assert_eq!(r.why, "not something this machine shares");
        assert_eq!(outcome(h, 200, r#"{"ok":true,"body":{"state":"sent"}}"#, 1), None);
    }
    #[test] fn taken_goes_refused_stays_with_why_and_busy_or_down_is_tried_again() {
        let a = h("m1", "L~t", 0);
        assert_eq!(outcome(&a, 200, r#"{"t":"reply","ok":true}"#, 5), None);
        let r = outcome(&a, 200, r#"{"t":"reply","ok":false,"error":"unknown thread"}"#, 5).unwrap(); assert_eq!((r.state.as_str(), r.why.as_str(), r.next), ("refused", "unknown thread", u64::MAX));
        assert_eq!(rows(&[r], "L~t", 5)[0]["text"], "hello m1 (unknown thread)");
        assert_eq!(outcome(&a, 403, "threads are shared with the owner's own machines", 5).unwrap().state, "refused");
        for c in [0, 502, 503, 429, 408] { assert_eq!(outcome(&a, c, "", 5).unwrap().state, "away", "{c}") }
        assert_eq!((backoff_ms(1), backoff_ms(2), backoff_ms(3), backoff_ms(4), backoff_ms(40)), (1000, 2000, 4000, 5000, 5000));
    }
    #[test] fn each_thread_sends_its_oldest_first_and_a_refused_one_does_not_block_the_rest() {
        let (a, b, c) = (h("m1", "L~t", 0), h("m2", "L~t", 0), h("m3", "L~u", 0));
        let d = due(&[a.clone(), b.clone(), c.clone()], 10, &[]); assert_eq!(d.iter().map(|x| x.msg.as_str()).collect::<Vec<_>>(), vec!["m1", "m3"]);
        assert_eq!(due(&[a.clone(), b.clone()], 10, &["m1".into()]).len(), 0, "the first is on its way: the second waits behind it");
        let mut later = a.clone(); later.next = 50; assert!(due(&[later, b.clone()], 10, &[]).is_empty(), "an away machine is tried again when its time comes");
        let mut no = a; no.state = "refused".into(); assert_eq!(due(&[no, b], 10, &[])[0].msg, "m2");
    }
    #[test] fn the_outbox_keeps_its_messages_across_a_restart_and_holds_one_id_once() {
        let dir = crate::plat::tmp::dir("farsend"); let f = dir.join("far-sends.json");
        let o = Outbox::open(Some(f.clone()));
        o.hold("L~t", "L", json!({"thread": "t", "text": "one", "msg": "m1"}), 1); o.hold("L~t", "L", json!({"thread": "t", "text": "one", "msg": "m1"}), 2);
        o.hold("L~t", "L", json!({"thread": "t", "text": "two", "msg": "m2"}), 3);
        o.answered("m1", 0, "", 4);
        let again = Outbox::open(Some(f.clone())); let items = again.items();
        assert_eq!(items.iter().map(|h| (h.msg.as_str(), h.state.as_str())).collect::<Vec<_>>(), vec![("m1", "away"), ("m2", "")]);
        assert_eq!(again.remove("L~t", "m2").as_deref(), Some("two")); assert_eq!(again.remove("L~t", "m2"), None, "gone already");
        again.answered("m1", 200, r#"{"ok":true}"#, 9); assert!(Outbox::open(Some(f)).items().is_empty());
    }
}

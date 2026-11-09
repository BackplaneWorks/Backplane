//! Phone push: what may reach a phone, when, and how it is cleared.
//!
//! One rule, enforced here: a push starts only from a `PushOrdered` event, and a `PushOrdered` can only be built from a `Source`, whose fields are private. The two ways
//! to get one are `Source::notify` (the uberbot's own notify_person) and `Source::reminder` (a reminder the person scheduled, or the uberbot's remind_person). Nothing else
//! holds one: worker threads, other bots, routines' turns, the mailroom, webhooks and turn ends write the `alert.<thread>` record, which reaches a desk (settings::alerts) and is a push of its own (`alert`, below),
//! and their news goes to the uberbot as mail, which decides. No RPC commits a client's event, and `setting.set` refuses `push.book` (`reserved`). The worker checks the
//! event again against the world when it arrives (`admit`), so a wrong commit in this hub is refused too.
//!
//! Respect, in order: a repeat is refused before it is ordered, against the told records (`repeat`); quiet hours in the person's zone hold an order until they end, a needs_you
//! passing only when the person allows it (`notify.quiet`, `notify.quiet.urgent`); a burst is held and collapsed into one (`route`, `collapse`); a muted device hears only
//! what the person let through. What is waiting is the book (`push.book`): the badge counts it with the open asks, and an item leaves it when its ask closes or, with no ask,
//! when the person writes in its thread or any device opens it (`push.seen`); every device then gets a silent clear. Android has no push service here: it hears the same `PushSent`
//! over its socket (data/1 `notify`), so it needs the app alive.
//!
//! One more way, 0.14's rule (src/server/push.bend: "an alert is a push of its own, raised when a thread's alert record is written, unless a desk in use is showing the
//! thread"): a thread's `alert.<thread>` record with a kind (done, fail, ask, merged, a note, a reminder) reaches the phones too, unless a focused desk in use shows the
//! thread (settings/alerts.rs decides that, as it does for the desk itself; `alert` below). Each is one order, through the same respect, book and collapsing as any other;
//! the uberbot's own orders are unchanged.
pub mod apns;
pub mod page;

use crate::{bots::{rpc::Bots, world::{Device, Routine, World}}, hub::{now_ms, Hub}};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;

pub const BOOK: &str = "push.book";
pub const QUIET: &str = "notify.quiet";
pub const QUIET_URGENT: &str = "notify.quiet.urgent";
pub const BURST: &str = "push.burst.s";
/// Quiet hours until the person sets their own ("off" turns them off).
pub const QUIET_DEFAULT: &str = "22:00-07:00";
/// The person's zone when the hub knows no other (the rule of America/New_York).
pub const ZONE_DEFAULT: &str = "EST5EDT,M3.2.0,M11.1.0";
pub const BURST_S: u64 = 120;
/// How far back a repeat is looked for, in days.
pub const REPEAT_DAYS: u64 = 1;
const KEEP_S: u64 = 7 * 86400;
const OPEN_MAX: usize = 200;

/// The proof that an order may be made. Its fields are private: `notify` and `reminder` are the only ways to one.
#[derive(Clone, Debug, PartialEq)]
pub struct Source { how: &'static str, bot: String }
impl Source {
    /// The uberbot, and only the uberbot.
    pub fn notify(w: &World, bot: &str) -> Option<Source> { w.triage_owner(bot).then(|| Source { how: "notify", bot: bot.into() }) }
    /// A reminder the person scheduled (no bot's name on it), or one the uberbot set with remind_person.
    pub fn reminder(w: &World, r: &Routine) -> Option<Source> { (r.kind == "remind" && (r.by.is_empty() || w.triage_owner(&r.by))).then(|| Source { how: "reminder", bot: r.by.clone() }) }
    /// The hub's own word that bringing a 0.14 home across is over (import014): one notice, to the phones that were away. Nothing else may use it.
    pub(crate) fn import_done() -> Source { Source { how: "import", bot: String::new() } }
    /// A thread's alert record that no desk in use is showing (`alert`). Nothing else may use it.
    fn alert() -> Source { Source { how: "alert", bot: String::new() } }
    /// The one event a push starts from.
    pub fn order(&self, id: &str, reason: &str, thread: &str, ask: &str, text: &str, at: u64) -> Value {
        json!({"$":"PushOrdered","id":id,"how":self.how,"bot":self.bot,"reason":reason,"thread":thread,"ask":ask,"text":short(text, 500),"at":at})
    }
}
/// The worker's own check of an order: a known way, and the uberbot's name on a notify.
pub fn admit(e: &Value, w: &World) -> bool {
    let (how, bot) = (e["how"].as_str().unwrap_or(""), e["bot"].as_str().unwrap_or(""));
    match how { "notify" => w.triage_owner(bot), "reminder" => bot.is_empty() || w.triage_owner(bot), "import" => bot.is_empty() && e["reason"] == "import",
        // a thread's alert record: the hub's own, about a thread
        "alert" => bot.is_empty() && !e["thread"].as_str().unwrap_or("").is_empty() && matches!(e["reason"].as_str(), Some("done" | "needs_you" | "reminder")),
        _ => false }
}
/// A thread's alert record (`kind`: done, fail, ask with the open ask's id in `ask` when known, merged, note, reminder; `key` its own) that no focused desk in use is
/// showing: the phones hear of it (settings/alerts.rs calls this; `body` is what 0.14's phone said). One order a record; the burst rule collapses a run.
pub async fn alert(hub: &Arc<Hub>, thread: &str, kind: &str, key: &str, body: &str, ask: &str, at: u64) {
    let reason = match kind { "ask" => "needs_you", "reminder" => "reminder", "" => return, _ => "done" };
    let _ = hub.commit(&Source::alert().order(&format!("alert-{key}-{at}"), reason, thread, ask, body, at)).await;
}
/// What a client may not write (`setting.set`).
pub fn reserved(key: &str) -> bool { key == BOOK || key.starts_with(SIGNIN) }
/// The hub's record that a provider's sign-in expired with no account to take over (settings/accounts_hub.rs `signin_alert`): written, the uberbot's one needs_you alert goes
/// (the uberbot cannot say it itself: it runs on an account too, and on Nov 7 it was the one that had expired); emptied, the alert is dealt with.
pub const SIGNIN: &str = "account.signin.alert.";
/// The order a sign-in record makes, in the uberbot's name, to its own thread; `link` opens the account's sign-in on a phone.
pub fn signin_order(w: &World, key: &str, rec: &Value, at: u64) -> Option<Value> {
    let src = Source::notify(w, &w.uber_bot())?;
    let provider = key.strip_prefix(SIGNIN).unwrap_or("");
    let mut e = src.order(&format!("signin-{provider}-{at}"), "needs_you", &w.triage_thread(), "", rec["text"].as_str().unwrap_or("Sign-in expired: tap to sign in again"), at);
    e["link"] = json!(rec["link"].as_str().unwrap_or("")); Some(e)
}
/// Whether needs_you or done text was already said (a reminder or digest repeats by design).
pub fn repeat(reason: &str, told: &[crate::bots::ledger::Told], now: u64, text: &str) -> bool {
    matches!(reason, "needs_you" | "done") && !crate::bots::ledger::check(told, now, "", text, REPEAT_DAYS).is_empty()
}
/// One line, cut to `max` characters at a word when it can be.
pub fn short(t: &str, max: usize) -> String {
    let line = t.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if line.chars().count() <= max { return line.to_string() }
    let cut: String = line.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.rsplit_once(' ').map_or(cut.as_str(), |(a, _)| a).trim_end())
}

/// `HH:MM-HH:MM` as minutes of the day: None when off, empty of length, or not that shape.
pub fn window(spec: &str) -> Option<(u64, u64)> {
    let m = |s: &str| { let (h, m) = s.trim().split_once(':')?; let (h, m): (u64, u64) = (h.parse().ok()?, m.parse().ok()?); (h < 24 && m < 60).then_some(h * 60 + m) };
    let (a, b) = spec.split_once('-')?; let (a, b) = (m(a)?, m(b)?); (a != b).then_some((a, b))
}
/// Seconds from `wall` (seconds of the local day) to the end of the quiet window; None outside it. A window may cross midnight.
pub fn quiet_left(spec: &str, wall: u64) -> Option<u64> {
    let (a, b) = window(spec)?; let t = wall % 86400; let (a, b) = (a * 60, b * 60);
    if a < b { (t >= a && t < b).then(|| b - t) } else if t >= a { Some(86400 - t + b) } else if t < b { Some(b - t) } else { None }
}
/// When an order goes: now (None), or at the time it is held to. `quiet` is the quiet window's end, `through` that this order passes it.
pub fn route(now: u64, last: u64, burst: u64, quiet: Option<u64>, through: bool) -> Option<u64> {
    let q = quiet.filter(|_| !through); let b = (last > 0 && now < last + burst).then_some(last + burst);
    match (q, b) { (None, None) => None, (Some(x), None) | (None, Some(x)) => Some(x), (Some(x), Some(y)) => Some(x.max(y)) }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Item { pub id: String, pub reason: String, pub thread: String, pub ask: String, pub text: String, pub at: u64, #[serde(default)] pub due: u64, #[serde(default)] pub key: String,
    /// What a tap opens instead of the thread ("account:<id>": that account's sign-in); empty for most.
    #[serde(default)] pub link: String }
/// What is waiting: `open` was shown, `held` waits for its time. `last` is when a push last went.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Book { pub open: Vec<Item>, pub held: Vec<Item>, pub last: u64 }
impl Book {
    pub fn next(&self) -> Option<u64> { self.held.iter().map(|i| i.due).min() }
    /// Take what is due at `now`.
    pub fn due(&mut self, now: u64) -> Vec<Item> { let (d, h): (Vec<_>, Vec<_>) = std::mem::take(&mut self.held).into_iter().partition(|i| i.due <= now); self.held = h; d }
    /// Drop what is dealt with: answers the keys no open item still shows, and whether anything left.
    pub fn deal(&mut self, dealt: impl Fn(&Item) -> bool) -> (Vec<String>, bool) {
        let n = self.open.len() + self.held.len();
        let gone: Vec<String> = self.open.iter().filter(|i| dealt(i)).map(|i| i.key.clone()).collect();
        self.open.retain(|i| !dealt(i)); self.held.retain(|i| !dealt(i));
        let mut keys: Vec<String> = gone.into_iter().filter(|k| !k.is_empty() && !self.open.iter().any(|i| &i.key == k)).collect();
        keys.sort(); keys.dedup();
        (keys, self.open.len() + self.held.len() != n)
    }
    pub fn trim(&mut self, now: u64) { self.open.retain(|i| i.at + KEEP_S > now); let n = self.open.len(); if n > OPEN_MAX { self.open.drain(..n - OPEN_MAX); } }
    /// The badge: every open ask, and every item not about one.
    pub fn badge(&self, asks: u64) -> u64 { asks + self.open.iter().chain(&self.held).filter(|i| i.ask.is_empty()).count() as u64 }
}
/// What one push says. Several items collapse into one, whose link is their thread when they share one, else the uberbot's.
#[derive(Clone, Debug, PartialEq)]
pub struct Say { pub key: String, pub title: String, pub body: String, pub thread: String, pub ask: String, pub reason: String, pub link: String }
pub fn collapse(items: &[Item], title: &str, home: &str, stamp: &str) -> Say {
    if let [i] = items { return Say { key: format!("push-{}", i.id), title: if title.is_empty() { "Backplane".into() } else { title.into() }, body: short(&i.text, 180), thread: i.thread.clone(), ask: i.ask.clone(), reason: i.reason.clone(), link: i.link.clone() } }
    let same = items.iter().all(|i| i.thread == items[0].thread);
    let need = items.iter().filter(|i| i.reason == "needs_you").count();
    let body = if need > 0 { format!("{need} of {} need you", items.len()) } else { format!("{} updates", items.len()) };
    Say { key: format!("push-{stamp}"), title: "Backplane".into(), body, thread: if same { items[0].thread.clone() } else { home.into() }, ask: String::new(), reason: if need > 0 { "needs_you".into() } else { "digest".into() },
        link: if items.iter().all(|i| i.link == items[0].link) { items[0].link.clone() } else { String::new() } }
}
/// Who hears it: a device that is not muted, and a muted one only for what the person let through.
pub fn hears(d: &Device, through: bool) -> bool { d.kind == "alert" && d.platform == "ios" && (!d.mute || through) }

/// A send's result. `Dead`: the token is retired (docs/push-retries.md).
#[derive(Clone, Debug, PartialEq)]
pub enum Sent { Ok, Dead, Later(String) }
pub trait Carrier: Send + Sync { fn send(self: Arc<Self>, hub: Arc<Hub>, d: Device, frame: apns::Frame) -> futures_util::future::BoxFuture<'static, Sent>; }

enum Msg { Devices, Order(Value), Asked(String), Spoke(String), Seen(String), Prefs, SignIn(String, String) }
pub struct Push { tx: mpsc::UnboundedSender<Msg> }
impl Push {
    /// `push.seen`: a device opened the thread.
    pub fn seen(&self, thread: &str) { let _ = self.tx.send(Msg::Seen(thread.into())); }
}
struct Watch(Arc<Push>);
impl crate::api::Observer for Watch {
    fn committed(&self, _hub: &Arc<Hub>, events: &[Value], _first: u32) {
        for e in events { let tx = &self.0.tx; let _ = match e["$"].as_str() {
            Some("PushOrdered") => tx.send(Msg::Order(e.clone())),
            Some("DeviceRegistered" | "DeviceDropped" | "DevicePrefs") => tx.send(Msg::Devices),
            Some("AskClosed") => tx.send(Msg::Asked(e["id"].as_str().unwrap_or("").into())),
            Some("MessagePosted") if e["role"] == "user" && crate::state::fam::turnend::policy_direct(e["msg"].as_str().unwrap_or(""), e["text"].as_str().unwrap_or("")) => tx.send(Msg::Spoke(e["thread"].as_str().unwrap_or("").into())),
            Some("SettingSet") if matches!(e["key"].as_str(), Some(QUIET | QUIET_URGENT)) => tx.send(Msg::Prefs),
            // the person saw the import's summary: its notice leaves the book (it has no thread to be seen in)
            Some("SettingSet") if e["key"].as_str() == Some(crate::import014::ACK) => tx.send(Msg::Seen(String::new())),
            Some("SettingSet") if e["key"].as_str().map_or(false, |k| k.starts_with(SIGNIN)) => tx.send(Msg::SignIn(e["key"].as_str().unwrap_or("").into(), e["value"].as_str().unwrap_or("").into())),
            _ => Ok(()),
        }; }
    }
}

/// `push.seen {thread}`: a device opened it. `push.state`: what is waiting, for a phone to set its badge and drop what was dealt with while it slept.
struct PushRpc(Arc<Push>);
impl crate::api::RpcHandler for PushRpc {
    fn methods(&self) -> &'static [&'static str] { &["push.seen", "push.state"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a crate::api::Caller, id: &'a Value, method: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if method == "push.seen" { self.0.seen(p["thread"].as_str().unwrap_or("")); return vec![json!({"t":"reply","id":id,"ok":true})] }
            let book: Book = hub.setting(BOOK).await.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
            let mut keys: Vec<String> = book.open.iter().map(|i| i.key.clone()).collect(); keys.sort(); keys.dedup();
            let quiet = hub.setting(QUIET).await.unwrap_or_default();
            vec![json!({"t":"reply","id":id,"ok":true,"result":{"keys":keys,"badge":book.badge(crate::data::open_ask_total(hub)),"quiet":if quiet.is_empty() { QUIET_DEFAULT.to_string() } else { quiet },"urgent":hub.setting(QUIET_URGENT).await.as_deref() == Some("on")}})]
        })
    }
}
pub fn start(hub: &Arc<Hub>, bots: Arc<Bots>) -> Arc<Push> { start_with(hub, bots, Arc::new(apns::Apns::new())) }
pub fn start_with(hub: &Arc<Hub>, bots: Arc<Bots>, carrier: Arc<dyn Carrier>) -> Arc<Push> {
    let (tx, rx) = mpsc::unbounded_channel();
    let p = Arc::new(Push { tx });
    hub.register_observer(Arc::new(Watch(p.clone())));
    hub.register_rpc(Arc::new(PushRpc(p.clone())));
    hub.register_rpc(Arc::new(page::NotifyPage(bots.clone())));
    // said once at start, so a hub that cannot push says so in its log (never the values)
    { let hub = hub.clone(); tokio::spawn(async move { match apns::credentials(&hub).await { Some((.., from)) => eprintln!("backplane: push: APNs configured (from {from})"), None => eprintln!("backplane: push: APNs is not configured: set push.apns.key, push.apns.keyid and push.apns.team, or write ~/.config/backplane/apns.env") } }); }
    tokio::spawn(Worker { hub: hub.clone(), bots, carrier, book: Book::default() }.run(rx));
    p
}

struct Worker { hub: Arc<Hub>, bots: Arc<Bots>, carrier: Arc<dyn Carrier>, book: Book }
fn now_s() -> u64 { now_ms() / 1000 }
impl Worker {
    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Msg>) {
        self.book = self.hub.setting(BOOK).await.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
        self.publish_devices().await;
        loop {
            // a timer only while something is held; nothing runs otherwise
            let wait = self.book.next().map(|t| Duration::from_secs(t.saturating_sub(now_s())));
            let msg = match wait {
                Some(d) => tokio::select! { m = rx.recv() => m.map(Some), _ = tokio::time::sleep(d) => Some(None) },
                None => rx.recv().await.map(Some),
            };
            match msg { None => return, Some(None) => self.flush().await, Some(Some(m)) => self.on(m).await }
        }
    }
    /// The phones that hear a push, for the Notifications page (`info["push.devices"]`: handles, never tokens).
    async fn publish_devices(&self) { let v = { let w = self.bots.world().await; page::devices_info(w.devices.values()) }; self.hub.set_info("push.devices", &v).await; }
    async fn on(&mut self, m: Msg) {
        match m {
            Msg::Devices => self.publish_devices().await,
            Msg::Order(e) => self.order(e).await,
            Msg::Asked(id) => self.deal(move |i| !id.is_empty() && i.ask == id).await,
            // an approval stays until it is answered: looking at it is not dealing with it
            Msg::Spoke(t) | Msg::Seen(t) => self.deal(move |i| i.thread == t && i.ask.is_empty()).await,
            // the record of an expired sign-in: the alert, once; its clearing (the account is back) takes it off every phone
            Msg::SignIn(key, v) => {
                let provider = key.strip_prefix(SIGNIN).unwrap_or("").to_string();
                if v.is_empty() { let pre = format!("signin-{provider}-"); return self.deal(move |i| i.id.starts_with(&pre)).await }
                let rec: Value = serde_json::from_str(&v).unwrap_or(Value::Null);
                let e = { let w = self.bots.world().await; signin_order(&w, &key, &rec, now_s()) };
                match e { Some(e) => { let _ = self.hub.commit(&e).await; } None => eprintln!("push: a sign-in expired and there is no uberbot to say so") }
            }
            Msg::Prefs => { let now = now_s(); let (q, _) = self.quiet(now).await; for i in &mut self.book.held { i.due = q.unwrap_or(now).max(now); } self.save().await }
        }
    }
    /// The quiet window's end, if in one, and whether a needs_you passes it.
    async fn quiet(&self, now: u64) -> (Option<u64>, bool) {
        let spec = self.hub.setting(QUIET).await.unwrap_or_default();
        let spec = if spec.is_empty() { QUIET_DEFAULT.to_string() } else { spec };
        let rule = { let w = self.bots.world().await; let z = crate::bots::clock::Zones { settings: &w.settings }; let n = z.person();
            if n == "UTC" { crate::bots::clock::TzRule::parse(ZONE_DEFAULT) } else { z.rule(&n).or_else(|| crate::bots::clock::TzRule::parse(ZONE_DEFAULT)) } };
        let wall = rule.map_or(now, |r| r.wall(now));
        (quiet_left(&spec, wall).map(|left| now + left), self.hub.setting(QUIET_URGENT).await.as_deref() == Some("on"))
    }
    async fn order(&mut self, e: Value) {
        let ok = { let w = self.bots.world().await; admit(&e, &w) };
        if !ok { eprintln!("push: an order with no source was refused"); return }
        let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
        let now = now_s();
        let mut item = Item { id: s("id"), reason: s("reason"), thread: s("thread"), ask: s("ask"), text: s("text"), at: e["at"].as_u64().unwrap_or(now), due: 0, key: String::new(), link: s("link") };
        let (quiet, allowed) = self.quiet(now).await;
        let burst = self.hub.setting(BURST).await.and_then(|v| v.parse().ok()).unwrap_or(BURST_S);
        match route(now, self.book.last, burst, quiet, allowed && item.reason == "needs_you") {
            Some(t) => { item.due = t; self.book.held.push(item); self.save().await }
            None => self.deliver(vec![item]).await,
        }
    }
    async fn flush(&mut self) { let d = self.book.due(now_s()); if d.is_empty() { self.save().await } else { self.deliver(d).await } }
    async fn deliver(&mut self, mut items: Vec<Item>) {
        let now = now_s();
        let (devices, home) = { let w = self.bots.world().await; (w.devices.values().cloned().collect::<Vec<_>>(), w.triage_thread()) };
        let title = { let m = self.hub.model.lock().await; items.first().and_then(|i| m.threads.get(&i.thread)).map(|t| t.title.clone()).unwrap_or_default() };
        let say = collapse(&items, &title, &home, &format!("{now}-{}", items.len()));
        for i in &mut items { i.key = say.key.clone() }
        self.book.open.extend(items.iter().cloned()); self.book.last = now; self.book.trim(now);
        let through = self.hub.setting(QUIET_URGENT).await.as_deref() == Some("on") && say.reason == "needs_you";
        let badge = self.book.badge(self.asks());
        let frame = apns::Frame::alert(&say, badge);
        let sent = self.send(devices.iter().filter(|d| hears(d, through)), &frame).await;
        let ev = json!({"$":"PushSent","key":say.key,"thread":say.thread,"ask":say.ask,"kind":say.reason,"title":say.title,"body":say.body,"badge":badge,"through":through,"sent":sent,"link":say.link,"at":now});
        let _ = self.hub.commit(&ev).await;
        self.save().await;
    }
    async fn deal(&mut self, dealt: impl Fn(&Item) -> bool) {
        let (keys, changed) = self.book.deal(dealt);
        if !changed { return }
        self.save().await;
        let badge = self.book.badge(self.asks());
        let devices = { let w = self.bots.world().await; w.devices.values().cloned().collect::<Vec<_>>() };
        let frame = apns::Frame::clear(&keys, badge);
        // recorded first: a socket (Android, the desktop) hears the clear no later than an iOS phone does
        let _ = self.hub.commit(&json!({"$":"PushCleared","keys":keys,"badge":badge,"at":now_s()})).await;
        self.send(devices.iter().filter(|d| d.kind == "alert" && d.platform == "ios"), &frame).await;
    }
    fn asks(&self) -> u64 { crate::data::open_ask_total(&self.hub) }
    async fn send(&self, devices: impl Iterator<Item = &Device>, frame: &apns::Frame) -> u64 {
        let mut n = 0;
        for d in devices {
            match self.carrier.clone().send(self.hub.clone(), d.clone(), frame.clone()).await {
                Sent::Ok => n += 1,
                Sent::Dead => { let _ = self.hub.commit(&crate::net::devices::dropped(&json!({"token":d.token}), now_s())).await; }
                Sent::Later(why) => eprintln!("push: not delivered: {why}"),
            }
        }
        n
    }
    async fn save(&self) { if let Ok(v) = serde_json::to_string(&self.book) { let _ = self.hub.commit(&json!({"$":"SettingSet","key":BOOK,"value":v,"at":now_s()})).await; } }
}

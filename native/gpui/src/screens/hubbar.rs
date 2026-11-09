//! Restart hub: a strip at the top of the window when the hub needs the person (safe mode, a part not running, a build gone back, no answer), and the commands in the palette.
//! It asks the hub over plain HTTP, on its own, never through the socket or a view, so it works when those are what broke. What it offers comes from the hub or its guard (`/hub/status`,
//! docs/HUB-RESILIENCE.md). With no answer at all, a window on the hub's machine leaves the guard a request in `<home>/guard/`, beside the token file.
use super::attach::Target;
use crate::{theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{io::{Read, Write}, net::{TcpStream, ToSocketAddrs}, path::{Path, PathBuf}, time::{Duration, Instant}};

/// How often the hub is asked while it answers, and while it does not.
const WELL: Duration = Duration::from_secs(10);
const ILL: Duration = Duration::from_secs(3);
/// How long the confirming click stays asked for.
pub const CONFIRM: Duration = Duration::from_secs(6);
/// Unanswered asks before the strip says so: a hub restarting for a new build is gone for a moment.
const PATIENCE: u32 = 3;
/* i18n-ok: English, compared and tested; strip draws it through trs */ const NO_ANSWER: &str = "The hub is not answering";

#[derive(Clone, Debug, PartialEq)]
pub struct Act { pub id: String, pub label: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Bar { pub text: String, pub tone: String, pub acts: Vec<Act> }

#[derive(Default)]
pub struct State { pub ever: bool, pub well: bool, pub bar: Option<Bar>, pub said: String, pub confirm: Option<(String, Instant)>, pub target: Option<Target>, pub guard: Option<PathBuf> }

/* i18n-ok: English, compared and tested; strip draws it through trs */ fn restart() -> Act { Act { id: "restart".into(), label: "Restart hub".into() } }
fn acts(v: &Value) -> Vec<Act> { v.as_array().into_iter().flatten().filter_map(|a| Some(Act { id: a["id"].as_str()?.into(), label: a["label"].as_str()?.into() })).collect() }
fn or_restart(a: Vec<Act>) -> Vec<Act> { if a.is_empty() { vec![restart()] } else { a } }

/// The window's socket to the hub: `live` while it is Connected, `ever` once it has been (or `/healthz` has answered) since the window opened.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sock { pub live: bool, pub ever: bool }
/// What the strip shows, from `/healthz` and `/hub/status` (None: no answer), after `misses` unanswered asks in a row. None: nothing to say.
/// A hub whose socket is Connected is answering, whatever its HTTP side says; before the first connection the window says "Connecting…" itself, so the strip waits twice as long.
pub fn bar_of(health: Option<&Value>, status: Option<&Value>, misses: u32, sock: Sock) -> Option<Bar> {
    let patience = if sock.ever { PATIENCE } else { 2 * PATIENCE };
    let Some(h) = health else { return (!sock.live && misses >= patience).then(|| Bar { text: NO_ANSWER.into(), tone: "bad".into(), acts: vec![restart()] }) };
    let guard = status.map_or(&Value::Null, |s| &s["guard"]);
    match h["status"].as_str().unwrap_or("") {
        /* i18n-ok: English, compared and tested; strip draws it through trs */ "starting" => Some(Bar { text: match h["phase"].as_str() { Some(p) if !p.is_empty() => format!("The hub is starting: {p}"), _ => "The hub is starting".into() }, tone: "info".into(), acts: vec![] }),
        /* i18n-ok: English, compared and tested; strip draws it through trs */ "down" => Some(Bar { text: guard["text"].as_str().filter(|t| !t.is_empty()).map_or_else(|| format!("The hub is down: {}", h["why"].as_str().unwrap_or("")), str::to_string), tone: "bad".into(), acts: or_restart(acts(&guard["actions"])) }),
        /* i18n-ok: English, compared and tested; strip draws it through trs */ "hung" => Some(Bar { text: format!("The hub is stuck in {}", h["hung"].as_array().map_or(String::new(), |a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))), tone: "bad".into(), acts: or_restart(acts(&guard["actions"])) }),
        _ => {
            let n = status.map_or(&Value::Null, |s| &s["notice"]);
            let t = n["text"].as_str().unwrap_or("");
            (!t.is_empty()).then(|| Bar { text: t.to_string(), tone: n["tone"].as_str().unwrap_or("info").to_string(), acts: acts(&n["actions"]) })
        }
    }
}

/// One request, over TLS when the hub is (a hub behind `tailscale serve`: plain HTTP to it never answers): (status, body), None when nothing answered.
fn call(t: &Target, method: &str, path: &str, token: &str, body: &str) -> Option<(u16, String)> {
    let addr = t.addr.as_str();
    let a = addr.to_socket_addrs().ok()?.next()?; let d = Duration::from_secs(5);
    let tcp = TcpStream::connect_timeout(&a, d).ok()?; tcp.set_read_timeout(Some(d)).ok()?; tcp.set_write_timeout(Some(d)).ok()?;
    let mut s = crate::pairing::wrap(addr, tcp, t.tls).ok()?;
    /* i18n-ok: HTTP */ s.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).ok()?;
    let mut out = Vec::new(); let _ = s.take(1 << 20).read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out);
    Some((text.split_whitespace().nth(1)?.parse().ok()?, text.split_once("\r\n\r\n").map_or(String::new(), |x| x.1.to_string())))
}
fn ask(t: &Target) -> (Option<Value>, Option<Value>) {
    let parse = |r: Option<(u16, String)>| r.and_then(|(_, b)| serde_json::from_str::<Value>(&b).ok());
    /* i18n-ok: HTTP */ let h = parse(call(t, "GET", "/healthz", "", ""));
    /* i18n-ok: HTTP */ let s = if h.is_some() { call(t, "GET", "/hub/status", &t.token, "").filter(|r| r.0 == 200).and_then(|r| serde_json::from_str(&r.1).ok()) } else { None };
    (h, s)
}
/// The request a window on the hub's machine leaves the guard when nothing answers on the network: whole, and renamed into place.
pub fn leave_request(dir: &Path, action: &str) -> std::io::Result<()> {
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let tmp = dir.join(format!(".request.{}", std::process::id()));
    std::fs::write(&tmp, json!({"id": format!("w{at}-{}", std::process::id()), "action": action, "build": "", "from": "local", "at": at}).to_string())?;
    std::fs::rename(&tmp, dir.join("request"))
}
/// Do it, and say how it went.
fn send(t: Option<&Target>, guard: Option<&Path>, action: &str) -> String {
    if let Some(t) = t {
        /* i18n-ok: HTTP */ if let Some((code, b)) = call(t, "POST", "/hub/control", &t.token, &json!({"action": action}).to_string()) {
            let said = serde_json::from_str::<Value>(&b).ok().and_then(|v| v["text"].as_str().map(str::to_string)).unwrap_or_default();
            /* i18n-ok: English, compared and tested; strip draws it through trs */ return if said.is_empty() { format!("The hub said {code}") } else { said };
        }
    }
    /* i18n-ok: English, compared and tested; strip draws it through trs */ match guard.filter(|g| g.is_dir()).map(|g| leave_request(g, action)) { Some(Ok(())) => "Asked the guard on this machine".into(), _ => "The hub's machine cannot be reached".into() }
}

/// Look now and then. The window is redrawn only when what it shows changes.
pub fn watch(cx: &mut Context<Backplane>, target: Option<Target>, token_file: &Path) {
    let guard = token_file.parent().map(|h| h.join("guard"));
    let Some(t) = target.clone() else { return };
    cx.spawn(async move |weak, cx| {
        let mut misses = 0u32;
        let _ = weak.update(cx, |this, _| { this.screens.hubbar.target = target; this.screens.hubbar.guard = guard; });
        loop {
            let t2 = t.clone();
            let (h, s) = cx.background_executor().spawn(async move { ask(&t2) }).await;
            misses = if h.is_some() { 0 } else { misses + 1 };
            if weak.update(cx, |this, cx| {
                let live = this.status == "Connected";
                let b = &mut this.screens.hubbar; b.ever |= live || h.is_some();
                let now = bar_of(h.as_ref(), s.as_ref(), misses, Sock { live, ever: b.ever });
                b.well = now.is_none() && (h.is_some() || live); let stale = b.confirm.as_ref().map_or(false, |c| c.1.elapsed() > CONFIRM);
                if stale { b.confirm = None; }
                if b.bar != now || stale { if now.is_none() { b.said.clear(); } b.bar = now; cx.notify(); }
            }).is_err() { break }
            let well = weak.update(cx, |this, _| this.screens.hubbar.well).unwrap_or(false);
            cx.background_executor().timer(if well { WELL } else { ILL }).await;
        }
    }).detach();
}

/// The strip: the words, the hub's actions, and the confirming click. Shown when there is something to say, or a palette command asks to be sure.
pub fn strip(cx: &mut Context<Backplane>, s: &State, light: bool) -> Option<AnyElement> {
    let (bar, asked) = (s.bar.as_ref(), s.confirm.as_ref().map(|c| c.0.clone()));
    if bar.is_none() && asked.is_none() && s.said.is_empty() { return None }
    let c = |n: &str| theme::color(light, n);
    let hue = match bar.map_or("info", |b| b.tone.as_str()) { "bad" => c("bad"), "warn" => c("warn"), _ => c("accent") };
    let text = if s.said.is_empty() { bar.map_or(String::new(), |b| b.text.clone()) } else { s.said.clone() };
    let mut acts = bar.map_or(vec![], |b| b.acts.clone());
    if let Some(a) = &asked { if !acts.iter().any(|x| x.id == *a) { acts.push(Act { id: a.clone(), label: label_of(a).into() }); } }
    let mut row = div().flex().items_center().gap_1();
    for a in acts {
        let sure = asked.as_deref() == Some(a.id.as_str());
        let (id, label) = (a.id.clone(), if sure { crate::i18n::tf("gpui.hubbar.click.again", &[("label", crate::i18n::trs(&a.label).into())]) } else { crate::i18n::trs(&a.label) });
        row = row.child(div().id(SharedString::from(format!("hub-{id}"))).px_2().h(px(16.)).flex().items_center().cursor_pointer().text_size(px(11.)).font_weight(FontWeight::BOLD)
            .bg(if sure { c("bad") } else { c("raise") }).text_color(if sure { c("on-accent") } else { c("text") }).child(label)
            .on_click(cx.listener(move |this, _, _, cx| { this.hub_click(&id, cx); })));
    }
    Some(div().h(px(22.)).flex_shrink_0().flex().items_center().justify_between().px_3().bg(c("raise")).border_b_1().border_color(hue)
        .child(div().text_size(px(11.)).text_color(c("text")).overflow_hidden().child(crate::i18n::trs(&text))).child(row).into_any_element())
}

/// The socket came up: the hub answers, so a strip that said it did not goes at once (the next ask decides anything else).
pub fn connected(s: &mut State) -> bool { s.ever = true; if s.bar.as_ref().map_or(false, |b| b.text == NO_ANSWER) { s.bar = None; s.said.clear(); true } else { false } }

/* i18n-ok: English, compared and tested; strip draws it through trs */ pub fn label_of(action: &str) -> &'static str { match action { "safe" => "Restart in safe mode", "rollback" => "Roll back hub", _ => "Restart hub" } }

impl Backplane {
    /// The first click asks to be sure; the second, within CONFIRM, does it. The palette's commands come through here too.
    pub(crate) fn hub_click(&mut self, action: &str, cx: &mut Context<Self>) {
        let b = &mut self.screens.hubbar;
        match b.confirm.take() {
            Some((a, t)) if a == action && t.elapsed() <= CONFIRM => {
                let (t, g, a) = (b.target.clone(), b.guard.clone(), a.clone());
                /* i18n-ok: English, compared and tested; strip draws it through trs */ b.said = "Asking…".into();
                cx.spawn(async move |weak, cx| {
                    let said = cx.background_executor().spawn(async move { send(t.as_ref(), g.as_deref(), &a) }).await;
                    let _ = weak.update(cx, |this, cx| { this.screens.hubbar.said = said; cx.notify(); });
                }).detach();
            }
            _ => { b.said.clear(); b.confirm = Some((action.to_string(), Instant::now())); }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    #[test] fn a_well_hub_shows_nothing() {
        assert_eq!(bar_of(Some(&json!({"status": "ok"})), Some(&json!({"notice": {"text": ""}})), 0, UP), None);
        assert_eq!(bar_of(Some(&json!({"status": "ok"})), None, 0, UP), None, "a hub with no notice to give");
    }
    const UP: Sock = Sock { live: false, ever: true };
    #[test] fn a_moment_without_an_answer_is_not_news() {
        assert_eq!(bar_of(None, None, 1, UP), None);
        let b = bar_of(None, None, PATIENCE, UP).unwrap(); assert_eq!(b.tone, "bad"); assert_eq!(b.acts[0].id, "restart", "restart is on offer with nothing from the hub at all");
    }
    #[test] fn safe_mode_comes_with_the_hubs_own_actions() {
        let b = bar_of(Some(&json!({"status": "degraded", "safe": true})), Some(&json!({"notice": {"text": "Safe mode", "tone": "warn", "actions": [{"id": "restart", "label": "Restart normally"}]}})), 0, UP).unwrap();
        assert_eq!((b.text.as_str(), b.tone.as_str(), b.acts[0].label.as_str()), ("Safe mode", "warn", "Restart normally"));
    }
    #[test] fn rescue_offers_what_the_guard_offers_and_restart_at_the_least() {
        let b = bar_of(Some(&json!({"status": "down", "why": "panic"})), Some(&json!({"guard": {"text": "Hub is down: panic", "actions": [{"id": "rollback", "label": "Roll back to abc"}]}})), 0, UP).unwrap();
        assert_eq!(b.acts[0].id, "rollback"); assert_eq!(b.text, "Hub is down: panic");
        assert_eq!(bar_of(Some(&json!({"status": "down", "why": "panic"})), None, 0, UP).unwrap().acts[0].id, "restart", "no credential for /hub/status: still a way back");
    }
    #[test] fn starting_is_said_and_asks_nothing() { let b = bar_of(Some(&json!({"status": "starting", "phase": "model built"})), None, 0, UP).unwrap(); assert!(b.acts.is_empty()); assert!(b.text.contains("model built")); }
    #[test] fn a_hung_hub_names_what_stopped() { assert!(bar_of(Some(&json!({"status": "hung", "hung": ["writer"]})), None, 0, UP).unwrap().text.contains("writer")); }
    #[test] fn a_connected_socket_is_an_answer() {
        // a remote hub behind tailscale serve: /healthz never answered over plain HTTP while the socket was Connected and the viewers drew
        assert_eq!(bar_of(None, None, 99, Sock { live: true, ever: true }), None, "Connected is answering");
        assert_eq!(bar_of(None, None, PATIENCE, Sock { live: false, ever: false }), None, "the first connection is the window's own Connecting…");
        assert!(bar_of(None, None, 2 * PATIENCE, Sock { live: false, ever: false }).is_some(), "a hub that never comes up is still said");
        let mut s = State { bar: bar_of(None, None, PATIENCE, UP), ..Default::default() }; assert!(s.bar.is_some());
        assert!(connected(&mut s) && s.bar.is_none(), "it goes the moment the socket is up");
        let mut s = State { bar: bar_of(Some(&json!({"status": "starting"})), None, 0, UP), ..Default::default() }; assert!(!connected(&mut s) && s.bar.is_some(), "what the hub said stays");
    }
    #[test] fn health_is_asked_over_the_hubs_own_scheme() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || { let (mut c, _) = l.accept().unwrap(); let mut b = [0u8; 512]; let _ = c.read(&mut b); let _ = c.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n{\"status\":\"ok\"}"); });
        let (h, _) = ask(&Target { addr: addr.clone(), token: "t".into(), tls: false }); assert_eq!(h.unwrap()["status"], "ok");
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
        let seen = std::thread::spawn(move || { let (mut c, _) = l.accept().unwrap(); let mut b = [0u8; 8]; let n = c.read(&mut b).unwrap_or(0); b[..n].to_vec() });
        let _ = ask(&Target { addr, token: "t".into(), tls: true });
        assert_eq!(seen.join().unwrap().first(), Some(&0x16), "a TLS hub gets a TLS handshake, not a plain GET");
    }
    #[test] fn a_request_for_the_guard_lands_whole() {
        let d = std::env::temp_dir().join(format!("bp-hubbar-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        leave_request(&d, "restart").unwrap(); let v: Value = serde_json::from_str(&std::fs::read_to_string(d.join("request")).unwrap()).unwrap();
        assert_eq!((v["action"].as_str(), v["from"].as_str()), (Some("restart"), Some("local")));
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "nothing left beside it");
    }
    #[test] fn with_nothing_to_reach_it_says_so() { assert_eq!(send(None, None, "restart"), "The hub's machine cannot be reached"); }
}

//! Google: the section of Settings → Accounts for the accounts bots may use (docs/GOOGLE.md). The hub keeps the accounts and the sign-in; this file asks it with the
//! `google` request (the reply's `result` is the View) and draws what it says. The sign-in comes back to a listener this client opens itself on 127.0.0.1
//! (RFC 8252): it catches the one `GET /oauth/google?...` and sends the hub the whole address as the `code`. The pasted code is never a draft, a `bfield` or a log line.
use super::accounts::button;
use super::composer::{Composer, ComposerEvent};
use super::settings::Secret;
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Default)]
pub struct Google {
    /// The newest View the hub sent (Null before the first answer).
    pub view: Value,
    /// The account whose Remove was pressed once (the second press removes).
    pub armed: String,
    pub notice: String,
    /// The sign-in attempt whose link was opened (once each).
    pub opened: String,
    /// This client started a sign-in whose link is not yet opened.
    pub want_open: bool,
    /// Stops the listener of the sign-in in flight.
    pub stop: Option<Arc<AtomicBool>>,
    pub paste: Option<Entity<Composer>>,
    pub polling: bool,
}

// ---- pure ------------------------------------------------------------------------------------------------------------------------------------------

pub const PATH: &str = "/oauth/google";
pub fn redirect_for(port: u16) -> String { format!("http://127.0.0.1:{port}{PATH}") }
/// The request target (path and query) of a `GET /oauth/google?...` that carries a query; anything else is not the browser coming back.
pub fn request_target(head: &str) -> Option<String> {
    let line = head.lines().next()?;
    let mut p = line.split(' ');
    let (method, target) = (p.next()?, p.next()?);
    let (path, query) = target.split_once('?')?;
    (method == "GET" && path == PATH && !query.is_empty()).then(|| target.to_string())
}
/// The Host header must name this listener (a name that resolves here from a page elsewhere is refused).
pub fn host_ok(head: &str, port: u16) -> bool {
    let host = head.lines().skip(1).find_map(|l| { let (k, v) = l.split_once(':')?; k.trim().eq_ignore_ascii_case("host").then(|| v.trim().to_string()) });
    matches!(host, Some(h) if h == format!("127.0.0.1:{port}") || h == format!("localhost:{port}"))
}
/// The address sent to the hub as the `code`: the redirect it was given with the query the browser came back with.
pub fn caught(port: u16, target: &str) -> Option<String> { let (_, q) = target.split_once('?')?; Some(format!("{}?{q}", redirect_for(port))) }
/// What a connection to the listener asked: the caught address (answer 200) or nothing (answer 404).
pub fn classify(head: &str, port: u16) -> Option<String> {
    if !host_ok(head, port) { return None }
    caught(port, &request_target(head)?)
}

/// The words a Google section shows, from the hub's View.
#[derive(Debug, PartialEq, Clone)]
pub struct AccountRow { pub id: String, pub email: String, pub scopes: String, pub line: String, pub tone: String, pub bots: String }
#[derive(Debug, PartialEq, Clone)]
pub struct BotRow { pub name: String, pub grants: Vec<(String, String, bool)> }
pub fn account_rows(v: &Value) -> Vec<AccountRow> {
    v["accounts"].as_array().into_iter().flatten().map(|a| {
        let bots = string(a, "bots");
        AccountRow { id: string(a, "id"), email: string(a, "email"), scopes: string(a, "scopes"), line: string(a, "line"), tone: if a["tone"] == "bad" { "bad".into() } else { "ok".into() },
            /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ bots: if bots.is_empty() { "Bots: none".into() } else { format!("Bots: {bots}") } }
    }).collect()
}
/// Bots with a toggle per account: (account id, email, on). A bot with no accounts to choose from is left out.
pub fn bot_rows(v: &Value) -> Vec<BotRow> {
    v["bots"].as_array().into_iter().flatten().map(|b| BotRow { name: { let n = string(b, "name"); if n.is_empty() { string(b, "id") } else { n } }, grants: b["accounts"].as_array().into_iter().flatten().map(|g| (string(g, "account"), string(g, "email"), g["on"] == true)).collect() })
        .filter(|b| !b.grants.is_empty()).collect()
}
/// The OAuth client's line, only when it is not ready.
pub fn client_line(v: &Value) -> Option<String> { (v["client"]["ready"] != true).then(|| string(&v["client"], "line")).filter(|l| !l.is_empty()) }
/// The Add button's words; empty hides it.
pub fn add_label(v: &Value) -> Option<String> { Some(string(v, "add")).filter(|l| !l.is_empty()) }
pub fn busy(v: &Value) -> bool { v["signin"]["busy"] == true }
pub fn wants_paste(v: &Value) -> bool { v["signin"]["paste"] == true }

// ---- listener --------------------------------------------------------------------------------------------------------------------------------------

/* i18n-ok: notices and button labels are drawn through trs; HTTP text */ const PAGE: &str = "<!doctype html><meta charset=utf-8><title>Signed in</title><body style=\"font:16px sans-serif;margin:3em\">Signed in. You can close this tab.</body>";
/* i18n-ok: notices and button labels are drawn through trs; HTTP text */ fn answer(mut s: TcpStream, status: &str, body: &str) { let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}", body.len()); }
fn serve_one(mut s: TcpStream, port: u16) -> Option<String> {
    let _ = s.set_nonblocking(false); let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let mut buf = Vec::new(); let mut chunk = [0u8; 1024];
    while buf.len() < 8192 && !buf.windows(4).any(|w| w == b"\r\n\r\n") { match s.read(&mut chunk) { Ok(0) | Err(_) => break, Ok(n) => buf.extend_from_slice(&chunk[..n]) } }
    /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ match classify(&String::from_utf8_lossy(&buf), port) { Some(url) => { answer(s, "200 OK", PAGE); Some(url) } None => { answer(s, "404 Not Found", "Not found"); None } }
}
/// Accepts until one good request, the stop flag, or the sign-in's 15 minutes; the caught address goes down the channel.
fn listen(l: TcpListener, port: u16, stop: Arc<AtomicBool>, tx: async_channel::Sender<String>) {
    let _ = l.set_nonblocking(true);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15 * 60);
    while !stop.load(Ordering::Relaxed) && std::time::Instant::now() < end {
        match l.accept() {
            Ok((s, _)) => if let Some(url) = serve_one(s, port) { let _ = tx.send_blocking(url); return },
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
}

// ---- requests --------------------------------------------------------------------------------------------------------------------------------------

pub fn ask(this: &mut Backplane, params: Value) {
    /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ if this.tx.try_send(Command::Rpc("google".into(), Secret(params), "google")).is_err() { this.screens.acct.google.notice = "The hub is not reachable. Try again.".into(); }
}
pub fn view(this: &mut Backplane) { ask(this, json!({"op": "view"})) }
fn stop_listener(g: &mut Google) { if let Some(s) = g.stop.take() { s.store(true, Ordering::Relaxed) } }

/// Settings → Accounts opened: the Google View is fresh.
pub fn open_page(this: &mut Backplane) { this.screens.acct.google.armed.clear(); view(this) }

/// Add Google account, or Sign in again for `account`: the listener first, then the hub is told where the browser comes back.
fn start_signin(this: &mut Backplane, account: Option<String>, cx: &mut Context<Backplane>) {
    stop_listener(&mut this.screens.acct.google);
    let g = &mut this.screens.acct.google; g.notice.clear(); g.opened.clear(); g.want_open = true;
    /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ let listener = match TcpListener::bind(("127.0.0.1", 0)) { Ok(l) => l, Err(_) => { g.notice = "Could not open a local port for the sign-in. Paste the address instead.".into(); ask(this, json!({"op": "signin"})); return } };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let stop = Arc::new(AtomicBool::new(false)); g.stop = Some(stop.clone());
    let (tx, rx) = async_channel::bounded::<String>(1);
    std::thread::spawn(move || listen(listener, port, stop, tx));
    cx.spawn(async move |me, cx| { if let Ok(url) = rx.recv().await { let _ = me.update(cx, |this, cx| { send_code(this, &url); cx.notify() }); } }).detach();
    let mut p = json!({"op": "signin", "redirect": redirect_for(port)});
    if let Some(a) = account { p["account"] = json!(a) }
    ask(this, p);
}
fn send_code(this: &mut Backplane, code: &str) {
    let attempt = string(&this.screens.acct.google.view["signin"], "attempt");
    if attempt.is_empty() { return }
    ask(this, json!({"op": "code", "attempt": attempt, "code": code}));
}
fn paste(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.acct.google.paste.clone() else { return };
    let code = input.read(cx).content.to_string();
    /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ if code.trim().is_empty() { this.screens.acct.google.notice = "Paste the address the browser landed on.".into(); return }
    send_code(this, code.trim());
    input.update(cx, |i, cx| i.set(String::new(), cx));
}
fn cancel(this: &mut Backplane) {
    let attempt = string(&this.screens.acct.google.view["signin"], "attempt");
    stop_listener(&mut this.screens.acct.google);
    if !attempt.is_empty() { ask(this, json!({"op": "cancel", "attempt": attempt})) }
}

/// The hub's View: kept, the link opened once, the paste box made, and polling while the sign-in waits.
pub fn got(this: &mut Backplane, v: Value, cx: &mut Context<Backplane>) {
    if !v.is_object() { return }
    let g = &mut this.screens.acct.google;
    g.notice = string(&v, "note");
    let attempt = string(&v["signin"], "attempt"); let url = string(&v["signin"], "url");
    if !attempt.is_empty() && !url.is_empty() && g.opened != attempt && g.want_open && busy(&v) { g.opened = attempt.clone(); g.want_open = false; cx.open_url(&url); }
    if !busy(&v) { stop_listener(g); }
    if wants_paste(&v) && g.paste.is_none() {
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        input.update(cx, |i, _| { i.set_placeholder(&crate::i18n::trs("Paste the address the browser landed on")); i.masked = true; });
        cx.subscribe(&input, |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { paste(this, cx) }).detach();
        this.screens.acct.google.paste = Some(input);
    }
    let g = &mut this.screens.acct.google;
    if !wants_paste(&v) { g.paste = None }
    g.view = v;
    poll(this, cx);
}
/// The hub says an account changed: the View is asked again when the section has been shown.
pub fn changed(this: &mut Backplane) { if this.screens.acct.google.view.is_object() { view(this) } }
/* i18n-ok: notices and button labels are drawn through trs; HTTP text */ pub fn done(this: &mut Backplane, ok: bool, why: &str) { if !ok { this.screens.acct.google.notice = if why.is_empty() { "The hub refused that.".into() } else { why.to_string() } } }
fn poll(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if !busy(&this.screens.acct.google.view) || this.screens.acct.google.polling { return }
    this.screens.acct.google.polling = true;
    cx.spawn(async move |me, cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_millis(1500)).await;
            let go = me.update(cx, |this, cx| { let on = busy(&this.screens.acct.google.view); if on { view(this) } else { this.screens.acct.google.polling = false } cx.notify(); on });
            if !matches!(go, Ok(true)) { break }
        }
    }).detach();
}

// ---- the section -----------------------------------------------------------------------------------------------------------------------------------

pub fn section(cx: &mut Context<Backplane>, light: bool, g: &Google) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let v = &g.view;
    if !v.is_object() { return vec![] }
    let mut out: Vec<AnyElement> = vec![div().pt_3().pb_2().text_size(px(14.)).font_weight(FontWeight::BOLD).child("Google" /* i18n-ok: the product name */).into_any_element()];
    if !g.notice.is_empty() { out.push(div().mb_3().p_3().bg(c("raise")).text_size(px(12.)).text_color(c("dim")).child(crate::i18n::trs(&g.notice)).into_any_element()); }
    for (i, a) in account_rows(v).into_iter().enumerate() {
        let (id, armed) = (a.id.clone(), g.armed == a.id);
        let head = div().flex().items_center().gap_3()
            .child(div().flex().flex_col().gap(px(2.)).flex_1().min_w(px(140.))
                .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).child(a.email.clone()))
                .child(div().text_size(px(12.)).text_color(c("dim")).child(a.scopes.clone()))
                .child(div().text_size(px(12.)).text_color(c("dim")).child(crate::i18n::trs(&a.bots))))
            .child(div().flex().items_center().gap_2().flex_shrink_0().child(div().size(px(8.)).bg(c(&a.tone))).child(div().text_size(px(12.)).text_color(c(&a.tone)).child(a.line.clone())));
        let mut row = div().flex().items_center().gap_2().mt_3().flex_wrap();
        let aid = id.clone();
        /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ row = row.child(button(cx, light, format!("g-again-{i}"), "Sign in again", false, false, move |this, cx| start_signin(this, Some(aid.clone()), cx)));
        if armed {
            let rid = id.clone();
            /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ row = row.child(button(cx, light, format!("g-remove-{i}"), "Confirm remove", false, true, move |this, _| { this.screens.acct.google.armed.clear(); ask(this, json!({"op": "remove", "account": rid})) }))
                /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ .child(button(cx, light, format!("g-keep-{i}"), "Keep", false, false, |this, _| this.screens.acct.google.armed.clear()));
        } else {
            let rid = id.clone();
            /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ row = row.child(button(cx, light, format!("g-delete-{i}"), "Remove", false, true, move |this, _| this.screens.acct.google.armed = rid.clone()));
        }
        out.push(div().flex().flex_col().p_4().mb_3().border_1().border_color(c("line")).bg(c("view")).child(head).child(row).into_any_element());
    }
    for (bi, b) in bot_rows(v).into_iter().enumerate() {
        let mut toggles = div().flex().items_center().gap_2().flex_wrap();
        for (ai, (account, email, on)) in b.grants.into_iter().enumerate() {
            let bot = string(&v["bots"][bi_of(v, &b.name)], "id");
            let label = format!("{} {}", if on { "✓" } else { "○" }, email);
            toggles = toggles.child(button(cx, light, format!("g-grant-{bi}-{ai}"), &label, on, false, move |this, _| ask(this, json!({"op": "grant", "bot": bot, "account": account, "on": !on}))));
        }
        out.push(div().flex().flex_col().gap_2().p_3().mb_2().border_1().border_color(c("line")).bg(c("view")).child(div().text_size(px(13.)).child(b.name)).child(toggles).into_any_element());
    }
    if let Some(l) = client_line(v) { out.push(div().mb_3().p_3().bg(c("raise")).text_size(px(12.)).text_color(c("dim")).child(l).into_any_element()); }
    if v["signin"].is_object() && v["signin"]["state"] != "done" {
        let s = &v["signin"];
        let mut col = div().flex().flex_col().gap_2().mb_3().p_3().bg(c("panel")).border_1().border_color(c("line"));
        let text = string(s, "text"); if !text.is_empty() { col = col.child(div().text_size(px(12.)).text_color(c("dim")).child(text)); }
        let err = string(s, "error"); if !err.is_empty() { col = col.child(div().text_size(px(12.)).text_color(c("bad")).child(err)); }
        if let Some(input) = g.paste.as_ref().filter(|_| wants_paste(v)) {
            col = col.child(div().flex().items_center().gap_2()
                .child(div().flex_1().h(px(28.)).px_2().flex().items_center().bg(c("raise")).child(input.clone()))
                /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ .child(button(cx, light, "g-paste".into(), "Submit", true, false, |this, cx| paste(this, cx))));
        }
        /* i18n-ok: notices and button labels are drawn through trs; HTTP text */ col = col.child(div().flex().child(button(cx, light, "g-cancel".into(), "Cancel", false, false, |this, _| cancel(this))));
        out.push(col.into_any_element());
    }
    if let Some(label) = add_label(v) { out.push(div().flex().child(button(cx, light, "g-add".into(), &label, true, false, |this, cx| start_signin(this, None, cx))).into_any_element()); }
    out
}
fn bi_of(v: &Value, name: &str) -> usize { v["bots"].as_array().and_then(|a| a.iter().position(|b| { let n = string(b, "name"); (if n.is_empty() { string(b, "id") } else { n }) == name })).unwrap_or(0) }

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test; // gpui exports its own `test`
    fn view() -> Value {
        json!({"client":{"ready":false,"line":"Add the OAuth client."},"add":"Add Google account",
            "accounts":[{"id":"a1","email":"me@x.com","scopes":"Gmail, Calendar","state":"connected","line":"connected","tone":"ok","bots":"Calendar, Email-Whipper"},
                {"id":"a2","email":"you@x.com","scopes":"Gmail","state":"expired","line":"sign-in expired","tone":"bad","bots":""}],
            "bots":[{"id":"cal","name":"Calendar","accounts":[{"account":"a1","email":"me@x.com","on":true},{"account":"a2","email":"you@x.com","on":false}]},{"id":"none","name":"Empty","accounts":[]}],
            "signin":{"attempt":"s1","url":"https://accounts.google.com/x","busy":true,"paste":true,"state":"waiting"},"note":""})
    }
    #[test] fn the_request_line_must_be_the_one_get() {
        assert_eq!(request_target("GET /oauth/google?code=a&state=b HTTP/1.1\r\nHost: x\r\n\r\n").as_deref(), Some("/oauth/google?code=a&state=b"));
        for bad in ["GET /favicon.ico HTTP/1.1", "GET /oauth/google HTTP/1.1", "GET /oauth/google? HTTP/1.1", "POST /oauth/google?code=a HTTP/1.1", "GET /oauth/googlex?code=a HTTP/1.1", "", "GET"] { assert_eq!(request_target(bad), None, "{bad}"); }
    }
    #[test] fn the_address_sent_is_the_redirect_with_the_query_and_the_host_must_be_ours() {
        let head = "GET /oauth/google?code=a%2Fb&state=s HTTP/1.1\r\nHost: 127.0.0.1:4001\r\n\r\n";
        assert_eq!(classify(head, 4001).as_deref(), Some("http://127.0.0.1:4001/oauth/google?code=a%2Fb&state=s"));
        assert_eq!(classify(head, 4002), None, "another port's name");
        assert_eq!(classify("GET /oauth/google?code=a HTTP/1.1\r\nHost: evil.example\r\n\r\n", 4001), None);
        assert_eq!(classify("GET /oauth/google?code=a HTTP/1.1\r\n\r\n", 4001), None, "no Host");
        assert!(host_ok("GET / HTTP/1.1\r\nhost: localhost:4001\r\n", 4001));
        assert_eq!(redirect_for(7), "http://127.0.0.1:7/oauth/google");
    }
    #[test] fn the_section_is_drawn_from_the_view() {
        let v = view();
        let a = account_rows(&v);
        assert_eq!((a[0].email.as_str(), a[0].scopes.as_str(), a[0].line.as_str(), a[0].tone.as_str(), a[0].bots.as_str()), ("me@x.com", "Gmail, Calendar", "connected", "ok", "Bots: Calendar, Email-Whipper"));
        assert_eq!((a[1].tone.as_str(), a[1].bots.as_str()), ("bad", "Bots: none"));
        let b = bot_rows(&v); assert_eq!(b.len(), 1, "a bot with no account to choose is left out");
        assert_eq!(b[0].grants, vec![("a1".into(), "me@x.com".into(), true), ("a2".into(), "you@x.com".into(), false)]);
        assert_eq!(client_line(&v).as_deref(), Some("Add the OAuth client."));
        let mut ready = v.clone(); ready["client"]["ready"] = json!(true); assert_eq!(client_line(&ready), None);
        assert_eq!(add_label(&v).as_deref(), Some("Add Google account")); assert_eq!(add_label(&json!({"add":""})), None);
        assert!(busy(&v) && wants_paste(&v)); assert!(!busy(&json!({"signin":null})));
    }
    #[test] fn the_listener_catches_one_return_and_answers_the_rest_404() {
        use std::io::{Read, Write};
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap(); let port = l.local_addr().unwrap().port();
        let (tx, rx) = async_channel::bounded(1); let stop = Arc::new(AtomicBool::new(false));
        let t = std::thread::spawn(move || listen(l, port, stop, tx));
        let get = |path: &str| { let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap(); write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").unwrap(); let mut r = String::new(); s.read_to_string(&mut r).unwrap(); r };
        assert!(get("/favicon.ico").starts_with("HTTP/1.1 404"));
        let ok = get("/oauth/google?code=c&state=s"); assert!(ok.starts_with("HTTP/1.1 200") && ok.contains("Signed in. You can close this tab."));
        assert_eq!(rx.recv_blocking().unwrap(), format!("http://127.0.0.1:{port}/oauth/google?code=c&state=s")); t.join().unwrap();
    }
}

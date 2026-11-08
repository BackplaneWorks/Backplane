//! Accounts: Settings → Accounts (add, sign in, refresh, disconnect, delete, default for new threads) and the composer's account chip with its menu.
//! The hub keeps the profiles (host `settings/accounts*.rs`); this file asks it with the `accounts` request (the reply's `result` is the snapshot: profiles,
//! sign-in attempts with their link and code, and, when a thread is named, what that thread is bound to) and sends `accounts.select` for a pick.
//! The sign-in code Claude shows is typed in a field here and goes straight to that attempt (`respond`); it is never a draft, a `bfield` or a log line.
use super::composer::{Composer, ComposerEvent};
use super::settings::Secret;
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Default)]
pub struct Accounts {
    /// The newest snapshot the hub sent (Null before the first answer).
    pub snap: Value,
    /// The thread the chip last asked about, and whether the menu under the chip is open.
    pub asked_for: String,
    pub menu: bool,
    /// Whether the limits menu (every account's remaining allowance) is open.
    pub limits: bool,
    /// The menu's keyboard: its digits work while this has focus (context `AccountMenu`), wherever the keyboard was.
    pub focus: Option<FocusHandle>,
    /// The profile whose Delete was pressed once (the second press deletes).
    pub armed: String,
    /// The last refusal or hint, said where the person is looking.
    pub notice: String,
    /// What a bulk move did (not a refusal: said in the plain colour).
    pub info: String,
    pub label: Option<Entity<Composer>>,
    pub codes: HashMap<String, Entity<Composer>>,
    pub polling: bool,
    /// Attempts whose code was sent and not yet answered (the hub checks the sign-in before it says).
    pub checking: std::collections::HashSet<String>,
    /// Which provider the add box makes an account for ("" is Claude).
    pub add_provider: String,
}

// ---- pure: what the snapshot says -----------------------------------------------------------------------------------------------------------------------

pub fn profiles(snap: &Value) -> Vec<Value> { snap["profiles"].as_array().cloned().unwrap_or_default() }
/// A sign-in attempt still going (the hub keeps finished ones in the list for a while).
pub fn is_live(a: &Value) -> bool { matches!(a["state"].as_str(), Some("starting" | "waiting" | "pending")) }
pub fn attempts_of(snap: &Value, profile: &str) -> Vec<Value> { snap["attempts"].as_array().into_iter().flatten().filter(|a| a["profile"] == profile && is_live(a)).cloned().collect() }
/// Why the newest finished attempt of a profile failed, when it did.
pub fn failure_of(snap: &Value, profile: &str) -> String { snap["attempts"].as_array().into_iter().flatten().filter(|a| a["profile"] == profile && !is_live(a)).filter_map(|a| a["message"].as_str().filter(|m| !m.is_empty()).map(str::to_string)).last().unwrap_or_default() }
/// What the thread is bound to: (profile id, epoch); the CLI's own login is ("", 0).
pub fn binding(snap: &Value) -> (String, u64) {
    let b: Value = serde_json::from_str(snap["thread"]["binding"].as_str().unwrap_or("")).unwrap_or(Value::Null);
    (b["id"].as_str().unwrap_or("").to_string(), b["epoch"].as_u64().unwrap_or(0))
}
/// The chip's words: the bound account's label, else "Default".
pub fn chip_label(snap: &Value) -> String {
    let (id, _) = binding(snap);
    if id.is_empty() { return "Default".into() }
    profiles(snap).iter().find(|p| p["id"] == id.as_str()).and_then(|p| p["label"].as_str().map(str::to_string)).unwrap_or_else(|| "Unavailable".into())
}
/// The words and the colour token of a state.
pub fn badge(state: &str) -> (&'static str, &'static str) {
    match state { "connected" => ("Connected", "ok"), "connecting" => ("Signing in", "accent"), "unchecked" => ("Not checked", "warn"), "expired" => ("Sign-in expired", "bad"), _ => ("Not connected", "faint") }
}
/// Whether the hub is waiting for the person or the provider (so the page asks again every couple of seconds).
pub fn wants_poll(snap: &Value) -> bool {
    snap["attempts"].as_array().map_or(false, |a| a.iter().any(is_live))
        || profiles(snap).iter().any(|p| p["state"] == "connecting")
}
/// The identity line of a profile: "Claude · a@b.com · max".
pub fn identity(p: &Value) -> String {
    let mut parts = vec![if p["provider"] == "codex" { "Codex".to_string() } else { "Claude".to_string() }];
    for k in ["email", "plan"] { if let Some(s) = p["identity"][k].as_str().filter(|s| !s.is_empty()) { parts.push(s.to_string()) } }
    parts.join(" · ")
}
/// The most used window of a profile's allowance as "62% of 5-hour used", when the provider reported one.
pub fn allowance(p: &Value) -> String {
    let w = p["allowance"]["windows"].as_array().into_iter().flatten().filter_map(|w| Some((w["used_percent"].as_f64()?, w["scope"].as_str().unwrap_or("").to_string()))).fold(None::<(f64, String)>, |a, b| match a { Some(x) if x.0 >= b.0 => Some(x), _ => Some(b) });
    match w { Some((u, scope)) => format!("{:.0}% of {} used", u, scope.replace('_', "-").replace("five-hour", "5-hour").replace("seven-day", "7-day")), None => String::new() }
}
/// One line of the account menu: the key that picks it, what it is, what it has left, and where a press goes.
#[derive(Debug, PartialEq, Clone)]
pub struct MenuRow { pub key: Option<u8>, pub label: String, pub detail: String, pub left: String, pub out: bool, pub on: bool, pub usable: bool, pub profile: String, pub epoch: u64 }
/// What one login has left, in a few words: the hub's word when it is out, else its fullest window, else nothing known.
fn left_of(holds: &Value, id: &str, al: &Value, now: u64) -> (String, bool) {
    if let Some(t) = holds[id]["text"].as_str() { return (t.to_string(), true) }
    let fullest = al["windows"].as_array().into_iter().flatten().filter(|w| w["used_percent"].as_f64().or_else(|| w["used_permille"].as_f64().map(|p| p / 10.0)).is_some())
        .max_by(|a, b| { let u = |w: &Value| w["used_percent"].as_f64().or_else(|| w["used_permille"].as_f64().map(|p| p / 10.0)).unwrap_or(0.0); u(a).total_cmp(&u(b)) });
    (fullest.and_then(|w| window_line(w, now)).unwrap_or_default(), false)
}
/// The menu's rows: the thread's provider's logins, the CLI's own first, each of the first nine with its digit. The hub's snapshot decides all of it.
pub fn menu_rows(snap: &Value, now: u64) -> Vec<MenuRow> {
    let (bound, _) = binding(snap);
    let provider = snap["thread"]["provider"].as_str().filter(|p| !p.is_empty()).unwrap_or("claude");
    let holds = &snap["holds"];
    let (left, out) = left_of(holds, "", if provider == "claude" { &snap["default_allowance"] } else { &Value::Null }, now);
    let mut rows = vec![MenuRow { key: None, label: "Default account".into(), detail: "CLI login".into(), left, out, on: bound.is_empty(), usable: true, profile: String::new(), epoch: 0 }];
    for p in profiles(snap).iter().filter(|p| p["provider"] == provider) {
        let (id, ok) = (string(p, "id"), p["state"] == "connected");
        let (left, out) = left_of(holds, &id, &p["allowance"], now);
        let detail = if ok { p["identity"]["email"].as_str().unwrap_or("").to_string() } else { badge(p["state"].as_str().unwrap_or("")).0.to_string() };
        rows.push(MenuRow { key: None, label: string(p, "label"), detail, left, out, on: id == bound, usable: ok, profile: id, epoch: p["epoch"].as_u64().unwrap_or(0) });
    }
    for (i, r) in rows.iter_mut().enumerate().take(9) { r.key = Some(i as u8 + 1) }
    rows
}
/// The `accounts.select` request for a pick (the binding the menu was drawn from rides along, so a stale menu is refused).
pub fn select_params(thread: &str, profile: &str, epoch: u64, snap: &Value) -> Value {
    json!({"thread": thread, "profile": profile, "epoch": epoch, "expected": snap["thread"]["binding"].as_str().unwrap_or("")})
}
/// Whether this profile is the default for new threads of its provider.
pub fn is_default(snap: &Value, p: &Value) -> bool {
    let d: Value = serde_json::from_str(snap["defaults"][p["provider"].as_str().unwrap_or("")].as_str().unwrap_or("")).unwrap_or(Value::Null);
    d["id"] == p["id"] && d["epoch"] == p["epoch"]
}

// ---- requests ------------------------------------------------------------------------------------------------------------------------------------

/// The hub knows a thread by its own id; the picture names it with the hub in front (`hub|id`).
fn current_thread(this: &Backplane) -> String { super::attach::raw_id(&string(&this.data["thread"], "id")).to_string() }
/// An `accounts` request; the snapshot comes back as `Event::Data("acct", ..)`.
pub fn ask(this: &mut Backplane, mut params: Value) {
    let th = current_thread(this);
    if !th.is_empty() { params["thread"] = json!(th); }
    if this.tx.try_send(Command::Rpc("accounts".into(), Secret(params), "acct")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); }
}
pub fn list(this: &mut Backplane) { ask(this, json!({"op": "list"})) }

/// The picture arrived for `selected`: the chip asks once for that thread's account.
pub fn thread_changed(this: &mut Backplane, selected: &str) {
    if this.screens.acct.asked_for == selected { return }
    this.screens.acct.asked_for = selected.to_string(); this.screens.acct.menu = false;
    if !selected.is_empty() { list(this) }
}

/// The hub's snapshot: kept, code boxes made for the sign-ins waiting, and polling started while something waits.
pub fn got(this: &mut Backplane, v: Value, cx: &mut Context<Backplane>) {
    if !v.is_object() { return }
    this.screens.acct.notice = v["error"].as_str().unwrap_or("").to_string();
    for a in v["attempts"].as_array().into_iter().flatten().filter(|a| is_live(a)) {
        let id = a["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() || this.screens.acct.codes.contains_key(&id) { continue }
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        input.update(cx, |i, _| { i.set_placeholder("Paste the code from Claude's page"); i.masked = true; });
        let aid = id.clone();
        cx.subscribe(&input, move |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { respond(this, &aid, cx) }).detach();
        this.screens.acct.codes.insert(id, input);
    }
    let live: Vec<String> = v["attempts"].as_array().into_iter().flatten().filter(|a| is_live(a)).filter_map(|a| a["id"].as_str().map(str::to_string)).collect();
    this.screens.acct.codes.retain(|k, _| live.contains(k));
    let said: Vec<String> = v["attempts"].as_array().into_iter().flatten().filter(|a| !a["message"].as_str().unwrap_or("").is_empty()).filter_map(|a| a["id"].as_str().map(str::to_string)).collect();
    this.screens.acct.checking.retain(|k| live.contains(k) && !said.contains(k));
    this.screens.acct.snap = v;
    super::onboard::accounts_changed(this);
    poll(this, cx);
}
/// An answer to one of the requests: a refusal is said; a success asks for the snapshot again where the request has none of its own.
pub fn done(this: &mut Backplane, tag: &str, ok: bool, why: &str, _cx: &mut Context<Backplane>) {
    if !ok {
        // a refusal of an `accounts` request carries its reason in the snapshot's `error` (kept by `got`); only a request that has none is given a plain sentence
        if !why.is_empty() { this.screens.acct.notice = why.to_string() } else if this.screens.acct.notice.is_empty() { this.screens.acct.notice = "The hub refused that.".into() }
        return
    }
    if tag == "acct-moveall" { list(this); return }
    if tag == "acct-select" || tag == "acct-default" { if this.screens.acct.menu { this.want_focus = true } this.screens.acct.menu = false; this.screens.acct.notice.clear(); list(this); }
}
/// The answer to "Move all threads here": how many moved, and the running ones it left (a turn is never switched under itself; they can be moved when it ends).
pub fn moved(this: &mut Backplane, v: &Value) { this.screens.acct.info = moved_text(v); }
pub fn moved_text(v: &Value) -> String {
    let n = |k: &str| v[k].as_u64().unwrap_or(0); let len = |k: &str| v[k].as_array().map_or(0, |a| a.len());
    let (moved, already, running, failed) = (n("moved"), n("already"), len("running"), len("failed"));
    let threads = |k: u64| if k == 1 { "1 thread".to_string() } else { format!("{k} threads") };
    let mut s = if moved == 0 && already > 0 { format!("All {} were already on this account.", threads(already)) } else { format!("Moved {} to this account.", threads(moved)) };
    if running > 0 { s += &format!(" {} with a turn running {} not moved; press again when {} done.", threads(running as u64), if running == 1 { "was" } else { "were" }, if running == 1 { "it is" } else { "they are" }) }
    if failed > 0 { s += &format!(" {} could not be moved.", threads(failed as u64)) }
    s
}
fn poll(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if !wants_poll(&this.screens.acct.snap) || this.screens.acct.polling { return }
    this.screens.acct.polling = true;
    cx.spawn(async move |me, cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
            let go = me.update(cx, |this, cx| { let on = wants_poll(&this.screens.acct.snap); if on { list(this) } else { this.screens.acct.polling = false } cx.notify(); on });
            if !matches!(go, Ok(true)) { break }
        }
    }).detach();
}

/// Settings → Accounts opened (or the chip's "Manage accounts"): the add box exists and the list is fresh.
pub fn open_page(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.acct.label.is_none() {
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        input.update(cx, |i, _| i.set_placeholder("Account label (e.g. Work)"));
        cx.subscribe(&input, |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { add(this, cx) }).detach();
        this.screens.acct.label = Some(input);
    }
    this.screens.acct.armed.clear();
    list(this);
}
fn add(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.acct.label.clone() else { return };
    let label = input.read(cx).content.to_string();
    if label.trim().is_empty() { this.screens.acct.notice = "Name the account first, e.g. Work or Personal.".into(); return }
    let provider = if this.screens.acct.add_provider == "codex" { "codex" } else { "claude" };
    ask(this, json!({"op": "add", "provider": provider, "label": label.trim()}));
    input.update(cx, |i, cx| i.set(String::new(), cx));
}
fn respond(this: &mut Backplane, attempt: &str, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.acct.codes.get(attempt).cloned() else { return };
    let profile = this.screens.acct.snap["attempts"].as_array().into_iter().flatten().find(|a| a["id"] == attempt).map(|a| string(a, "profile")).unwrap_or_default();
    let code = input.read(cx).content.to_string();
    if code.trim().is_empty() { this.screens.acct.notice = "Paste the code Claude shows after you sign in.".into(); return }
    this.screens.acct.checking.insert(attempt.to_string()); this.screens.acct.notice.clear();
    ask(this, json!({"op": "respond", "profile": profile, "attempt": attempt, "code": code.trim()}));
}

// ---- the page --------------------------------------------------------------------------------------------------------------------------------------

fn button(cx: &mut Context<Backplane>, light: bool, id: String, label: &str, primary: bool, danger: bool, on: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).h(px(28.)).px_3().flex().items_center().justify_center().text_size(px(12.)).cursor_pointer();
    let d = if primary { d.bg(c("accent")).text_color(c("on-accent")) } else if danger { d.bg(c("raise")).text_color(c("bad")).hover(|s| s.bg(c("line"))) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
    d.child(label.to_string()).on_click(cx.listener(move |this, _, _, cx| { on(this, cx); cx.notify(); })).into_any_element()
}

fn attempt_panel(cx: &mut Context<Backplane>, light: bool, st: &Accounts, a: &Value, pid: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (aid, url, code, msg) = (string(a, "id"), string(a, "url"), string(a, "userCode"), string(a, "message"));
    let mut col = div().flex().flex_col().gap_2().mt_3().p_3().bg(c("panel")).border_1().border_color(c("line"));
    let ready = !url.is_empty();
    let help = if !ready { "Generating the sign-in link…" } else if code.is_empty() { "Open the link, sign in to Claude, then paste the code it shows." } else { "Open the link and enter this code." };
    col = col.child(div().text_size(px(12.)).text_color(c(if ready { "dim" } else { "accent" })).child(help));
    if !code.is_empty() { col = col.child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child(code.clone())); }
    if !url.is_empty() {
        let (u1, u2) = (url.clone(), url.clone());
        col = col.child(div().flex().items_center().gap_2()
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(11.)).text_color(c("faint")).child(url))
            .child(button(cx, light, format!("acct-open-{aid}"), "Open", true, false, move |_, cx| cx.open_url(&u1)))
            .child(button(cx, light, format!("acct-copy-{aid}"), "Copy link", false, false, move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(u2.clone())))));
    }
    if let Some(input) = st.codes.get(&aid).filter(|_| code.is_empty() && ready) {
        let (a1, a2, p2) = (aid.clone(), aid.clone(), pid.to_string());
        let _ = (&a2, &p2);
        col = col.child(div().flex().items_center().gap_2()
            .child(div().flex_1().h(px(28.)).px_2().flex().items_center().bg(c("raise")).child(input.clone()))
            .child(button(cx, light, format!("acct-code-{aid}"), "Submit code", true, false, move |this, cx| respond(this, &a1, cx))));
    }
    if st.checking.contains(&aid) { col = col.child(div().text_size(px(12.)).text_color(c("accent")).child("Checking the code and your sign-in…")); }
    if !msg.is_empty() { col = col.child(div().text_size(px(12.)).text_color(c("bad")).child(msg)); }
    let (aid2, pid2) = (aid.clone(), pid.to_string());
    col.child(div().flex().child(button(cx, light, format!("acct-cancel-{aid}"), "Cancel sign-in", false, false, move |this, _| ask(this, json!({"op": "cancel", "profile": pid2, "attempt": aid2}))))).into_any_element()
}

/// `narrow`: the state goes under the name instead of beside it (a page under ADD_ROW_MIN wide).
fn card(cx: &mut Context<Backplane>, light: bool, st: &Accounts, p: &Value, i: usize, narrow: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (id, state) = (string(p, "id"), string(p, "state"));
    let (word, tok) = badge(&state);
    let epoch = p["epoch"].as_u64().unwrap_or(0);
    let default = is_default(&st.snap, p);
    let mut sub = vec![identity(p)];
    let al = allowance(p); if !al.is_empty() { sub.push(al) }
    let head = if narrow { div().flex().flex_col().items_start() } else { div().flex().items_center() }.gap_3()
        .child(div().flex().flex_col().gap(px(2.)).flex_1().min_w(px(140.)).max_w_full()
            .child(div().flex().items_center().gap_2().child(super::picker::logo(if p["provider"] == "codex" { "codex" } else { "claude" })).child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).child(string(p, "label")))
                .when(default, |d| d.child(div().px_2().py(px(1.)).bg(c("raise")).text_size(px(11.)).text_color(c("dim")).child("New threads"))))
            .child(div().text_size(px(12.)).text_color(c("dim")).child(sub.join("  ·  "))))
        .child(div().flex().items_center().gap_2().flex_shrink_0().child(div().size(px(8.)).bg(c(tok))).child(div().text_size(px(12.)).text_color(c(tok)).child(word)));
    let mut row = div().flex().items_center().gap_2().mt_3().flex_wrap();
    let armed = st.armed == id;
    let mut btn = |label: &str, key: &str, primary: bool, danger: bool, on: Box<dyn Fn(&mut Backplane, &mut Context<Backplane>)>| { row = std::mem::replace(&mut row, div()).child(button(cx, light, format!("acct-{key}-{i}"), label, primary, danger, on)); };
    let live = !attempts_of(&st.snap, &id).is_empty() || state == "connecting";
    let pid = id.clone();
    if !live {
        if state == "connected" {
            btn("Refresh", "status", false, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "status", "profile": pid, "asked": true})) }));
            if !default { let (prov, pid2) = (string(p, "provider"), pid.clone()); btn("Use for new threads", "default", false, false, Box::new(move |this, _| { if this.tx.try_send(Command::Rpc("accounts.default".into(), Secret(json!({"provider": prov, "profile": pid2, "epoch": epoch})), "acct-default")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); } })); }
            { let (prov, pid2) = (string(p, "provider"), pid.clone()); btn("Move all threads here", "moveall", false, false, Box::new(move |this, _| { this.screens.acct.notice.clear(); this.screens.acct.info.clear(); if this.tx.try_send(Command::Rpc("accounts.move_all".into(), Secret(json!({"provider": prov, "profile": pid2, "epoch": epoch, "default": true})), "acct-moveall")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); } })); }
            btn("Sign out", "logout", false, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "disconnect", "profile": pid})) }));
        } else {
            if state == "expired" { btn("Check now", "status", false, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "status", "profile": pid, "asked": true})) })); }
            btn(if state == "unchecked" { "Check sign-in" } else if state == "expired" { "Sign in again" } else { "Connect" }, "login", state != "unchecked", false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": if state_of(&this.screens.acct.snap, &pid) == "unchecked" { "status" } else { "login" }, "profile": pid})) }));
        }
    }
    // Delete is always there: it is the way out of a sign-in that is stuck (the hub ends the attempt first)
    if armed {
        btn("Confirm delete", "remove", false, true, Box::new({ let pid = pid.clone(); move |this, _| { this.screens.acct.armed.clear(); ask(this, json!({"op": "remove", "profile": pid})) } }));
        btn("Keep", "keep", false, false, Box::new(|this, _| this.screens.acct.armed.clear()));
    } else {
        btn("Delete", "delete", false, true, Box::new({ let pid = pid.clone(); move |this, _| this.screens.acct.armed = pid.clone() }));
    }
    let mut card = div().flex().flex_col().p_4().mb_3().border_1().border_color(c("line")).bg(c("view")).child(head).child(row);
    for a in attempts_of(&st.snap, &id) { card = card.child(attempt_panel(cx, light, st, &a, &id)); }
    let why = failure_of(&st.snap, &id); if !why.is_empty() && attempts_of(&st.snap, &id).is_empty() { card = card.child(div().mt_2().text_size(px(12.)).text_color(c("bad")).child(why)); }
    card.into_any_element()
}
fn state_of(snap: &Value, id: &str) -> String { profiles(snap).iter().find(|p| p["id"] == id).map(|p| string(p, "state")).unwrap_or_default() }

/// The page: the profiles as cards, then the add box.
/// The width under which the label box and Add go one under the other (settings::Fit::rows).
pub const ADD_ROW_MIN: f32 = 380.0;
/// `avail`: the width the page's rows are laid out in.
pub fn page(cx: &mut Context<Backplane>, light: bool, st: &Accounts, avail: f32) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let mut out: Vec<AnyElement> = vec![];
    out.push(div().pb_3().text_size(px(12.)).text_color(c("dim")).child("Each account is a separate Claude subscription sign-in kept on this machine. A thread uses the account picked beside its model; when one runs out of allowance the thread carries on with the next account that has some left.").into_any_element());
    if !st.notice.is_empty() { out.push(div().mb_3().p_3().bg(c("raise")).text_size(px(12.)).text_color(c("bad")).child(st.notice.clone()).into_any_element()); }
    if !st.info.is_empty() { out.push(div().mb_3().p_3().bg(c("raise")).text_size(px(12.)).text_color(c("text")).child(st.info.clone()).into_any_element()); }
    // the CLI's own login is always there
    let def_default = !profiles(&st.snap).iter().any(|p| is_default(&st.snap, p));
    // narrow, the state goes under the words instead of squeezing them
    let card_row = if avail < ADD_ROW_MIN { div().flex().flex_col().items_start() } else { div().flex().items_center() };
    out.push(card_row.gap_3().p_4().mb_3().border_1().border_color(c("line")).bg(c("view"))
        .child(div().flex().flex_col().gap(px(2.)).flex_1().min_w(px(160.)).max_w_full().child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).child("Default account")).child(div().text_size(px(12.)).text_color(c("dim")).child("The Claude CLI's own sign-in on this machine, as before.")))
        .child(div().flex().flex_shrink_0().items_center().gap_2().child(div().size(px(8.)).bg(c("ok"))).child(div().text_size(px(12.)).text_color(c("ok")).child(if def_default { "Used for new threads" } else { "Available" }))).into_any_element());
    let ps = profiles(&st.snap);
    for (i, p) in ps.iter().enumerate() { out.push(card(cx, light, st, p, i, avail < ADD_ROW_MIN)); }
    if let Some(input) = &st.label {
        let row = if avail < ADD_ROW_MIN { div().flex().flex_col().items_start().gap_2().mt_2().child(div().w_full().h(px(32.)).px_3().flex().items_center().bg(c("raise")).child(input.clone())) }
            else { div().flex().items_center().gap_2().mt_2().child(div().flex_1().min_w_0().h(px(32.)).px_3().flex().items_center().bg(c("raise")).child(input.clone())) };
        out.push(row
            .child(button(cx, light, "acct-add".into(), "Add Claude account", true, false, |this, cx| add(this, cx))).into_any_element());
    }
    out
}

/// The same sign-in rows for one provider, inside a setup step.
pub fn panel(cx: &mut Context<Backplane>, light: bool, st: &Accounts, provider: &str) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let mut out: Vec<AnyElement> = vec![];
    if !st.notice.is_empty() { out.push(div().p_3().bg(c("raise")).text_size(px(12.)).text_color(c("bad")).child(st.notice.clone()).into_any_element()); }
    for (i, p) in profiles(&st.snap).iter().enumerate().filter(|(_, p)| p["provider"] == provider) { out.push(card(cx, light, st, p, i, false)); }
    if let Some(input) = &st.label {
        let word = if provider == "codex" { "Add Codex account" } else { "Add Claude account" };
        out.push(div().flex().items_center().gap_2().child(div().flex_1().h(px(32.)).px_3().flex().items_center().bg(c("raise")).child(input.clone()))
            .child(button(cx, light, "acct-add".into(), word, true, false, |this, cx| add(this, cx))).into_any_element());
    }
    out
}

// ---- the chip's menu -------------------------------------------------------------------------------------------------------------------------------

/// The menu opened or shut, from its chip or its key. Open, it has the keyboard, so its digits pick wherever the keyboard was; shut, the keyboard goes back to the message box.
pub fn toggle(this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) {
    this.screens.acct.menu = !this.screens.acct.menu;
    if this.screens.acct.menu {
        this.screens.acct.notice.clear(); list(this); refresh_usage(this); // saved sign-ins are rechecked after a restart, and the limits it shows are fresh (the hub reads a profile at most every 5 minutes)
        let h = this.screens.acct.focus.get_or_insert_with(|| cx.focus_handle()).clone(); window.focus(&h);
    } else { this.want_focus = true }
    cx.notify();
}
/// A row pressed, by the mouse or its digit: a usable account is asked for; one that is not signed in goes to where it is signed in.
fn press(this: &mut Backplane, row: &MenuRow, cx: &mut Context<Backplane>) {
    if row.usable {
        let thread = current_thread(this);
        let p = select_params(&thread, &row.profile, row.epoch, &this.screens.acct.snap);
        if this.tx.try_send(Command::Rpc("accounts.select".into(), Secret(p), "acct-select")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); }
    } else { this.screens.acct.menu = false; this.open_settings(cx); this.screens.settings_ui.device = false; this.screens.settings_ui.accounts = true; open_page(this, cx); }
    cx.notify();
}
/// The digit `n` in the open menu.
pub fn press_nth(this: &mut Backplane, n: u8, cx: &mut Context<Backplane>) {
    if !this.screens.acct.menu { return }
    if let Some(r) = menu_rows(&this.screens.acct.snap, now_s()).into_iter().find(|r| r.key == Some(n)) { press(this, &r, cx) }
}

/// The menu above the chip: the default account and each saved account of the thread's provider, each with its digit and what it has left, the bound one ticked; one that cannot be used says why.
pub fn menu(cx: &mut Context<Backplane>, light: bool, st: &Accounts) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut list = div().id("acct-menu").key_context("AccountMenu").w(px(340.)).max_h(px(420.)).overflow_y_scroll().flex().flex_col().py_1().border_1().border_color(c("line")).bg(c("panel")).shadow_lg().occlude()
        .on_mouse_down_out(cx.listener(|this, _, _, cx| { this.screens.acct.menu = false; this.want_focus = true; cx.notify(); }));
    if let Some(h) = &st.focus { list = list.track_focus(h); }
    for (i, r) in menu_rows(&st.snap, now_s()).into_iter().enumerate() {
        let row = r.clone();
        let mut text = div().flex_1().min_w_0().flex().flex_col().gap(px(1.))
            .child(div().flex().items_center().gap_2()
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(13.)).text_color(c(if r.usable { "text" } else { "faint" })).child(r.label))
                .child(div().flex_shrink_0().text_size(px(11.)).text_color(c("faint")).child(r.detail)));
        if !r.left.is_empty() { text = text.child(div().overflow_hidden().whitespace_nowrap().text_size(px(11.)).text_color(c(if r.out { "warn" } else { "dim" })).child(r.left)); }
        list = list.child(div().id(SharedString::from(format!("acct-m-{i}"))).flex().items_center().gap_2().px_3().py(px(6.)).mx_1().cursor_pointer().hover(|s| s.bg(c("raise")))
            .child(div().w(px(16.)).flex_shrink_0().text_size(px(11.)).text_color(c("dim")).child(r.key.map_or(String::new(), |k| k.to_string())))
            .child(div().w(px(14.)).flex_shrink_0().child(if r.on { crate::art::icon("circle-check", 14., c("accent")) } else { div().into_any_element() }))
            .child(text)
            .on_click(cx.listener(move |this, _, _, cx| press(this, &row, cx))));
    }
    if !st.notice.is_empty() { list = list.child(div().px_3().py_1().text_size(px(11.)).text_color(c("bad")).child(st.notice.clone())); }
    list = list.child(div().my_1().mx_2().h(px(1.)).bg(c("line")))
        .child(div().id("acct-m-manage").flex().items_center().gap_2().px_3().py(px(7.)).mx_1().cursor_pointer().hover(|s| s.bg(c("raise"))).text_size(px(13.)).text_color(c("dim"))
            .child("Manage accounts in Settings").on_click(cx.listener(|this, _, _, cx| { this.screens.acct.menu = false; this.want_focus = true; this.open_settings(cx); this.screens.settings_ui.device = false; this.screens.settings_ui.accounts = true; open_page(this, cx); cx.notify(); })));
    list.into_any_element()
}

/// The composer's account chip: the account in force, centred, opening the menu.
pub fn chip(cx: &mut Context<Backplane>, light: bool, st: &Accounts, key: &str) -> Div {
    let c = |n: &str| theme::color(light, n);
    let wrap = div().relative();
    let btn = div().id("acct-chip").h(px(30.)).min_w_0().overflow_hidden().px_3().flex().items_center().justify_center().gap_2().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).whitespace_nowrap().cursor_pointer()
        .hover(|s| s.bg(c("raise")))
        .child(crate::art::icon("user", 16., c("text"))).child(chip_label(&st.snap))
        .tooltip(crate::keys::tip("Account", key, light))
        .on_click(cx.listener(|this, _, window, cx| toggle(this, window, cx)));
    let wrap = wrap.child(btn);
    if st.menu { wrap.child(deferred(anchored().anchor(Corner::BottomLeft).offset(point(px(0.), px(-4.))).snap_to_window_with_margin(px(8.)).child(menu(cx, light, st))).with_priority(10)) } else { wrap }
}

// ---- the limits chip -------------------------------------------------------------------------------------------------------------------------------

/// One account's block in the limits menu: its name, whether the thread uses it, and a line per limit window (or why there is none).
#[derive(Debug, PartialEq)]
pub struct LimitBlock { pub name: String, pub bound: bool, pub lines: Vec<String>, pub note: String }
fn window_name(scope: &str) -> String {
    match scope { "five_hour" => "5-hour".into(), "seven_day" => "Weekly".into(), "seven_day_opus" => "Weekly Opus".into(), "seven_day_sonnet" => "Weekly Sonnet".into(), o => o.replace('_', " ") }
}
/// "in 2d 3h" / "in 4h 5m" / "in 12m" / "now" for a reset time in epoch seconds.
pub fn resets_in(at: u64, now: u64) -> String {
    if at <= now { return "now".into() }
    let m = (at - now + 59) / 60;
    if m >= 1440 { format!("in {}d {}h", m / 1440, (m % 1440) / 60) } else if m >= 60 { format!("in {}h {}m", m / 60, m % 60) } else { format!("in {m}m") }
}
/// A window as "5-hour  72% left · resets in 3h 10m".
pub fn window_line(w: &Value, now: u64) -> Option<String> {
    let used = w["used_percent"].as_f64().or_else(|| w["used_permille"].as_f64().map(|p| p / 10.0)).filter(|u| (0.0..=100.0).contains(u))?;
    let mut s = format!("{}  {:.0}% left", window_name(w["scope"].as_str().unwrap_or("")), 100.0 - used);
    if let Some(r) = w["resets_at"].as_f64().filter(|r| *r > 0.0) { s.push_str(&format!(" · resets {}", resets_in(r as u64, now))) }
    Some(s)
}
fn block(name: String, bound: bool, al: &Value, missing: &str, now: u64) -> LimitBlock {
    let lines: Vec<String> = al["windows"].as_array().into_iter().flatten().filter_map(|w| window_line(w, now)).collect();
    let at = al["at"].as_u64().unwrap_or(0);
    let reason = al["reason"].as_str().unwrap_or("");
    let note = if lines.is_empty() { if reason.is_empty() { missing.to_string() } else { reason.to_string() } } else if at > 0 && now.saturating_sub(at) > 900 { format!("last read {}m ago", now.saturating_sub(at) / 60) } else { String::new() };
    LimitBlock { name, bound, lines, note }
}
/// Every Claude account's limits: the CLI's default login, then each saved account; the one the thread uses is marked.
pub fn limit_blocks(snap: &Value, now: u64) -> Vec<LimitBlock> {
    let (bound, _) = binding(snap);
    let mut out = vec![];
    let d = &snap["default_allowance"];
    let dmiss = if d["state"] == "unavailable" { "The Claude CLI is not signed in with a subscription, so it reports no limits." } else { "Not read yet. The hub checks every 5 minutes." };
    out.push(block("Default account".into(), bound.is_empty(), d, dmiss, now));
    for p in profiles(snap).iter().filter(|p| p["provider"] == "claude") {
        let miss = if p["state"] == "unchecked" { "Checking the sign-in…" } else if p["state"] != "connected" { "Not signed in, so no limits to read. Connect it in Settings." } else { "Claude has not reported limits for this account yet." };
        out.push(block(string(p, "label"), p["id"] == bound.as_str(), &p["allowance"], miss, now));
    }
    out
}
/// Opening the account or limits menu asks the hub to re-read each signed-in (or unchecked) account; the hub reads a profile at most every 5 minutes.
fn refresh_usage(this: &mut Backplane) {
    for p in profiles(&this.screens.acct.snap).iter().filter(|p| matches!(p["state"].as_str(), Some("connected" | "unchecked"))) { ask(this, json!({"op": "status", "profile": string(p, "id")})) }
}
fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
fn limits_menu(cx: &mut Context<Backplane>, light: bool, st: &Accounts) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut list = div().id("limits-menu").w(px(340.)).max_h(px(420.)).overflow_y_scroll().flex().flex_col().py_1().border_1().border_color(c("line")).bg(c("panel")).shadow_lg().occlude()
        .on_mouse_down_out(cx.listener(|this, _, _, cx| { this.screens.acct.limits = false; cx.notify(); }));
    list = list.child(div().px_3().py_1().text_size(px(11.)).text_color(c("faint")).child("Remaining allowance per Claude account"));
    for b in limit_blocks(&st.snap, now_s()) {
        let mut col = div().flex().flex_col().gap(px(2.)).px_3().py(px(6.))
            .child(div().flex().items_center().gap_2().child(div().w(px(14.)).flex_shrink_0().child(if b.bound { crate::art::icon("circle-check", 14., c("accent")) } else { div().into_any_element() }))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(13.)).font_weight(FontWeight::BOLD).child(b.name.clone()))
                .child(div().text_size(px(11.)).text_color(c("faint")).child(if b.bound { "this thread" } else { "" })));
        for l in b.lines { col = col.child(div().pl(px(22.)).text_size(px(12.)).child(l)) }
        if !b.note.is_empty() { col = col.child(div().pl(px(22.)).text_size(px(11.)).text_color(c("dim")).child(b.note)) }
        list = list.child(col);
    }
    if !st.notice.is_empty() { list = list.child(div().px_3().py_1().text_size(px(11.)).text_color(c("bad")).child(st.notice.clone())); }
    list.into_any_element()
}
/// The composer's limits chip: the thread's least remaining window, opening the per-account menu.
pub fn limits_chip(cx: &mut Context<Backplane>, light: bool, st: &Accounts, text: String) -> Div {
    let c = |n: &str| theme::color(light, n);
    let btn = div().id("limits-chip").h(px(30.)).min_w_0().overflow_hidden().px_3().flex().items_center().gap_2().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).whitespace_nowrap().cursor_pointer()
        .hover(|s| s.bg(c("raise"))).child(crate::art::icon("gauge", 14., c("dim"))).child(text)
        .on_click(cx.listener(|this, _, _, cx| { this.screens.acct.limits = !this.screens.acct.limits; if this.screens.acct.limits { list(this); refresh_usage(this) } cx.notify(); }));
    let wrap = div().relative().child(btn);
    if st.limits { wrap.child(deferred(anchored().anchor(Corner::BottomLeft).offset(point(px(0.), px(-4.))).snap_to_window_with_margin(px(8.)).child(limits_menu(cx, light, st))).with_priority(10)) } else { wrap }
}

#[cfg(test)]
mod tests {
    #[test] fn a_bulk_move_says_what_moved_and_what_waits() {
        use serde_json::json;
        assert_eq!(super::moved_text(&json!({"moved":3,"already":1,"running":[],"failed":[]})), "Moved 3 threads to this account.");
        assert_eq!(super::moved_text(&json!({"moved":0,"already":2,"running":[],"failed":[]})), "All 2 threads were already on this account.");
        assert_eq!(super::moved_text(&json!({"moved":1,"already":0,"running":["t2"],"failed":[]})), "Moved 1 thread to this account. 1 thread with a turn running was not moved; press again when it is done.");
        assert_eq!(super::moved_text(&json!({"moved":0,"already":0,"running":["a","b"],"failed":[{"thread":"c"}]})), "Moved 0 threads to this account. 2 threads with a turn running were not moved; press again when they are done. 1 thread could not be moved.");
    }

    use super::*;
    use core::prelude::v1::test; // gpui exports its own `test`
    fn snap() -> Value {
        json!({"profiles":[{"id":"w","provider":"claude","label":"Work","epoch":3,"state":"connected","identity":{"email":"a@b.com","plan":"max"},"allowance":{"windows":[{"scope":"five_hour","used_percent":62.0},{"scope":"seven_day","used_percent":20.0}]}},
            {"id":"d","provider":"claude","label":"Old","epoch":1,"state":"disconnected","identity":{}}],
            "attempts":[{"id":"a1","profile":"d","state":"waiting","url":"https://claude.ai/x","userCode":"","message":""}],
            "thread":{"id":"t1","provider":"claude","binding":"{\"id\":\"w\",\"epoch\":3}"},"defaults":{"claude":"{\"id\":\"w\",\"epoch\":3}"}})
    }
    #[test] fn the_chip_names_the_bound_account_or_says_default() {
        assert_eq!(chip_label(&snap()), "Work"); assert_eq!(chip_label(&Value::Null), "Default");
        let mut s = snap(); s["thread"]["binding"] = json!("{\"id\":\"\",\"epoch\":0}"); assert_eq!(chip_label(&s), "Default");
        s["thread"]["binding"] = json!("{\"id\":\"gone\",\"epoch\":1}"); assert_eq!(chip_label(&s), "Unavailable");
    }
    #[test] fn a_card_says_who_signed_in_and_how_much_is_used() {
        let ps = profiles(&snap()); assert_eq!(identity(&ps[0]), "Claude · a@b.com · max"); assert_eq!(identity(&ps[1]), "Claude");
        assert_eq!(allowance(&ps[0]), "62% of 5-hour used"); assert_eq!(allowance(&ps[1]), "");
        assert!(is_default(&snap(), &ps[0]) && !is_default(&snap(), &ps[1]));
        assert_eq!(badge("connected"), ("Connected", "ok")); assert_eq!(badge("expired"), ("Sign-in expired", "bad")); assert_eq!(badge("whatever").0, "Not connected");
    }
    #[test] fn the_page_asks_again_only_while_something_waits() {
        assert!(wants_poll(&snap())); let mut s = snap(); s["attempts"] = json!([]); assert!(!wants_poll(&s)); s["profiles"][1]["state"] = json!("connecting"); assert!(wants_poll(&s));
        assert_eq!(attempts_of(&snap(), "d").len(), 1); assert!(attempts_of(&snap(), "w").is_empty());
        let mut done = snap(); done["attempts"][0]["state"] = json!("done"); assert!(attempts_of(&done, "d").is_empty() && !wants_poll(&done), "a finished sign-in is not a panel");
        done["attempts"][0]["state"] = json!("failed"); done["attempts"][0]["message"] = json!("That code wasn't accepted."); assert_eq!(failure_of(&done, "d"), "That code wasn't accepted.");
    }
    #[test] fn the_menu_numbers_each_login_says_what_it_has_left_and_marks_the_current_one() {
        let mut s = snap(); s["default_allowance"] = json!({"windows":[{"scope":"five_hour","used_percent":25.0,"resets_at":1000}]}); s["holds"] = json!({});
        let r = menu_rows(&s, 100);
        assert_eq!(r.iter().map(|x| (x.key, x.label.as_str(), x.on, x.usable)).collect::<Vec<_>>(), vec![(Some(1), "Default account", false, true), (Some(2), "Work", true, true), (Some(3), "Old", false, false)]);
        assert_eq!(r[0].left, "5-hour  75% left · resets in 15m"); assert_eq!(r[1].left, "5-hour  38% left", "the fullest window is the one that decides"); assert_eq!(r[1].detail, "a@b.com"); assert_eq!(r[2].left, "", "nothing known is not said"); assert_eq!(r[2].detail, "Not connected");
        s["holds"] = json!({"w":{"until":5000,"text":"out, resets 15:40"}}); let r = menu_rows(&s, 100); assert_eq!((r[1].left.as_str(), r[1].out), ("out, resets 15:40", true), "the hub's word for one that ran out");
    }
    #[test] fn only_the_first_nine_logins_have_a_digit_and_other_providers_are_left_out() {
        let ps: Vec<Value> = (0..11).map(|i| json!({"id":format!("p{i}"),"provider":"claude","label":format!("A{i}"),"epoch":1,"state":"connected","identity":{}})).chain([json!({"id":"c","provider":"codex","label":"C","epoch":1,"state":"connected"})]).collect();
        let r = menu_rows(&json!({"profiles":ps,"thread":{"provider":"claude","binding":""}}), 0); assert_eq!(r.len(), 12, "the default and eleven: codex's is not this thread's"); assert_eq!((r[8].key, r[9].key), (Some(9), None)); assert!(r[0].on, "no binding is the default account");
        let c = menu_rows(&json!({"profiles":[{"id":"c","provider":"codex","label":"C","epoch":1,"state":"connected"}],"thread":{"provider":"codex","binding":"{\"id\":\"c\",\"epoch\":1}"},"default_allowance":{"windows":[{"scope":"five_hour","used_percent":1.0}]}}), 0); assert_eq!(c[0].left, "", "the kept default reading is Claude's"); assert!(c[1].on);
    }
    #[test] fn a_pick_carries_the_binding_the_menu_was_drawn_from() {
        let p = select_params("t1", "w", 3, &snap()); assert_eq!((p["thread"].clone(), p["profile"].clone(), p["epoch"].clone()), (json!("t1"), json!("w"), json!(3))); assert_eq!(p["expected"], "{\"id\":\"w\",\"epoch\":3}");
        assert_eq!(binding(&snap()), ("w".to_string(), 3));
    }
    #[test] fn limits_list_every_account_with_what_is_left_and_why_when_nothing() {
        let mut s = snap(); s["profiles"][0]["allowance"]["windows"][0]["resets_at"] = json!(10_000u64 + 3 * 3600 + 600);
        s["default_allowance"] = json!({"state":"ok","at":9990,"windows":[{"scope":"seven_day","used_percent":86.0,"resets_at":10_000u64 + 2 * 86400}]});
        let b = limit_blocks(&s, 10_000);
        assert_eq!(b.len(), 3);
        assert_eq!((b[0].name.as_str(), b[0].bound, b[0].lines.clone()), ("Default account", false, vec!["Weekly  14% left · resets in 2d 0h".to_string()]));
        assert_eq!((b[1].name.as_str(), b[1].bound), ("Work", true));
        assert_eq!(b[1].lines, vec!["5-hour  38% left · resets in 3h 10m".to_string(), "Weekly  80% left".to_string()]);
        assert!(b[2].lines.is_empty() && b[2].note.contains("Not signed in"));
        s["default_allowance"] = json!({"state":"unavailable","windows":[]}); assert!(limit_blocks(&s, 10_000)[0].note.contains("not signed in"));
        assert!(limit_blocks(&Value::Null, 1)[0].note.contains("Not read yet"));
        s["profiles"][1]["allowance"] = json!({"state":"unavailable","windows":[],"reason":"The Claude CLI did not answer within 30 seconds."}); s["profiles"][1]["state"] = json!("connected");
        assert_eq!(limit_blocks(&s, 10_000)[2].note, "The Claude CLI did not answer within 30 seconds.");
        assert_eq!(resets_in(5, 10), "now"); assert_eq!(resets_in(10 + 720, 10), "in 12m");
    }
}

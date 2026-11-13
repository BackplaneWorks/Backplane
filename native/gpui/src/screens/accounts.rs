//! Accounts: Settings → Accounts (add, sign in, refresh, disconnect, delete, default for new threads) and the Account part of the composer's one menu (screens/combo.rs).
//! The hub keeps the profiles (host `settings/accounts*.rs`); this file asks it with the `accounts` request (the reply's `result` is the snapshot: profiles,
//! sign-in attempts with their link and code, and, when a thread is named, what that thread is bound to) and sends `accounts.select` for a pick.
//! The sign-in code Claude shows is typed in a field here and goes straight to that attempt (`respond`); it is never a draft, a `bfield` or a log line.
use super::composer::{Composer, ComposerEvent};
use super::settings::Secret;
use crate::{hub::Command, i18n::{is_ja, t, tf, trs}, string, theme, Backplane};
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
    /// The Google accounts section under the profiles (screens/google.rs).
    pub google: super::google::Google,
    /// Attempts whose code was sent and not yet answered (the hub checks the sign-in before it says).
    pub checking: std::collections::HashSet<String>,
    /// Which provider the add box makes an account for ("" is Claude).
    pub add_provider: String,
    /// The composer row's width as last drawn (screens/combo.rs picks the chip's words from it; 0 before the first frame).
    pub bar_w: std::rc::Rc<std::cell::Cell<f32>>,
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
/// The words and the colour token of a state.
pub fn badge(state: &str) -> (&'static str, &'static str) {
    /* i18n-ok: drawn through tr */ match state { "connected" => ("Connected", "ok"), "connecting" => ("Signing in", "accent"), "unchecked" => ("Not checked", "warn"), "expired" => ("Sign-in expired", "bad"), _ => ("Not connected", "faint") }
}
/// The state a card shows: the hub's "limited until 15:40" while the login is held after running out, else its sign-in state.
pub fn state_word(snap: &Value, id: &str, state: &str) -> (String, &'static str) {
    if state == "connected" || id.is_empty() { if let Some(t) = snap["holds"][id]["text"].as_str() { return (trs(t), "warn") } }
    let (w, tok) = badge(state); (trs(w), tok)
}
/// Whether the hub is waiting for the person or the provider (so the page asks again every couple of seconds).
pub fn wants_poll(snap: &Value) -> bool {
    snap["attempts"].as_array().map_or(false, |a| a.iter().any(is_live))
        || profiles(snap).iter().any(|p| p["state"] == "connecting")
}
/// The identity line of a profile: "Claude · a@b.com · max".
pub fn identity(p: &Value) -> String {
    /* i18n-ok: product names */ let mut parts = vec![if p["provider"] == "codex" { "Codex".to_string() } else { "Claude".to_string() }];
    for k in ["email", "plan"] { if let Some(s) = p["identity"][k].as_str().filter(|s| !s.is_empty()) { parts.push(s.to_string()) } }
    if is_sync(p) { parts.push(sync_line(p, now_s())) }
    parts.join(" · ")
}
/// A sync account: Claude Code's long-lived token, shared with the person's linked machines (the hub's accounts_sync.rs).
pub fn is_sync(p: &Value) -> bool { p["kind"] == "sync" }
/// What a sync account's card says of its token: where it came from and how long it has left.
pub fn sync_line(p: &Value, now: u64) -> String {
    let ends = p["tokenExpiresAt"].as_u64().unwrap_or(0);
    let from = if p["origin"].as_str().map_or(true, str::is_empty) { trs("sync token") } else { trs("sync token from a linked machine") };
    if ends == 0 { return from }
    /* i18n-ok: joins two catalog words */ if ends <= now { return format!("{from}, {}", trs("ended")) }
    /* i18n-ok: joins two catalog words */ format!("{from}, {}", tf("gpui.acct.sync.days", &[("n", ((ends - now) / 86400).into())]))
}
/// The most used window of a profile's allowance as "62% of 5-hour used", when the provider reported one.
pub fn allowance(p: &Value) -> String {
    let w = p["allowance"]["windows"].as_array().into_iter().flatten().filter_map(|w| Some((w["used_percent"].as_f64()?, w["scope"].as_str().unwrap_or("").to_string()))).fold(None::<(f64, String)>, |a, b| match a { Some(x) if x.0 >= b.0 => Some(x), _ => Some(b) });
    match w { Some((u, scope)) => tf("gpui.acct.used", &[("pct", format!("{u:.0}").into()), ("scope", trs(&scope.replace('_', "-").replace("five-hour", "5-hour").replace("seven-day", "7-day")).into())]), None => String::new() }
}
/// One line of the account menu: the key that picks it, what it is, what it has left, and where a press goes.
#[derive(Debug, PartialEq, Clone)]
pub struct MenuRow { pub key: Option<u8>, pub label: String, pub detail: String, pub left: String, pub out: bool, pub on: bool, pub usable: bool, pub profile: String, pub epoch: u64 }
/// What one login has left, in a few words: the hub's word when it is out, else its fullest window, else nothing known.
fn left_of(holds: &Value, id: &str, al: &Value, now: u64) -> (String, bool) {
    if let Some(t) = holds[id]["text"].as_str() { return (trs(t), true) }
    let fullest = al["windows"].as_array().into_iter().flatten().filter(|w| w["used_percent"].as_f64().or_else(|| w["used_permille"].as_f64().map(|p| p / 10.0)).is_some())
        .max_by(|a, b| { let u = |w: &Value| w["used_percent"].as_f64().or_else(|| w["used_permille"].as_f64().map(|p| p / 10.0)).unwrap_or(0.0); u(a).total_cmp(&u(b)) });
    (fullest.and_then(|w| window_line(w, now)).unwrap_or_default(), false)
}
/// Who the provider's CLI login is, as the hub found it (`default_login`): its email, else why it cannot say.
pub fn default_who(snap: &Value, provider: &str) -> (Option<String>, String) {
    let d = &snap["default_login"][provider];
    match d["state"].as_str() {
        Some("known") => (d["email"].as_str().filter(|e| !e.is_empty()).map(str::to_string), String::new()),
        Some("signed-out") => (None, trs("signed out")),
        Some(_) => (None, d["reason"].as_str().map(trs).unwrap_or_else(|| trs("can't tell who is signed in"))),
        None => (None, trs("not checked yet")),
    }
}
/// The added accounts that are the provider's CLI login under another name (the hub marks them `sameAsDefault`): drawn as that login, never as a second account.
pub fn twins(snap: &Value, provider: &str) -> Vec<Value> { profiles(snap).into_iter().filter(|p| p["provider"] == provider && p["sameAsDefault"] == true).collect() }
/// The menu's rows: the thread's provider's logins, the CLI's own first, each of the first nine with its digit. The hub's snapshot decides all of it.
pub fn menu_rows(snap: &Value, now: u64) -> Vec<MenuRow> {
    let (bound, _) = binding(snap);
    let provider = snap["thread"]["provider"].as_str().filter(|p| !p.is_empty()).unwrap_or("claude");
    let holds = &snap["holds"];
    // the CLI's own login and any added account of the same identity are one row: its usage and holds are theirs together, and a thread on either is on it
    let twins = twins(snap, provider);
    let own_al = if provider == "claude" { snap["default_allowance"].clone() } else { Value::Null };
    let al = if own_al["windows"].as_array().map_or(true, |w| w.is_empty()) { twins.first().map(|t| t["allowance"].clone()).unwrap_or(own_al) } else { own_al };
    let (mut left, mut out) = left_of(holds, "", &al, now);
    if !out { if let Some(t) = twins.iter().find(|t| holds[string(t, "id")]["text"].is_string()) { (left, out) = left_of(holds, &string(t, "id"), &Value::Null, now) } }
    let (who, why) = default_who(snap, provider);
    let (label, detail) = match who { Some(e) => (format!("{} · {e}", trs("Default account")), trs("CLI login")), /* i18n-ok: joins catalog words */ None => (trs("Default account"), format!("{} · {why}", trs("CLI login"))) };
    let on = bound.is_empty() || twins.iter().any(|t| string(t, "id") == bound);
    let mut rows = vec![MenuRow { key: None, label, detail, left, out, on, usable: true, profile: String::new(), epoch: 0 }];
    for p in profiles(snap).iter().filter(|p| p["provider"] == provider && p["sameAsDefault"] != true) {
        let (id, ok) = (string(p, "id"), p["state"] == "connected");
        let (left, out) = left_of(holds, &id, &p["allowance"], now);
        let detail = if ok { p["identity"]["email"].as_str().unwrap_or("").to_string() } else { trs(badge(p["state"].as_str().unwrap_or("")).0) };
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
    /* i18n-ok: drawn through tr */ if this.tx.try_send(Command::Rpc("accounts".into(), Secret(params), "acct")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); }
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
        input.update(cx, |i, _| { i.set_placeholder(&trs("Paste the code from Claude's page")); i.masked = true; });
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
        /* i18n-ok: drawn through tr */ if !why.is_empty() { this.screens.acct.notice = why.to_string() } else if this.screens.acct.notice.is_empty() { this.screens.acct.notice = "The hub refused that.".into() }
        return
    }
    if tag == "acct-moveall" { list(this); return }
    // a pick shuts the menu it was made in (screens/combo.rs)
    if tag == "acct-select" { if this.screens.acct.menu || this.screens.picker_open { this.want_focus = true } this.screens.acct.menu = false; this.screens.picker_open = false; this.screens.acct.notice.clear(); list(this); }
    if tag == "acct-default" { this.screens.acct.notice.clear(); list(this); }
}
/// The answer to "Move all threads here": how many moved, and the running ones it left (a turn is never switched under itself; they can be moved when it ends).
pub fn moved(this: &mut Backplane, v: &Value) { this.screens.acct.info = moved_text(v); }
pub fn moved_text(v: &Value) -> String {
    let n = |k: &str| v[k].as_u64().unwrap_or(0); let len = |k: &str| v[k].as_array().map_or(0, |a| a.len());
    let (moved, already, running, failed) = (n("moved"), n("already"), len("running"), len("failed"));
    let mut parts = vec![if moved == 0 && already > 0 { tf("gpui.acct.moved.already", &[("n", already.into())]) } else { tf("gpui.acct.moved", &[("n", moved.into())]) }];
    if running > 0 { parts.push(tf("gpui.acct.moved.running", &[("n", running.into())])) }
    if failed > 0 { parts.push(tf("gpui.acct.moved.failed", &[("n", failed.into())])) }
    // Japanese sentences follow one another with no space
    parts.join(if is_ja() { "" } else { " " })
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
        input.update(cx, |i, _| i.set_placeholder(&trs("Account label (e.g. Work)")));
        cx.subscribe(&input, |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { add(this, cx) }).detach();
        this.screens.acct.label = Some(input);
    }
    this.screens.acct.armed.clear();
    list(this);
    super::google::open_page(this);
}
fn add(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.acct.label.clone() else { return };
    let label = input.read(cx).content.to_string();
    /* i18n-ok: drawn through tr */ if label.trim().is_empty() { this.screens.acct.notice = "Name the account first, e.g. Work or Personal.".into(); return }
    let provider = if this.screens.acct.add_provider == "codex" { "codex" } else { "claude" };
    ask(this, json!({"op": "add", "provider": provider, "label": label.trim()}));
    input.update(cx, |i, cx| i.set(String::new(), cx));
}
/// Add account → Claude (sync): a sync account named in the box, its `claude setup-token` started at once on the hub.
fn add_sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.acct.label.clone() else { return };
    let label = input.read(cx).content.to_string();
    /* i18n-ok: drawn through tr */ if label.trim().is_empty() { this.screens.acct.notice = "Name the account first, e.g. Work or Personal.".into(); return }
    ask(this, json!({"op": "add", "provider": "claude", "kind": "sync", "label": label.trim(), "start": true}));
    input.update(cx, |i, cx| i.set(String::new(), cx));
}
fn respond(this: &mut Backplane, attempt: &str, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.acct.codes.get(attempt).cloned() else { return };
    let profile = this.screens.acct.snap["attempts"].as_array().into_iter().flatten().find(|a| a["id"] == attempt).map(|a| string(a, "profile")).unwrap_or_default();
    let code = input.read(cx).content.to_string();
    /* i18n-ok: drawn through tr */ if code.trim().is_empty() { this.screens.acct.notice = "Paste the code Claude shows after you sign in.".into(); return }
    this.screens.acct.checking.insert(attempt.to_string()); this.screens.acct.notice.clear();
    ask(this, json!({"op": "respond", "profile": profile, "attempt": attempt, "code": code.trim()}));
}

// ---- the page --------------------------------------------------------------------------------------------------------------------------------------

pub(super) fn button(cx: &mut Context<Backplane>, light: bool, id: String, label: &str, primary: bool, danger: bool, on: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).h(px(28.)).px_3().flex().items_center().justify_center().text_size(px(12.)).cursor_pointer();
    let d = if primary { d.bg(c("accent")).text_color(c("on-accent")) } else if danger { d.bg(c("raise")).text_color(c("bad")).hover(|s| s.bg(c("line"))) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
    d.child(crate::i18n::trs(label)).on_click(cx.listener(move |this, _, _, cx| { on(this, cx); cx.notify(); })).into_any_element()
}

fn attempt_panel(cx: &mut Context<Backplane>, light: bool, st: &Accounts, a: &Value, pid: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (aid, url, code, msg) = (string(a, "id"), string(a, "url"), string(a, "userCode"), string(a, "message"));
    let mut col = div().flex().flex_col().gap_2().mt_3().p_3().bg(c("panel")).border_1().border_color(c("line"));
    let ready = !url.is_empty();
    let help = if !ready { trs("Generating the sign-in link…") } else if code.is_empty() { trs("Open the link, sign in to Claude, then paste the code it shows.") } else { trs("Open the link and enter this code.") };
    col = col.child(div().text_size(px(12.)).text_color(c(if ready { "dim" } else { "accent" })).child(help));
    if !code.is_empty() { col = col.child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child(code.clone())); }
    if !url.is_empty() {
        let (u1, u2) = (url.clone(), url.clone());
        col = col.child(div().flex().items_center().gap_2()
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(11.)).text_color(c("faint")).child(url))
            .child(button(cx, light, format!("acct-open-{aid}"), &trs("Open"), true, false, move |_, cx| cx.open_url(&u1)))
            .child(button(cx, light, format!("acct-copy-{aid}"), &trs("Copy link"), false, false, move |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(u2.clone())))));
    }
    if let Some(input) = st.codes.get(&aid).filter(|_| code.is_empty() && ready) {
        let (a1, a2, p2) = (aid.clone(), aid.clone(), pid.to_string());
        let _ = (&a2, &p2);
        col = col.child(div().flex().items_center().gap_2()
            .child(div().flex_1().h(px(28.)).px_2().flex().items_center().bg(c("raise")).child(input.clone()))
            .child(button(cx, light, format!("acct-code-{aid}"), &trs("Submit code"), true, false, move |this, cx| respond(this, &a1, cx))));
    }
    if st.checking.contains(&aid) { col = col.child(div().text_size(px(12.)).text_color(c("accent")).child(trs("Checking the code and your sign-in…"))); }
    if !msg.is_empty() { col = col.child(div().text_size(px(12.)).text_color(c("bad")).child(trs(&msg))); }
    let (aid2, pid2) = (aid.clone(), pid.to_string());
    col.child(div().flex().child(button(cx, light, format!("acct-cancel-{aid}"), &trs("Cancel sign-in"), false, false, move |this, _| ask(this, json!({"op": "cancel", "profile": pid2, "attempt": aid2}))))).into_any_element()
}

/// `narrow`: the state goes under the name instead of beside it (a page under ADD_ROW_MIN wide).
fn card(cx: &mut Context<Backplane>, light: bool, st: &Accounts, p: &Value, i: usize, narrow: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (id, state) = (string(p, "id"), string(p, "state"));
    let (word, tok) = state_word(&st.snap, &id, &state);
    let epoch = p["epoch"].as_u64().unwrap_or(0);
    let default = is_default(&st.snap, p);
    let mut sub = vec![identity(p)];
    let al = allowance(p); if !al.is_empty() { sub.push(al) }
    let head = if narrow { div().flex().flex_col().items_start() } else { div().flex().items_center() }.gap_3()
        .child(div().flex().flex_col().gap(px(2.)).flex_1().min_w(px(140.)).max_w_full()
            .child(div().flex().items_center().gap_2().child(super::picker::logo(if p["provider"] == "codex" { "codex" } else { "claude" })).child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).child(string(p, "label")))
                .when(default, |d| d.child(div().px_2().py(px(1.)).bg(c("raise")).text_size(px(11.)).text_color(c("dim")).child(trs("New threads")))))
            .child(div().text_size(px(12.)).text_color(c("dim")).child(sub.join("  ·  "))))
        .child(div().flex().items_center().gap_2().flex_shrink_0().child(div().size(px(8.)).bg(c(tok))).child(div().text_size(px(12.)).text_color(c(tok)).child(word)));
    let mut row = div().flex().items_center().gap_2().mt_3().flex_wrap();
    let armed = st.armed == id;
    let mut btn = |label: &str, key: &str, primary: bool, danger: bool, on: Box<dyn Fn(&mut Backplane, &mut Context<Backplane>)>| { row = std::mem::replace(&mut row, div()).child(button(cx, light, format!("acct-{key}-{i}"), label, primary, danger, on)); };
    let live = !attempts_of(&st.snap, &id).is_empty() || state == "connecting";
    let pid = id.clone();
    if !live {
        if state == "connected" {
            btn(&trs("Check now"), "status", false, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "status", "profile": pid, "asked": true})) }));
            /* i18n-ok: drawn through tr */ if !default { let (prov, pid2) = (string(p, "provider"), pid.clone()); btn(&trs("Use for new threads"), "default", false, false, Box::new(move |this, _| { if this.tx.try_send(Command::Rpc("accounts.default".into(), Secret(json!({"provider": prov, "profile": pid2, "epoch": epoch})), "acct-default")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); } })); }
            /* i18n-ok: drawn through tr */ { let (prov, pid2) = (string(p, "provider"), pid.clone()); btn(&trs("Move all threads here"), "moveall", false, false, Box::new(move |this, _| { this.screens.acct.notice.clear(); this.screens.acct.info.clear(); if this.tx.try_send(Command::Rpc("accounts.move_all".into(), Secret(json!({"provider": prov, "profile": pid2, "epoch": epoch, "default": true})), "acct-moveall")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); } })); }
            if is_sync(p) { btn(&trs("Renew the sync token"), "renew", false, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "login", "profile": pid})) })); }
            else if p["provider"] == "claude" { btn(&trs("Make a sync token"), "synctoken", false, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "synctoken", "profile": pid})) })); }
            btn(&trs("Sign out"), "logout", false, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "disconnect", "profile": pid})) }));
        } else if is_sync(p) && state != "unchecked" {
            btn(&if state == "expired" { trs("Renew the sync token") } else { trs("Make the sync token") }, "login", true, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "login", "profile": pid})) }));
        } else {
            if state == "expired" { btn(&trs("Check now"), "status", false, false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": "status", "profile": pid, "asked": true})) })); }
            btn(&if state == "unchecked" { trs("Check sign-in") } else if state == "expired" { trs("Sign in again") } else { trs("Connect") }, "login", state != "unchecked", false, Box::new({ let pid = pid.clone(); move |this, _| ask(this, json!({"op": if state_of(&this.screens.acct.snap, &pid) == "unchecked" { "status" } else { "login" }, "profile": pid})) }));
        }
    }
    // Delete is always there: it is the way out of a sign-in that is stuck (the hub ends the attempt first)
    if armed {
        btn(&trs("Confirm delete"), "remove", false, true, Box::new({ let pid = pid.clone(); move |this, _| { this.screens.acct.armed.clear(); ask(this, json!({"op": "remove", "profile": pid})) } }));
        btn(&trs("Keep"), "keep", false, false, Box::new(|this, _| this.screens.acct.armed.clear()));
    } else {
        btn(&trs("Delete"), "delete", false, true, Box::new({ let pid = pid.clone(); move |this, _| this.screens.acct.armed = pid.clone() }));
    }
    let mut card = div().flex().flex_col().p_4().mb_3().border_1().border_color(c("line")).bg(c("view")).child(head).child(row);
    for a in attempts_of(&st.snap, &id) { card = card.child(attempt_panel(cx, light, st, &a, &id)); }
    let why = failure_of(&st.snap, &id); if !why.is_empty() && attempts_of(&st.snap, &id).is_empty() { card = card.child(div().mt_2().text_size(px(12.)).text_color(c("bad")).child(trs(&why))); }
    card.into_any_element()
}
fn state_of(snap: &Value, id: &str) -> String { profiles(snap).iter().find(|p| p["id"] == id).map(|p| string(p, "state")).unwrap_or_default() }

/// What the CLI login's card says under its name: "Claude · a@b.com · max · this computer's own Claude Code sign-in", or why it cannot say who.
pub fn default_line(snap: &Value, provider: &str) -> String {
    /* i18n-ok: product names */ let name = if provider == "codex" { "Codex" } else { "Claude" };
    let d = &snap["default_login"][provider];
    match default_who(snap, provider) {
        (Some(e), _) => { let mut parts = vec![name.to_string(), e]; if let Some(p) = d["plan"].as_str().filter(|p| !p.is_empty()) { parts.push(p.to_string()) } parts.push(trs("CLI login")); parts.join(" · ") }
        /* i18n-ok: joins catalog words */ (None, why) => format!("{name} · {} · {why}", trs("CLI login")),
    }
}
/// The added accounts folded into the CLI login's card: "Also added as Work: the same account, counted once".
pub fn twin_line(snap: &Value, provider: &str) -> Option<String> {
    let names: Vec<String> = twins(snap, provider).iter().map(|t| string(t, "label")).collect();
    if names.is_empty() { None } else { Some(format!("{} {}: {}", trs("Also added as"), names.join(", "), trs("the same account, counted once"))) }
}
fn default_card(cx: &mut Context<Backplane>, light: bool, st: &Accounts, provider: &str, narrow: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let def_default = !profiles(&st.snap).iter().any(|p| p["provider"] == provider && is_default(&st.snap, p));
    let twins = twins(&st.snap, provider);
    // a hold on the login or on an added account of the same identity: one account, out once
    let held = std::iter::once(String::new()).chain(twins.iter().map(|t| string(t, "id"))).find_map(|id| st.snap["holds"][&id]["text"].as_str().map(str::to_string));
    let card_row = if narrow { div().flex().flex_col().items_start() } else { div().flex().items_center() };
    let mut words = div().flex().flex_col().gap(px(2.)).flex_1().min_w(px(160.)).max_w_full().child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).child(trs("Default account")))
        .child(div().text_size(px(12.)).text_color(c("dim")).child(default_line(&st.snap, provider)));
    if let Some(t) = twin_line(&st.snap, provider) { words = words.child(div().text_size(px(12.)).text_color(c("dim")).child(t)); }
    let mut row = card_row.gap_3().p_4().mb_3().border_1().border_color(c("line")).bg(c("view")).child(words)
        .child({ let (word, tok) = match held { Some(t) => (trs(&t), "warn"), None => (if def_default { trs("Used for new threads") } else { trs("Available") }, "ok") };
            div().flex().flex_shrink_0().items_center().gap_2().child(div().size(px(8.)).bg(c(tok))).child(div().text_size(px(12.)).text_color(c(tok)).child(word)) });
    // the added copy can still be deleted from here (threads on it go on on the CLI login's identity once it is gone)
    for t in &twins {
        let pid = string(t, "id");
        row = row.child(if st.armed == pid {
            div().flex().gap_2().child(button(cx, light, format!("acct-twin-rm-{pid}"), &format!("{} {}", trs("Confirm delete"), string(t, "label")), false, true, { let pid = pid.clone(); move |this, _| { this.screens.acct.armed.clear(); ask(this, json!({"op": "remove", "profile": pid})) } }))
                .child(button(cx, light, format!("acct-twin-keep-{pid}"), &trs("Keep"), false, false, |this, _| this.screens.acct.armed.clear())).into_any_element()
        } else { button(cx, light, format!("acct-twin-del-{pid}"), &format!("{} {}", trs("Delete"), string(t, "label")), false, true, { let pid = pid.clone(); move |this, _| this.screens.acct.armed = pid.clone() }) });
    }
    row.into_any_element()
}

/// The page: the profiles as cards, then the add box.
/// The width under which the label box and Add go one under the other (settings::Fit::rows).
pub const ADD_ROW_MIN: f32 = 380.0;
/// `avail`: the width the page's rows are laid out in.
pub fn page(cx: &mut Context<Backplane>, light: bool, st: &Accounts, avail: f32) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let mut out: Vec<AnyElement> = vec![];
    out.push(div().pb_3().text_size(px(12.)).text_color(c("dim")).child(trs("Each account is a separate Claude subscription sign-in kept on this machine. A thread uses the account picked beside its model; when one runs out of allowance the thread carries on with the next account that has some left.")).into_any_element());
    if !st.notice.is_empty() { out.push(div().mb_3().p_3().bg(c("raise")).text_size(px(12.)).text_color(c("bad")).child(trs(&st.notice)).into_any_element()); }
    if !st.info.is_empty() { out.push(div().mb_3().p_3().bg(c("raise")).text_size(px(12.)).text_color(c("text")).child(st.info.clone()).into_any_element()); }
    // the CLI's own login is always there, for each provider whose CLI said anything (Claude always): who it is, and an added account of the same identity folded in
    for provider in ["claude", "codex"] {
        if provider == "codex" && st.snap["default_login"]["codex"].is_null() { continue }
        out.push(default_card(cx, light, st, provider, avail < ADD_ROW_MIN));
    }
    let ps = profiles(&st.snap);
    for (i, p) in ps.iter().enumerate().filter(|(_, p)| p["sameAsDefault"] != true) { out.push(card(cx, light, st, p, i, avail < ADD_ROW_MIN)); }
    if let Some(input) = &st.label {
        let row = if avail < ADD_ROW_MIN { div().flex().flex_col().items_start().gap_2().mt_2().child(div().w_full().h(px(32.)).px_3().flex().items_center().bg(c("raise")).child(input.clone())) }
            else { div().flex().items_center().gap_2().mt_2().child(div().flex_1().min_w_0().h(px(32.)).px_3().flex().items_center().bg(c("raise")).child(input.clone())) };
        out.push(row
            .child(button(cx, light, "acct-add".into(), &trs("Add Claude account"), true, false, |this, cx| add(this, cx)))
            .child(button(cx, light, "acct-add-sync".into(), &trs("Add Claude (sync)"), false, false, |this, cx| add_sync(this, cx))).into_any_element());
    }
    out.extend(super::google::section(cx, light, &st.google));
    out
}

/// The same sign-in rows for one provider, inside a setup step.
pub fn panel(cx: &mut Context<Backplane>, light: bool, st: &Accounts, provider: &str) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let mut out: Vec<AnyElement> = vec![];
    if !st.notice.is_empty() { out.push(div().p_3().bg(c("raise")).text_size(px(12.)).text_color(c("bad")).child(trs(&st.notice)).into_any_element()); }
    for (i, p) in profiles(&st.snap).iter().enumerate().filter(|(_, p)| p["provider"] == provider) { out.push(card(cx, light, st, p, i, false)); }
    if let Some(input) = &st.label {
        let word = if provider == "codex" { trs("Add Codex account") } else { trs("Add Claude account") };
        out.push(div().flex().items_center().gap_2().child(div().flex_1().h(px(32.)).px_3().flex().items_center().bg(c("raise")).child(input.clone()))
            .child(button(cx, light, "acct-add".into(), &word, true, false, |this, cx| add(this, cx))).into_any_element());
    }
    out
}

// ---- the chip's menu -------------------------------------------------------------------------------------------------------------------------------

/// The account key: the one menu with the keyboard on its accounts (screens/combo.rs).
pub fn toggle(this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) { this.combo_account(window, cx) }
/// A row pressed, by the mouse or its digit: a usable account is asked for; one that is not signed in goes to where it is signed in.
fn press(this: &mut Backplane, row: &MenuRow, cx: &mut Context<Backplane>) {
    if row.usable {
        let thread = current_thread(this);
        let p = select_params(&thread, &row.profile, row.epoch, &this.screens.acct.snap);
        /* i18n-ok: drawn through tr */ if this.tx.try_send(Command::Rpc("accounts.select".into(), Secret(p), "acct-select")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); }
    } else { this.screens.acct.menu = false; this.screens.picker_open = false; this.open_settings(cx); this.screens.settings_ui.device = false; this.screens.settings_ui.accounts = true; open_page(this, cx); }
    cx.notify();
}
/// The digit `n` in the open menu.
pub fn press_nth(this: &mut Backplane, n: u8, cx: &mut Context<Backplane>) {
    if !this.screens.acct.menu || !this.screens.picker_open { return }
    if let Some(r) = menu_rows(&this.screens.acct.snap, now_s()).into_iter().find(|r| r.key == Some(n)) { press(this, &r, cx) }
}

/// The Account part of the one menu: the default account and each saved account of the thread's provider, each with its digit and what it has left, the bound one
/// ticked; one that cannot be used says why. It has the keyboard (context `AccountMenu`, its digits) when the account key opened the menu.
pub fn section(cx: &mut Context<Backplane>, light: bool, st: &Accounts, s: f32) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let mut list = div().id("acct-menu").key_context("AccountMenu").max_h(px(300. * s)).overflow_y_scroll().flex().flex_col().pb_1();
    if let Some(h) = &st.focus { list = list.track_focus(h); }
    for (i, r) in menu_rows(&st.snap, now_s()).into_iter().enumerate() {
        let row = r.clone();
        let mut text = div().flex_1().min_w_0().flex().flex_col().gap(px(1.))
            .child(div().flex().items_center().gap_2()
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(px(13. * s)).text_color(c(if r.usable { "text" } else { "faint" })).child(r.label))
                .child(div().flex_shrink_0().max_w(px(150. * s)).overflow_hidden().whitespace_nowrap().text_size(px(11. * s)).text_color(c("faint")).child(r.detail)));
        if !r.left.is_empty() { text = text.child(div().overflow_hidden().whitespace_nowrap().text_size(px(11. * s)).text_color(c(if r.out { "warn" } else { "dim" })).child(r.left)); }
        list = list.child(div().id(SharedString::from(format!("acct-m-{i}"))).flex().items_center().gap_2().px_3().py(px(5.)).mx_1().cursor_pointer().hover(|s| s.bg(c("raise")))
            .child(div().w(px(16.)).flex_shrink_0().text_size(px(11. * s)).text_color(c(if st.menu { "accent" } else { "dim" })).child(r.key.map_or(String::new(), |k| k.to_string())))
            .child(div().w(px(14.)).flex_shrink_0().child(if r.on { crate::art::icon("circle-check", 14., c("accent")) } else { div().into_any_element() }))
            .child(text)
            .on_click(cx.listener(move |this, _, _, cx| press(this, &row, cx))));
    }
    if !st.notice.is_empty() { list = list.child(div().px_3().py_1().text_size(px(11. * s)).text_color(c("bad")).child(trs(&st.notice))); }
    if !st.info.is_empty() { list = list.child(div().px_3().py_1().text_size(px(11. * s)).text_color(c("text")).child(st.info.clone())); }
    // the bound account's "Move all threads here", as on its card in Settings
    let (bound, epoch) = binding(&st.snap);
    if let Some(p) = profiles(&st.snap).into_iter().find(|p| p["id"] == bound.as_str() && p["state"] == "connected") {
        let prov = string(&p, "provider");
        list = list.child(div().id("acct-m-moveall").flex().items_center().gap_2().px_3().py(px(6.)).mx_1().cursor_pointer().hover(|s| s.bg(c("raise"))).text_size(px(13. * s)).text_color(c("dim"))
            /* i18n-ok: drawn through tr */ .child(trs("Move all threads here")).on_click(cx.listener(move |this, _, _, cx| { this.screens.acct.notice.clear(); this.screens.acct.info.clear(); if this.tx.try_send(Command::Rpc("accounts.move_all".into(), Secret(json!({"provider": prov, "profile": bound, "epoch": epoch, "default": true})), "acct-moveall")).is_err() { this.screens.acct.notice = "The hub is not reachable. Try again.".into(); } cx.notify(); })));
    }
    list.child(div().id("acct-m-manage").flex().items_center().gap_2().px_3().py(px(6.)).mx_1().cursor_pointer().hover(|s| s.bg(c("raise"))).text_size(px(13. * s)).text_color(c("dim"))
        .child(trs("Manage accounts in Settings")).on_click(cx.listener(|this, _, _, cx| { this.screens.acct.menu = false; this.screens.picker_open = false; this.want_focus = true; this.open_settings(cx); this.screens.settings_ui.device = false; this.screens.settings_ui.accounts = true; open_page(this, cx); cx.notify(); })))
}

// ---- what an account has left ----------------------------------------------------------------------------------------------------------------

fn window_name(w: &Value) -> String {
    if let Some(l) = w["label"].as_str().filter(|l| !l.is_empty()) { return l.to_string() }
    match w["scope"].as_str().unwrap_or("") { "five_hour" => trs("5-hour"), "seven_day" => trs("Weekly"), "seven_day_opus" => trs("Weekly Opus"), "seven_day_sonnet" => trs("Weekly Sonnet"), o => o.replace('_', " ") }
}
/// "in 2d 3h" / "in 4h 5m" / "in 12m" / "now" for a reset time in epoch seconds.
pub fn resets_in(at: u64, now: u64) -> String {
    if at <= now { return t("gpui.acct.now") }
    let m = (at - now + 59) / 60;
    if m >= 1440 { tf("gpui.acct.in.days", &[("d", (m / 1440).into()), ("h", ((m % 1440) / 60).into())]) } else if m >= 60 { tf("gpui.acct.in.hours", &[("h", (m / 60).into()), ("m", (m % 60).into())]) } else { tf("gpui.acct.in.minutes", &[("m", m.into())]) }
}
/// A window as "5-hour  72% left · resets in 3h 10m".
pub fn window_line(w: &Value, now: u64) -> Option<String> {
    let used = w["used_percent"].as_f64().or_else(|| w["used_permille"].as_f64().map(|p| p / 10.0)).filter(|u| (0.0..=100.0).contains(u))?;
    let mut s = tf("gpui.acct.left", &[("window", window_name(w).into()), ("pct", format!("{:.0}", 100.0 - used).into())]);
    if let Some(r) = w["resets_at"].as_f64().filter(|r| *r > 0.0) { s.push_str(&if r as u64 <= now { t("gpui.acct.resets.now") } else { tf("gpui.acct.resets", &[("when", resets_in(r as u64, now).into())]) }) }
    Some(s)
}
/// Opening the menu asks the hub to re-read each signed-in (or unchecked) account; the hub reads a profile at most every 5 minutes.
pub fn refresh_usage(this: &mut Backplane) {
    for p in profiles(&this.screens.acct.snap).iter().filter(|p| matches!(p["state"].as_str(), Some("connected" | "unchecked"))) { ask(this, json!({"op": "status", "profile": string(p, "id")})) }
}
fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

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
    #[test] fn a_sync_card_says_where_its_token_came_from_and_how_long_it_has_left() {
        let p = json!({"kind":"sync","origin":"","tokenExpiresAt":1000 + 40 * 86400}); assert!(is_sync(&p) && !is_sync(&json!({"kind":""})));
        assert_eq!(sync_line(&p, 1000), "sync token, 40 days left");
        assert_eq!(sync_line(&json!({"kind":"sync","origin":"L1","tokenExpiresAt":10}), 1000), "sync token from a linked machine, ended");
        assert_eq!(sync_line(&json!({"kind":"sync"}), 1000), "sync token");
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
        s["holds"] = json!({"w":{"until":5000,"text":"limited until 15:40"}}); let r = menu_rows(&s, 100); assert_eq!((r[1].left.as_str(), r[1].out), ("limited until 15:40", true), "the hub's word for one that ran out");
        assert_eq!(state_word(&s, "w", "connected"), ("limited until 15:40".to_string(), "warn"), "Settings marks a held account limited until its reset");
        assert_eq!(state_word(&s, "w", "expired").1, "bad", "a dead sign-in says so, not the hold"); assert_eq!(state_word(&s, "p", "connected").1, "ok");
    }
    /// Nov 13: the CLI's own login and an added account were the same helmut@backplaneworks.com, and the menu listed them as 1 and 4.
    #[test] fn the_cli_login_says_who_it_is_and_an_account_added_again_is_one_row() {
        let mut s = snap(); s["holds"] = json!({});
        s["default_login"] = json!({"claude":{"state":"known","email":"a@b.com","plan":"max"},"codex":{"state":"unknown","reason":"Codex is signed in with an API key: it does not say whose"}});
        s["profiles"][0]["sameAsDefault"] = json!(true); // Work is the CLI's own login, added again; the thread is on it
        let r = menu_rows(&s, 100);
        assert_eq!(r.iter().map(|x| (x.label.as_str(), x.detail.as_str(), x.on)).collect::<Vec<_>>(), vec![("Default account · a@b.com", "CLI login", true), ("Old", "Not connected", false)], "one row, ticked for a thread on either name");
        assert_eq!(r[0].left, "5-hour  38% left", "its usage is the copy's when the login has no reading of its own");
        s["holds"] = json!({"w":{"until":5000,"text":"limited until 15:40"}}); let r = menu_rows(&s, 100); assert_eq!((r[0].left.as_str(), r[0].out), ("limited until 15:40", true), "out through its copy");
        assert_eq!(default_line(&s, "claude"), "Claude · a@b.com · max · CLI login"); assert_eq!(twin_line(&s, "claude").as_deref(), Some("Also added as Work: the same account, counted once"));
        // a CLI that cannot say who: said plainly, and nothing is folded into it
        assert_eq!(default_line(&s, "codex"), "Codex · CLI login · Codex is signed in with an API key: it does not say whose"); assert_eq!(twin_line(&s, "codex"), None);
        s["thread"]["provider"] = json!("codex"); assert_eq!((menu_rows(&s, 100)[0].label.as_str(), menu_rows(&s, 100)[0].detail.as_str()), ("Default account", "CLI login · Codex is signed in with an API key: it does not say whose"));
        s["default_login"]["codex"] = json!({"state":"signed-out"}); assert_eq!(default_line(&s, "codex"), "Codex · CLI login · signed out");
        assert_eq!(default_line(&snap(), "claude"), "Claude · CLI login · not checked yet");
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
    #[test] fn the_move_summary_and_what_is_left_read_in_japanese() {
        crate::i18n::with_lang("ja", || {
            assert_eq!(moved_text(&json!({"moved":1,"already":0,"running":["t2"],"failed":[{"thread":"c"}]})), "1件のスレッドをこのアカウントに移しました。ターン実行中の1件のスレッドは移していません。終わったらもう一度押してください。1件のスレッドを移せませんでした。");
            assert_eq!(moved_text(&json!({"moved":0,"already":2,"running":[],"failed":[]})), "2件のスレッドはすべて、すでにこのアカウントでした。");
            assert_eq!(window_line(&json!({"scope":"seven_day","used_percent":86.0,"resets_at":10_000u64 + 2 * 86400}), 10_000).unwrap(), "週間  残り14% · 2日0時間後にリセット");
            assert_eq!(window_line(&json!({"scope":"five_hour","used_percent":25.0,"resets_at":5}), 10).unwrap(), "5時間  残り75% · まもなくリセット");
            assert_eq!(allowance(&profiles(&snap())[0]), "5時間の62%を使用");
            assert_eq!(menu_rows(&snap(), 0)[2].detail, "未接続"); assert_eq!(menu_rows(&snap(), 0)[0].label, "既定のアカウント");
        });
    }
    #[test] fn a_window_says_what_is_left_and_when_it_resets() {
        assert_eq!(window_line(&json!({"scope":"seven_day","used_percent":86.0,"resets_at":10_000u64 + 2 * 86400}), 10_000).unwrap(), "Weekly  14% left · resets in 2d 0h");
        assert_eq!(window_line(&json!({"scope":"seven_day_fable","label":"Fable weekly","used_percent":0.0}), 0).unwrap(), "Fable weekly  100% left", "the hub's own name");
        assert_eq!(resets_in(5, 10), "now"); assert_eq!(resets_in(10 + 720, 10), "in 12m");
    }
}

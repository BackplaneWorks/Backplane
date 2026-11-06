//! Settings and the other row lists the hub describes (`settings.rows`: label, note, buttons, fields, QR code).
//! Pure parsing and a generic renderer: every button is the view action the row names, every typed field goes out as
//! `bfield` ("<name>\u{1f}<text>") exactly as on the phones. Nothing here knows what a setting means.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Btn { pub label: String, pub action: String, pub value: String, pub on: bool }
/// A field to type in. A `vault` field (a key or a secret's value) is the app's own: what is typed is held here and handed over
/// once, at Save, in one request; it is never a `bfield`, never taken back from the hub, never in a format string (no Debug).
#[derive(Clone, PartialEq)]
pub struct Field { pub name: String, pub hint: String, pub secret: bool, pub value: String, pub vault: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Row { pub label: String, pub note: String, pub buttons: Vec<Btn>, pub fields: Vec<Field>, pub qr: Vec<String>, pub qrnote: String, pub local: bool }
impl std::fmt::Debug for Field { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "Field({}{})", self.name, if self.vault { ", vault" } else { "" }) } }

/// A request to the hub that carries a secret: its Debug shows nothing.
#[derive(Clone)]
pub struct Secret(pub Value);
impl std::fmt::Debug for Secret { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("Secret(..)") } }

/// At most this many rows are drawn at once; the rest are said ("N more"), never hidden.
pub const MAX_ROWS: usize = 300;

pub fn parse(settings: &Value) -> Vec<Row> {
    settings["rows"].as_array().into_iter().flatten().map(|r| Row {
        label: string(r, "label"), note: string(r, "note"),
        buttons: r["buttons"].as_array().into_iter().flatten().map(|b| Btn { label: string(b, "label"), action: string(b, "action"), value: string(b, "value"), on: b["on"].as_bool().unwrap_or(false) }).collect(),
        fields: r["fields"].as_array().into_iter().flatten().map(|f| { let vault = f["vault"].as_bool().unwrap_or(false); Field { name: string(f, "name"), hint: string(f, "hint"), secret: f["secret"].as_bool().unwrap_or(false), value: if vault { String::new() } else { string(f, "value") }, vault } }).collect(),
        qr: r["qr"].as_array().into_iter().flatten().filter_map(|l| l.as_str().map(str::to_string)).collect(),
        qrnote: string(r, "qrnote"), local: r["local"].as_bool().unwrap_or(false),
    }).collect()
}

/// What a typed change sends: a form field's text as `bfield`; a vault field's text never (None).
pub fn change(f: &Field, text: &str) -> Option<Command> { if f.vault { None } else { Some(Command::Action("bfield".into(), format!("{}\u{1f}{}", f.name, text))) } }
pub fn field_command(name: &str, text: &str) -> Command { Command::Action("bfield".into(), format!("{name}\u{1f}{text}")) }
/// What a button press sends: its action and value, except the vault Saves, which are `save_request`.
pub fn press(b: &Btn) -> Command { Command::Action(b.action.clone(), b.value.clone()) }
/// `open-url` is the client's own (it never goes to the hub): only web addresses open, in the system browser.
pub fn openable(url: &str) -> bool { (url.starts_with("https://") || url.starts_with("http://")) && url.len() <= 8000 && !url.chars().any(|c| c.is_whitespace() || c.is_control()) }
pub fn is_vault_save(b: &Btn) -> bool { b.action == "secret-save" }

/// The request a vault Save sends, straight to the hub (never a view action): an API key (`@apikey` + name) is `bots.voice`
/// `api-add`; a secret ("name\u001fpurpose\u001fthread") is `secret.set`. Nothing is sent without a name and a value.
pub fn save_request(packed: &str, typed_name: &str, value: &str) -> Result<(String, Value), &'static str> {
    let value = value.trim();
    if packed.starts_with("@apikey\u{1f}") {
        let rest = &packed["@apikey\u{1f}".len()..];
        let name = if typed_name.trim().is_empty() { rest.trim() } else { typed_name.trim() };
        if name.is_empty() { return Err("Name the key first."); }
        if value.is_empty() { return Err("Paste the key first."); }
        return Ok(("bots.voice".into(), serde_json::json!({"op": "api-add", "name": name, "value": value})));
    }
    let mut it = packed.split('\u{1f}');
    let (packed_name, purpose, thread) = (it.next().unwrap_or("").trim(), it.next().unwrap_or(""), it.next().unwrap_or(""));
    let name = if typed_name.trim().is_empty() { packed_name } else { typed_name.trim() };
    if name.is_empty() { return Err("Name the secret first."); }
    if value.is_empty() { return Err("Paste the secret first."); }
    Ok(("secret.set".into(), serde_json::json!({"name": name, "purpose": purpose, "thread": thread, "value": value})))
}

/// Recent texts sent for a field, so the hub's late echo of an older text is not mistaken for a change made elsewhere. Replies come in
/// order: once the hub echoes the newest text sent, everything earlier is settled and any other value is a real change.
const SENT_KEPT: usize = 32;
#[derive(Default, Debug)]
pub struct Echo { sent: String, log: std::collections::VecDeque<String> }
impl Echo {
    pub fn new(v: &str) -> Self { let mut e = Echo::default(); e.remember(v); e }
    pub fn remember(&mut self, t: &str) { self.sent = t.to_string(); self.log.push_back(t.to_string()); while self.log.len() > SENT_KEPT { self.log.pop_front(); } }
    pub fn sent(&self) -> &str { &self.sent }
    /// The hub shows `hub` while `typed` is in the field: Some(true) take it, Some(false) ignore it (an echo or the same text).
    pub fn on_hub(&mut self, hub: &str, typed: &str) -> bool {
        if hub == self.sent { self.log.clear(); self.log.push_back(self.sent.clone()); return false; }   // caught up
        hub != typed && !self.log.iter().any(|s| s == hub)
    }
}

/// The settings dialog's own state (the hub keeps the page; this keeps the list the left nav shows and what is typed in its search).
#[derive(Default)]
pub struct SettingsUi { pub accounts: bool, pub nav: Vec<(String, String)>, pub page: String, pub q: String, pub search: Option<Entity<Composer>>, pub auto: bool, pub open_local: bool, pub device: bool }
/// The pages in the order and groups the nav shows them (the hub's menu order, grouped with a divider between groups). An unknown page goes last.
const GROUPS: [&[&str]; 4] = [&["appearance", "threads", "agents", "writing"], &["cad", "ordering", "devices"], &["network", "voice", "keys", "timezone"], &["usage", "about", "privacy"]];
pub fn group_of(page: &str) -> usize { GROUPS.iter().position(|g| g.contains(&page)).unwrap_or(GROUPS.len()) }
/// An icon (from the set the client carries) for each page.
pub fn nav_icon(page: &str) -> &'static str {
    match page { "appearance" => "sun", "threads" => "message-square", "agents" => "cpu", "writing" => "square-pen", "cad" => "circuit-board", "ordering" => "shopping-cart",
        "devices" => "link", "network" => "globe", "voice" => "mic", "keys" => "code", "timezone" => "clock", "usage" => "gauge", "about" => "book", "privacy" => "file-text", _ => "settings" }
}
/// The hub shows its page menu (rows that are each one `settings-page` button naming a page) or one page (its first row is the way back).
#[derive(Debug, PartialEq)]
pub enum Shown { Menu(Vec<(String, String)>), Page, Other }
pub fn shown(rows: &[Row]) -> Shown {
    let items: Vec<(String, String)> = rows.iter().filter_map(|r| r.buttons.iter().find(|b| b.action == "settings-page" && !b.value.is_empty()).map(|b| (b.value.clone(), r.label.clone()))).collect();
    if items.len() >= 3 && items.len() * 2 >= rows.len() { return Shown::Menu(items); }
    if rows.first().map_or(false, |r| r.buttons.iter().any(|b| b.action == "settings-page" && b.value.is_empty())) { return Shown::Page; }
    Shown::Other
}
/// The nav list: the menu's pages in group order (stable within a group).
pub fn nav_order(items: &[(String, String)]) -> Vec<(String, String)> { let mut v = items.to_vec(); v.sort_by_key(|(p, _)| group_of(p)); v }
/// Whether a nav title matches what is typed in the search (an empty search matches all).
pub fn nav_matches(title: &str, q: &str) -> bool { let q = q.trim().to_lowercase(); q.is_empty() || title.to_lowercase().contains(&q) }

pub struct FieldSlot { pub input: Entity<Composer>, pub vault: bool, pub hub: String, echo: Echo, pending: Option<String>, timer: bool }

impl FieldSlot { fn remember(&mut self, t: &str) { self.echo.remember(t); } }

/// One composer per field name. A form field's typing goes out as `bfield`, coalesced (the first change at once, the rest the newest after
/// 30 ms); the hub's value is taken only when it is a real change. A vault field holds its text here and sends nothing until Save.
pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let rows = parse(&this.data["settings"]);
    sync_nav(this, &rows, cx);
    let mut want: Vec<Field> = rows.into_iter().flat_map(|r| r.fields).collect();
    want.extend(super::bots::form_fields(&this.data)); want.extend(super::panels::form_fields(&this.data)); want.extend(super::tools::form_fields(&this.data));
    this.screens.fields.retain(|n, _| want.iter().any(|f| f.name == *n));
    for f in want {
        if let Some(slot) = this.screens.fields.get_mut(&f.name) {
            if slot.vault || slot.hub == f.value { continue; }
            slot.hub = f.value.clone();
            let typed = slot.input.read(cx).content.to_string();
            if slot.echo.on_hub(&f.value, &typed) { let (input, v) = (slot.input.clone(), f.value.clone()); slot.remember(&v); input.update(cx, |i, cx| i.set(v, cx)); }
            continue;
        }
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        let (hint, secret, v) = (f.hint.clone(), f.secret, f.value.clone());
        input.update(cx, |i, cx| { i.set_placeholder(&hint); i.masked = secret; i.set(v, cx); });
        let field = f.clone();
        if !f.vault {
            cx.observe(&input, move |this, input, cx| {
                let text = input.read(cx).content.to_string();
                let Some(slot) = this.screens.fields.get_mut(&field.name) else { return };
                if slot.echo.sent() == text { return; }
                slot.pending = Some(text);
                send_field(this, &field, cx);
            }).detach();
        }
        let sname = f.name.clone();
        cx.subscribe(&input, move |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { submit(this, &sname, cx); }).detach();
        this.screens.fields.insert(f.name.clone(), FieldSlot { input, vault: f.vault, hub: f.value.clone(), echo: Echo::new(&f.value), pending: None, timer: false });
    }
}

/// Keep the nav's pages from the hub's menu, open the first page when the dialog opens on the menu, and own the search box.
fn sync_nav(this: &mut Backplane, rows: &[Row], cx: &mut Context<Backplane>) {
    if !this.data["settings"].is_object() { let ui = &mut this.screens.settings_ui; ui.page.clear(); ui.auto = false; return; }
    if this.screens.settings_ui.search.is_none() {
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        input.update(cx, |i, _| { i.set_placeholder("Search"); });
        cx.observe(&input, |this, input, cx| { this.screens.settings_ui.q = input.read(cx).content.to_string(); cx.notify(); }).detach();
        this.screens.settings_ui.search = Some(input);
    }
    if let Shown::Menu(items) = shown(rows) {
        let nav = nav_order(&items);
        let ui = &mut this.screens.settings_ui;
        ui.nav = nav;
        if !ui.auto { ui.auto = true; if let Some((p, _)) = ui.nav.first().cloned() { ui.page = p.clone(); let _ = this.tx.try_send(Command::Action("settings-page".into(), p)); } }
    }
}

/// Send the newest pending text of a field if there is no window running; start the 30 ms window. `sent` moves only when the send worked.
fn send_field(this: &mut Backplane, f: &Field, cx: &mut Context<Backplane>) {
    let Some(slot) = this.screens.fields.get_mut(&f.name) else { return };
    if slot.timer { return; }
    let Some(text) = slot.pending.take() else { return };
    if text == slot.echo.sent() { return; }
    let Some(cmd) = change(f, &text) else { return };
    match this.tx.try_send(cmd) { Ok(_) => slot.remember(&text), Err(_) => slot.pending = Some(text) }
    slot.timer = true;
    let f = f.clone();
    cx.spawn(async move |weak, cx| {
        cx.background_executor().timer(std::time::Duration::from_millis(30)).await;
        let _ = weak.update(cx, |this, cx| {
            if let Some(s) = this.screens.fields.get_mut(&f.name) { s.timer = false; }
            send_field(this, &f, cx);
        });
    }).detach();
}

/// Send every field's pending (coalesced) text now: called before any button action, so a fast press never overtakes the last keystrokes.
pub fn flush_fields(this: &mut Backplane) {
    let names: Vec<String> = this.screens.fields.iter().filter(|(_, s)| !s.vault && s.pending.is_some()).map(|(n, _)| n.clone()).collect();
    for n in names {
        let Some(slot) = this.screens.fields.get_mut(&n) else { continue };
        let Some(text) = slot.pending.take() else { continue };
        if text == slot.echo.sent() { continue; }
        match this.tx.try_send(field_command(&n, &text)) { Ok(_) => slot.remember(&text), Err(_) => slot.pending = Some(text) }
    }
}

/// Add (or Enter) on the to-do list's new-step box: sent once, and the box is empty again.
pub fn send_todo(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.fields.get("todonew").map(|s| s.input.clone()) else { return };
    if input.read(cx).content.trim().is_empty() { return; }
    flush_fields(this);
    let _ = this.tx.try_send(Command::Action("todo-add".into(), String::new()));
    input.update(cx, |i, cx| i.set(String::new(), cx));
    cx.notify();
}

/// Enter in a typed field: a room's or a remote bot's post box sends, as on the window.
pub fn submit(this: &mut Backplane, name: &str, cx: &mut Context<Backplane>) {
    match name { "rpost" => super::bots::send_post(this, "rpost", "room-post", cx), "rtext" => super::bots::send_post(this, "rtext", "remote-send", cx), "title" => { // Enter in the rename box saves, as its Rename button does (nothing for a blank title)
            let id = string(&this.data["renaming"], "id");
            let text = this.screens.fields.get("title").map(|s| s.input.read(cx).content.to_string()).unwrap_or_default();
            if let Some(c) = super::tools::rename_save(&id, &text) { flush_fields(this); let _ = this.tx.try_send(c); cx.notify(); }
        }
        // Enter in a settings field is its row's Save
        "whurl" | "whsec" | "tzname" => { flush_fields(this); let a = match name { "whurl" => "hook-url", "whsec" => "hook-secret", _ => "tz-set" }; let _ = this.tx.try_send(Command::Action(a.into(), String::new())); cx.notify(); }
        "todonew" => send_todo(this, cx),
        n if n.starts_with("todo.") => { flush_fields(this); let _ = this.tx.try_send(Command::Action("todo-text".into(), n[5..].to_string())); cx.notify(); }
        n => if let Some(a) = n.strip_prefix("sp.") { super::bots::send_space(this, a, cx) } }
}

/// A vault Save: the field is the button's (`@apikey` is `kval`, else `sval`); the typed value leaves in one request straight to the hub, and
/// the field is emptied (history too) only when the hub accepted it. Nothing sent says why, as the phones do (`key-err`, `secret-err`).
pub fn save_vault(this: &mut Backplane, b: &Btn, cx: &mut Context<Backplane>) {
    let key = b.value.starts_with("@apikey\u{1f}");
    let (field, err) = if key { ("kval", "key-err") } else { ("sval", "secret-err") };
    let say = |this: &mut Backplane, why: &str| { let _ = this.tx.try_send(Command::Action(err.into(), why.into())); };
    let Some(input) = this.screens.fields.get(field).map(|s| s.input.clone()) else { return };
    let typed_name = this.screens.fields.get(if key { "kname" } else { "sname" }).map(|s| s.input.read(cx).content.to_string()).unwrap_or_default();
    let value = input.read(cx).content.to_string();
    match save_request(&b.value, &typed_name, &value) {
        Err(why) => say(this, why),
        Ok((m, p)) => {
            let tag = if key { "key" } else { "secret" };
            // the field is kept until the hub's answer (rpc_done): accepted empties it and shuts the form, refused says why
            if this.tx.try_send(Command::Rpc(m, Secret(p), tag)).is_err() { say(this, "The hub is not reachable. Try again."); }
        }
    }
    cx.notify();
}

/// Why the hub refused a request: its `error` is a plain string (`{"ok":false,"error":"..."}`); an object with `message` or `code` is read too.
pub fn reply_error(x: &Value) -> String {
    let e = &x["error"];
    e.as_str().or_else(|| e["message"].as_str()).or_else(|| e["code"].as_str()).unwrap_or("").to_string()
}

/// The hub answered a direct request: a secret's outcome is shown by the client (`secret-sent` shuts the box, `secret-err` says why).
pub fn rpc_done(this: &mut Backplane, tag: &str, ok: bool, why: &str, cx: &mut Context<Backplane>) {
    if tag == "browser" { if !ok { if let Some((_, page)) = &this.screens.browser { let page = page.clone(); page.update(cx, |p, cx| p.open_failed(why, cx)); } } return; }
    if tag.starts_with("hook") { return super::bots::hook_done(this, tag, ok, why, cx); }
    let (name, sent, err) = match tag { "key" => ("kval", "key-sent", "key-err"), "secret" => ("sval", "secret-sent", "secret-err"), _ => return };
    let (a, v) = if ok { (sent, String::new()) } else { (err, if why.is_empty() { format!("The hub refused the {}", if tag == "key" { "key" } else { "secret" }) } else { why.to_string() }) };
    // the typed value is kept until the hub accepted it, so a refusal can be corrected and sent again
    if ok { if let Some(input) = this.screens.fields.get(name).map(|s| s.input.clone()) { input.update(cx, |i, cx| i.set(String::new(), cx)); } }
    let _ = this.tx.try_send(Command::Action(a.into(), v));
}

fn qr(light: bool, lines: &[String]) -> AnyElement {
    let n = lines.len(); let cell = 4.0_f32;
    let dark = theme::color(light, "text"); let bg = theme::color(light, "view");
    let rows: Vec<Vec<bool>> = lines.iter().map(|l| l.chars().map(|c| c == '1').collect()).collect();
    div().p_2().bg(bg).child(canvas(|_, _, _| (), move |bounds, _, window, _| {
        for (y, row) in rows.iter().enumerate() { for (x, on) in row.iter().enumerate() {
            if *on { window.paint_quad(fill(Bounds::new(point(bounds.left() + px(x as f32 * cell), bounds.top() + px(y as f32 * cell)), size(px(cell), px(cell))), dark)); }
        } }
    }).size(px(n as f32 * cell))).into_any_element()
}

/// The overlay's structure as one JSON text: each row's label, whether it has a note, its buttons (label, action, value, on) and field names. No field values.
pub fn structure(settings: &Value) -> String {
    let rows: Vec<Value> = parse(settings).iter().map(|r| serde_json::json!({"label": r.label, "note": !r.note.is_empty(), "buttons": r.buttons.iter().map(|b| serde_json::json!([b.label, b.action, b.value, b.on])).collect::<Vec<_>>(), "fields": r.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>(), "qr": !r.qr.is_empty()})).collect();
    serde_json::json!({"keys": settings.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(), "rows": rows}).to_string()
}

/// The most chips on one line of a row's controls.
const CHIPS_PER_LINE: usize = 6;
/// A hub row as a dialog row: label and muted note on the left, the controls on the right, a hairline under it. A row that is only a label is a
/// section heading; a label and a note with nothing to press or type is a paragraph.
fn row_el(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, r: &Row, i: usize) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let controls = !r.buttons.is_empty() || r.fields.iter().any(|f| fields.contains_key(&f.name)) || !r.qr.is_empty();
    if !controls && r.note.is_empty() {
        return div().pt_4().pb_1().text_size(px(14.)).font_weight(FontWeight::BOLD).child(r.label.clone()).into_any_element();
    }
    let mut left = div().flex().flex_col().gap_1().flex_1().min_w_0();
    if !r.label.is_empty() { left = left.child(div().text_size(px(13.)).child(r.label.clone())); }
    if !r.note.is_empty() { left = left.child(div().text_size(px(12.)).text_color(c("dim")).child(r.note.clone())); }
    let mut right = div().flex().flex_col().items_end().gap_2().flex_shrink_0().max_w(px(430.));
    for f in &r.fields {
        if let Some(slot) = fields.get(&f.name) { right = right.child(div().w(px(260.)).px_2().py_1().bg(c("raise")).child(slot.input.clone())); }
    }
    if !r.qr.is_empty() { right = right.child(qr(light, &r.qr)); if !r.qrnote.is_empty() { right = right.child(div().text_size(px(11.)).text_color(c("dim")).child(r.qrnote.clone())); } }
    if !r.buttons.is_empty() {
        // chips in lines of at most CHIPS_PER_LINE (a wrapping flex row does not grow its parent's height in gpui, and the next row would be drawn over it)
        for (line, chunk) in r.buttons.chunks(CHIPS_PER_LINE).enumerate() {
            right = right.child(div().flex().justify_end().gap_1().children(chunk.iter().enumerate().map(|(k, b)| {
                let j = line * CHIPS_PER_LINE + k; let b2 = b.clone();
                let d = div().id(SharedString::from(format!("set-{i}-{j}"))).px_3().py_1().text_size(px(12.)).cursor_pointer();
                let d = if b.on { d.bg(c("accent")).text_color(c("on-accent")) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
                d.child(b.label.clone()).on_click(cx.listener(move |this, _, _, cx| { if let Some(name) = b2.action.strip_prefix("local:") { this.local_press(name, &b2.value, cx); return; } if b2.action == "open-url" { if openable(&b2.value) { cx.open_url(&b2.value); } return; } flush_fields(this); if is_vault_save(&b2) { save_vault(this, &b2, cx); } else { let _ = this.tx.try_send(press(&b2)); } cx.notify(); })).into_any_element()
            }).collect::<Vec<_>>()));
        }
    }
    div().flex().items_center().gap_4().py_3().border_b_1().border_color(c("line")).child(left).child(right).into_any_element()
}

/// The dialog while the hub has settings open; None when it has not: a title bar with a close ×, the pages on the left (search above, an icon and a
/// label each, the open one on a rounded fill, a thin divider between groups) and the page's rows on the right with their own scroll.
/// Settings shut (the ×, Esc, a click outside): this device's page and the accounts page at once, and the hub's pages when it has them open.
pub fn close(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let hub = this.data["settings"].is_object();
    let ui = &mut this.screens.settings_ui; ui.open_local = false; ui.device = false; ui.accounts = false;
    this.screens.acct.menu = false;
    if hub { let _ = this.tx.try_send(Command::Action("flag".into(), "settings".into())); }
    cx.notify();
}
/// Whether any settings page is up (the hub's, or this device's and the accounts page, which the hub need not serve).
pub fn is_open(this: &Backplane) -> bool { this.data["settings"].is_object() || this.screens.settings_ui.open_local }

pub fn view(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, settings: &Value, focus: &FocusHandle, ui: &SettingsUi, local: &crate::local::Local, acct: &super::accounts::Accounts) -> Option<AnyElement> {
    let hub_on = settings.is_object();
    if !hub_on && !ui.open_local { return None; }
    let on_acct = ui.accounts;
    let on_device = (ui.device || !hub_on) && !on_acct;
    let c = |n: &str| theme::color(light, n);
    let rows = parse(settings);
    let on_page = hub_on && !on_device && !on_acct && matches!(shown(&rows), Shown::Page);
    let skip = if on_page { 1 } else { 0 };
    let dev_rows = device_rows(local);
    let mut els: Vec<AnyElement> = if on_acct { super::accounts::page(cx, light, acct) } else if on_device { dev_rows.iter().enumerate().map(|(i, r)| row_el(cx, light, fields, r, i)).collect() } else { rows.iter().skip(skip).take(MAX_ROWS).enumerate().map(|(i, r)| row_el(cx, light, fields, r, i)).collect() };
    if !on_device && !on_acct && rows.len() > MAX_ROWS + skip { els.push(div().py_2().text_color(c("dim")).child(format!("… {} more", rows.len() - MAX_ROWS - skip)).into_any_element()); }
    // the left nav
    let mut nav = div().flex().flex_col().gap(px(2.)).p_3().w(px(220.)).flex_shrink_0().border_r_1().border_color(c("line"));
    if let Some(input) = &ui.search { nav = nav.child(div().flex().items_center().gap_2().px_2().py_1().mb_2().bg(c("raise")).child(crate::art::icon("search", 14., c("dim"))).child(div().flex_1().child(input.clone()))); }
    {
        let item = div().id("set-nav-device").flex().items_center().gap_3().px_3().py(px(6.)).cursor_pointer().text_size(px(13.));
        let item = if on_device { item.bg(c("raise")) } else { item.hover(|s| s.bg(c("panel"))) };
        nav = nav.child(div().px_3().pb_1().text_size(px(11.)).text_color(c("dim")).child("THIS DEVICE"))
            .child(item.child(crate::art::icon("settings", 16., c(if on_device { "text" } else { "dim" }))).child("This device").on_click(cx.listener(|this, _, _, cx| { flush_fields(this); this.screens.settings_ui.device = true; this.screens.settings_ui.accounts = false; cx.notify(); })));
        let acct_item = div().id("set-nav-accounts").flex().items_center().gap_3().px_3().py(px(6.)).cursor_pointer().text_size(px(13.));
        let acct_item = if on_acct { acct_item.bg(c("raise")) } else { acct_item.hover(|s| s.bg(c("panel"))) };
        nav = nav.child(div().my_2().mx_2().h(px(1.)).bg(c("line"))).child(div().px_3().pb_1().text_size(px(11.)).text_color(c("dim")).child("ALL DEVICES (HUB)"))
            .child(acct_item.child(crate::art::icon("user", 16., c(if on_acct { "text" } else { "dim" }))).child("Accounts").on_click(cx.listener(|this, _, _, cx| { flush_fields(this); this.screens.settings_ui.device = false; this.screens.settings_ui.accounts = true; super::accounts::open_page(this, cx); cx.notify(); })));
        if !hub_on { nav = nav.child(div().px_3().pt_2().text_size(px(11.)).text_color(c("dim")).child("The hub's other settings open here when it serves them.")); }
    }
    let mut last = usize::MAX;
    let title_now = rows.first().map(|r| r.label.clone()).unwrap_or_default();
    for (n, (page, t_nav)) in ui.nav.iter().enumerate().filter(|(_, (_, t))| hub_on && nav_matches(t, &ui.q)) {
        let title = t_nav;
        let g = group_of(page);
        if last != usize::MAX && g != last { nav = nav.child(div().my_2().mx_2().h(px(1.)).bg(c("line"))); }
        last = g;
        let (p2, on) = (page.clone(), !on_device && !on_acct && if on_page { title_now == *t_nav } else { *page == ui.page });
        let item = div().id(SharedString::from(format!("set-nav-{n}"))).flex().items_center().gap_3().px_3().py(px(6.)).cursor_pointer().text_size(px(13.));
        let item = if on { item.bg(c("raise")) } else { item.hover(|s| s.bg(c("panel"))) };
        nav = nav.child(item.child(crate::art::icon(nav_icon(page), 16., c(if on { "text" } else { "dim" }))).child(title.clone())
            .on_click(cx.listener(move |this, _, _, cx| { flush_fields(this); this.screens.settings_ui.device = false; this.screens.settings_ui.accounts = false; this.screens.settings_ui.page = p2.clone(); let _ = this.tx.try_send(Command::Action("settings-page".into(), p2.clone())); cx.notify(); })));
    }
    let title = if on_acct { "Accounts".to_string() } else if on_device { "This device".to_string() } else if on_page { rows[0].label.clone() } else { ui.nav.iter().find(|(p, _)| *p == ui.page).map(|(_, t)| t.clone()).unwrap_or_default() };
    let dialog = div().id("settings-dialog").flex().flex_col().w(px(1040.)).max_w_full().h(px(760.)).max_h_full().border_1().border_color(c("line")).bg(c("bg")).overflow_hidden().on_mouse_down_out(cx.listener(|this, _, _, cx| close(this, cx)))
        .child(div().h(px(44.)).flex_shrink_0().px_4().flex().items_center().border_b_1().border_color(c("line"))
            .child(div().flex_1().font_weight(FontWeight::BOLD).text_size(px(14.)).child("Settings"))
            .child(div().id("settings-close").p_1().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("text")).bg(c("raise"))).child(crate::art::icon("x", 16., c("dim")))
                .on_click(cx.listener(|this, _, _, cx| close(this, cx)))))
        .child(div().flex().flex_1().min_h_0().child(nav)
            .child(div().id("settings-rows").flex_1().min_w_0().overflow_y_scroll().px_6().py_3().flex().flex_col()
                .child(div().pb_1().text_size(px(15.)).font_weight(FontWeight::BOLD).child(title))
                .child(div().pb_2().text_size(px(12.)).text_color(c(if on_device { "accent" } else { "dim" })).child(if on_acct { "Kept in the hub for this machine; sign-ins never leave it." } else if on_device { "Kept on this computer only. Never sent to the hub, and other devices keep their own." } else { "Stored in the hub: the same on all your devices." })).children(els)));
    Some(div().id("settings").absolute().inset_0().occlude().track_focus(focus).bg(gpui::hsla(0., 0., 0., 0.55)).flex().items_center().justify_center().p_6().child(dialog).into_any_element())
}

/// The rows of the "This device" page: what only this computer decides. Every button is `local:<name>` with a value; none reaches the hub.

pub fn device_rows(l: &crate::local::Local) -> Vec<Row> {
    use crate::local as lo;
    let b = |label: &str, name: &str, value: &str, on: bool| Btn { label: label.into(), action: format!("local:{name}"), value: value.into(), on };
    let r = |label: &str, note: &str, buttons: Vec<Btn>| Row { label: label.into(), note: note.into(), buttons, fields: vec![], qr: vec![], qrnote: String::new(), local: true };
    let mut rows = vec![
        r("Appearance", "", vec![]),
        r("Theme", "Light or dark on this device, or follow what the hub says.", lo::THEMES.iter().map(|(k, lab)| b(lab, "theme", k, l.theme == *k)).collect()),
        r("Text size", &format!("{}% of the standard size.", l.text_pct), vec![b("Smaller", "text", "-", false), b("Larger", "text", "+", false), b("Reset", "text-reset", "", l.text_pct == 100)]),
        r("Font", "The typeface of the interface and the thread.", lo::FONTS.iter().map(|(k, lab)| b(lab, "font", k, l.font == *k)).collect()),
        r("Layout", "", vec![]),
        r("Sidebar width", &format!("{} px.", l.sidebar_px), vec![b("Narrower", "sidebar-width", "-", false), b("Wider", "sidebar-width", "+", false)]),
        r("Sidebar", "Whether it is folded when the app opens.", vec![b("Open", "sidebar-open", "on", l.sidebar_open), b("Folded", "sidebar-open", "off", !l.sidebar_open)]),
        r("Opens on", "What this device shows first.", lo::VIEWS.iter().map(|(k, lab)| b(lab, "default-view", k, l.default_view == *k)).collect()),
        r("Notifications", "", vec![]),
        r("Desktop notifications", "Kept for this device. Alerts for what needs you and for settled work show as desktop notifications.", vec![b("On", "notifications", "on", l.notifications), b("Off", "notifications", "off", !l.notifications)]),
        r("Keyboard shortcuts", "", vec![]),
    ];
    for x in crate::keys::TABLE {
        if x.kind == crate::keys::Kind::Fixed { rows.push(r(&format!("{} · {}", x.label, x.scope), &format!("{}. Fixed.", crate::keys::hint(l, x.id)), vec![])); }
        else { let k = crate::keys::hint(l, x.id); rows.push(r(&format!("{} · {}", x.label, x.scope), &format!("Now {k}. Press to take the next key."), vec![b(&k, "shortcut", x.id, false)])); }
    }
    for (k, why) in crate::keys::MISSING { rows.push(r(k, &format!("Not yet: {why}."), vec![])); }
    rows.push(r("Shortcuts to defaults", "", vec![b("Reset", "shortcuts-reset", "", false)]));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn the_device_page_lists_the_drawing_pads_keys_and_no_longer_says_they_are_missing() {
        let rows = device_rows(&crate::local::Local::default());
        let note = |label: &str| rows.iter().find(|r| r.label == label).map(|r| r.note.clone());
        assert_eq!(note("Undo the last stroke or change · Drawing pad").as_deref(), Some("Ctrl+Z. Fixed."));
        assert_eq!(note("Delete the selection, or clear the pad · Drawing pad").as_deref(), Some("Delete. Fixed."));
        assert_eq!(note("Delete (also) · Drawing pad").as_deref(), Some("Backspace. Fixed."));
        assert!(!rows.iter().any(|r| r.note.contains("Not yet") && r.label.to_lowercase().contains("drawing pad")));
    }
    #[test] fn the_performance_overlay_is_a_shortcut_on_the_device_page_and_no_longer_a_not_yet() {
        let rows = device_rows(&crate::local::Local::default());
        let r = rows.iter().find(|r| r.label.starts_with("Performance overlay")).expect("its row");
        assert!(r.note.contains("Ctrl+Shift+H") || r.note.contains("⌃⇧H"), "{}", r.note);
        assert_eq!((r.buttons[0].action.as_str(), r.buttons[0].value.as_str()), ("local:shortcut", "perf"));
        assert!(!rows.iter().any(|r| r.note.starts_with("Not yet") && r.note.contains("Performance")));
    }
    #[test] fn rows_parse_with_buttons_fields_and_qr_and_a_vault_field_keeps_no_hub_value() {
        let r = parse(&j(r#"{"rows":[{"label":"Theme","note":"light or dark","buttons":[{"label":"Dark","action":"theme","value":"dark","on":true},{"label":"Light","action":"theme","value":"light","on":false}]},
            {"label":"Key","note":"","buttons":[],"fields":[{"name":"kname","hint":"name","secret":false,"value":"OPENAI"},{"name":"kval","hint":"value","secret":true,"vault":true,"value":"leaked-from-hub"}]},
            {"label":"Pair","note":"","buttons":[],"qr":["101","010","101"],"qrnote":"scan"}]}"#));
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].buttons[0], Btn { label: "Dark".into(), action: "theme".into(), value: "dark".into(), on: true });
        assert!(!r[1].fields[0].vault && r[1].fields[0].value == "OPENAI"); assert!(r[1].fields[1].vault && r[1].fields[1].value.is_empty(), "a vault value is never taken from the hub");
        assert_eq!(r[2].qr.len(), 3); assert_eq!(r[2].qrnote, "scan"); assert!(parse(&Value::Null).is_empty());
    }
    #[test] fn a_button_press_is_its_action_and_a_form_field_change_is_bfield_name_unit_text() {
        match press(&Btn { label: "x".into(), action: "theme".into(), value: "dark".into(), on: false }) { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("theme", "dark")), c => panic!("{c:?}") }
        match field_command("kname", "OPENAI_API_KEY") { Command::Action(a, v) => { assert_eq!(a, "bfield"); assert_eq!(v, "kname\u{1f}OPENAI_API_KEY"); } c => panic!("{c:?}") }
    }
    /// The secret never rides a view action: typing in a vault field sends nothing, and the Save is a direct request.
    #[test] fn only_web_addresses_open() { assert!(openable("https://backplane.works/privacy") && openable("http://127.0.0.1:1/x")); for u in ["file:///etc/passwd", "javascript:alert(1)", "https://a b", "", "ftp://x"] { assert!(!openable(u), "{u}"); } }
    #[test] fn a_vault_value_never_travels_as_a_command_action() {
        let vault = Field { name: "kval".into(), hint: "".into(), secret: true, value: String::new(), vault: true };
        let plain = Field { name: "kname".into(), hint: "".into(), secret: false, value: String::new(), vault: false };
        assert!(change(&vault, "sk-SECRET").is_none()); assert!(change(&plain, "OPENAI").is_some());
        let (m, p) = save_request("@apikey\u{1f}OPENAI", "", "sk-SECRET").unwrap();
        assert_eq!((m.as_str(), p["op"].as_str(), p["name"].as_str(), p["value"].as_str()), ("bots.voice", Some("api-add"), Some("OPENAI"), Some("sk-SECRET")));
        let cmd = Command::Rpc(m, Secret(p), "key");
        assert!(!format!("{cmd:?}").contains("SECRET"), "Debug of a request shows no secret"); assert!(!format!("{vault:?}").contains("SECRET"));
        let (m, p) = save_request("db\u{1f}for the db\u{1f}t1", "", "pw").unwrap();
        assert_eq!((m.as_str(), p["name"].as_str(), p["purpose"].as_str(), p["thread"].as_str()), ("secret.set", Some("db"), Some("for the db"), Some("t1")));
        assert!(save_request("@apikey\u{1f}X", "", "   ").is_err(), "no value, no request"); assert!(save_request("@apikey\u{1f}", "", "v").unwrap_err().contains("Name"), "no name says why");
        assert_eq!(save_request("@apikey\u{1f}old", "typed", "v").unwrap().1["name"], "typed", "the name typed last wins");
    }
    /// Type "abc": the reply for "a" arrives late and must not put "a" back; when the hub really clears the field afterwards, it is cleared.
    #[test] fn the_hubs_reason_is_read_from_a_plain_error_string() {
        assert_eq!(reply_error(&j(r#"{"t":"reply","id":1073741825,"ok":false,"error":"A secret is at least 8 characters"}"#)), "A secret is at least 8 characters");
        assert_eq!(reply_error(&j(r#"{"ok":false,"error":{"message":"nope","code":"bad"}}"#)), "nope"); assert_eq!(reply_error(&j(r#"{"ok":false,"error":{"code":"bad"}}"#)), "bad");
        assert_eq!(reply_error(&j(r#"{"ok":true}"#)), "");
    }
    #[test] fn echoes_are_ignored_and_a_real_clear_after_catching_up_is_taken() {
        let mut e = Echo::new("");
        for t in ["a", "ab", "abc"] { e.remember(t); }
        assert!(!e.on_hub("a", "abc"), "a late echo of an older text"); assert!(!e.on_hub("ab", "abc"));
        assert!(!e.on_hub("abc", "abc"), "caught up: the log is settled");
        assert!(e.on_hub("", "abc"), "the hub cleared the field (a Save or a Set): that is a real change");
        let mut f = Echo::new(""); f.remember("x");
        assert!(f.on_hub("changed on another device", "x"), "a real change from elsewhere");
    }
    fn row(label: &str, page: &str) -> Row { Row { label: label.into(), note: String::new(), buttons: if page.is_empty() { vec![] } else { vec![Btn { label: format!("{label} ›"), action: "settings-page".into(), value: page.into(), on: false }] }, fields: vec![], qr: vec![], qrnote: String::new(), local: false } }
    #[test] fn the_pages_menu_is_told_from_a_page_and_the_nav_keeps_group_order() {
        let menu = vec![row("Usage", "usage"), row("Appearance", "appearance"), row("Voice", "voice"), row("Agents", "agents")];
        match shown(&menu) { Shown::Menu(items) => { let nav = nav_order(&items); assert_eq!(nav.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["appearance", "agents", "voice", "usage"], "grouped, stable inside a group"); } s => panic!("{s:?}") }
        let mut back = row("‹ Settings", ""); back.buttons.push(Btn { label: "‹ Settings".into(), action: "settings-page".into(), value: String::new(), on: false });
        assert_eq!(shown(&[back, row("Theme", "")]), Shown::Page);
        assert_eq!(shown(&[row("Theme", ""), row("Language", "")]), Shown::Other);
        assert_eq!(shown(&[]), Shown::Other);
    }
    #[test] fn every_page_has_an_icon_in_the_set_and_the_search_filters_titles_by_substring() {
        let icons: Vec<String> = ["sun", "message-square", "cpu", "square-pen", "circuit-board", "shopping-cart", "link", "globe", "mic", "code", "clock", "gauge", "book", "file-text", "settings", "search", "x"].iter().map(|s| s.to_string()).collect();
        for g in GROUPS { for p in g { assert!(icons.contains(&nav_icon(p).to_string()), "{p}"); } }
        assert!(nav_matches("Appearance", "") && nav_matches("Appearance", " ear") && nav_matches("Appearance", "APP") && !nav_matches("Appearance", "zzz"));
        assert_eq!(group_of("zzz"), GROUPS.len());
    }
}

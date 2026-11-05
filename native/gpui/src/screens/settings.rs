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

pub struct FieldSlot { pub input: Entity<Composer>, pub vault: bool, pub hub: String, echo: Echo, pending: Option<String>, timer: bool }

impl FieldSlot { fn remember(&mut self, t: &str) { self.echo.remember(t); } }

/// One composer per field name. A form field's typing goes out as `bfield`, coalesced (the first change at once, the rest the newest after
/// 30 ms); the hub's value is taken only when it is a real change. A vault field holds its text here and sends nothing until Save.
pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let rows = parse(&this.data["settings"]);
    let want: Vec<Field> = rows.into_iter().flat_map(|r| r.fields).collect();
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
        cx.subscribe(&input, |_, _, _: &ComposerEvent, _| {}).detach();
        this.screens.fields.insert(f.name.clone(), FieldSlot { input, vault: f.vault, hub: f.value.clone(), echo: Echo::new(&f.value), pending: None, timer: false });
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

fn row_el(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, r: &Row, i: usize) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().gap_1().py_2().border_b_1().border_color(c("line"));
    if !r.label.is_empty() { col = col.child(div().font_weight(FontWeight::BOLD).text_size(px(13.)).child(r.label.clone())); }
    if !r.note.is_empty() { col = col.child(div().text_size(px(12.)).text_color(c("dim")).child(r.note.clone())); }
    for f in &r.fields {
        if let Some(slot) = fields.get(&f.name) { col = col.child(div().w_full().max_w(px(520.)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(slot.input.clone())); }
    }
    if !r.qr.is_empty() { col = col.child(qr(light, &r.qr)); if !r.qrnote.is_empty() { col = col.child(div().text_size(px(11.)).text_color(c("dim")).child(r.qrnote.clone())); } }
    if !r.buttons.is_empty() {
        col = col.child(div().flex().flex_wrap().gap_2().children(r.buttons.iter().enumerate().map(|(j, b)| {
            let b2 = b.clone();
            let d = div().id(SharedString::from(format!("set-{i}-{j}"))).px_3().py_1().text_size(px(12.)).cursor_pointer().border_1();
            let d = if b.on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
            d.child(b.label.clone()).on_click(cx.listener(move |this, _, _, cx| { if is_vault_save(&b2) { save_vault(this, &b2, cx); } else { let _ = this.tx.try_send(press(&b2)); } cx.notify(); })).into_any_element()
        }).collect::<Vec<_>>()));
    }
    col.into_any_element()
}

/// The overlay over the main area while the hub has settings open; None when it has not.
pub fn view(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, settings: &Value, focus: &FocusHandle) -> Option<AnyElement> {
    if !settings.is_object() { return None; }
    let c = |n: &str| theme::color(light, n);
    let rows = parse(settings);
    let mut els: Vec<AnyElement> = rows.iter().take(MAX_ROWS).enumerate().map(|(i, r)| row_el(cx, light, fields, r, i)).collect();
    if rows.len() > MAX_ROWS { els.push(div().py_2().text_color(c("dim")).child(format!("… {} more", rows.len() - MAX_ROWS)).into_any_element()); }
    Some(div().id("settings").absolute().inset_0().occlude().track_focus(focus).bg(c("bg")).flex().flex_col()
        .child(div().h(px(36.)).flex_shrink_0().px_3().flex().items_center().gap_3().border_b_1().border_color(c("line"))
            .child(div().flex_1().font_weight(FontWeight::BOLD).child("Settings"))
            .child(div().id("settings-close").px_2().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("text"))).child("Close")
                .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("flag".into(), "settings".into())); cx.notify(); }))))
        .child(div().id("settings-rows").flex_1().overflow_y_scroll().flex().justify_center().child(div().w_full().max_w(px(760.)).px_4().py_2().flex().flex_col().children(els)))
        .into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
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
}

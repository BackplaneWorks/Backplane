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
#[derive(Clone, Debug, PartialEq)]
pub struct Field { pub name: String, pub hint: String, pub secret: bool, pub value: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Row { pub label: String, pub note: String, pub buttons: Vec<Btn>, pub fields: Vec<Field>, pub qr: Vec<String>, pub qrnote: String, pub local: bool }

/// At most this many rows are drawn at once; the rest are said ("N more"), never hidden.
pub const MAX_ROWS: usize = 300;

pub fn parse(settings: &Value) -> Vec<Row> {
    settings["rows"].as_array().into_iter().flatten().map(|r| Row {
        label: string(r, "label"), note: string(r, "note"),
        buttons: r["buttons"].as_array().into_iter().flatten().map(|b| Btn { label: string(b, "label"), action: string(b, "action"), value: string(b, "value"), on: b["on"].as_bool().unwrap_or(false) }).collect(),
        fields: r["fields"].as_array().into_iter().flatten().map(|f| Field { name: string(f, "name"), hint: string(f, "hint"), secret: f["secret"].as_bool().unwrap_or(false), value: string(f, "value") }).collect(),
        qr: r["qr"].as_array().into_iter().flatten().filter_map(|l| l.as_str().map(str::to_string)).collect(),
        qrnote: string(r, "qrnote"), local: r["local"].as_bool().unwrap_or(false),
    }).collect()
}

/// What a typed change sends.
pub fn field_command(name: &str, text: &str) -> Command { Command::Action("bfield".into(), format!("{name}\u{1f}{text}")) }
pub fn press(b: &Btn) -> Command { Command::Action(b.action.clone(), b.value.clone()) }

/// One composer per field name; the hub's value is taken when it changes, the person's typing goes out as it happens.
pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let rows = parse(&this.data["settings"]);
    let want: Vec<Field> = rows.into_iter().flat_map(|r| r.fields).collect();
    this.screens.fields.retain(|n, _| want.iter().any(|f| f.name == *n));
    for f in want {
        match this.screens.fields.get_mut(&f.name) {
            Some(slot) => {
                if slot.hub != f.value {
                    slot.hub = f.value.clone(); slot.sent = f.value.clone();
                    let (input, v) = (slot.input.clone(), f.value.clone());
                    if input.read(cx).content != v { input.update(cx, |i, cx| i.set(v, cx)); }
                }
            }
            None => {
                let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
                let (hint, secret, v) = (f.hint.clone(), f.secret, f.value.clone());
                input.update(cx, |i, cx| { i.set_placeholder(&hint); i.masked = secret; i.set(v, cx); });
                let name = f.name.clone();
                cx.observe(&input, move |this, input, cx| {
                    let text = input.read(cx).content.to_string();
                    let Some(slot) = this.screens.fields.get_mut(&name) else { return };
                    if slot.sent == text { return; }
                    slot.sent = text.clone();
                    let _ = this.tx.try_send(field_command(&name, &text));
                }).detach();
                // Enter in a field is not a send: the row's own buttons act
                cx.subscribe(&input, |_, _, _: &ComposerEvent, _| {}).detach();
                this.screens.fields.insert(f.name.clone(), FieldSlot { input, hub: f.value.clone(), sent: f.value });
            }
        }
    }
}

pub struct FieldSlot { pub input: Entity<Composer>, pub hub: String, pub sent: String }

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
            d.child(b.label.clone()).on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(press(&b2)); cx.notify(); })).into_any_element()
        }).collect::<Vec<_>>()));
    }
    col.into_any_element()
}

/// The overlay over the main area while the hub has settings open; None when it has not.
pub fn view(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, settings: &Value) -> Option<AnyElement> {
    if !settings.is_object() { return None; }
    let c = |n: &str| theme::color(light, n);
    let rows = parse(settings);
    let mut els: Vec<AnyElement> = rows.iter().take(MAX_ROWS).enumerate().map(|(i, r)| row_el(cx, light, fields, r, i)).collect();
    if rows.len() > MAX_ROWS { els.push(div().py_2().text_color(c("dim")).child(format!("… {} more", rows.len() - MAX_ROWS)).into_any_element()); }
    Some(div().id("settings").absolute().inset_0().bg(c("bg")).flex().flex_col()
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
    #[test] fn rows_parse_with_buttons_fields_and_qr() {
        let r = parse(&j(r#"{"rows":[{"label":"Theme","note":"light or dark","buttons":[{"label":"Dark","action":"theme","value":"dark","on":true},{"label":"Light","action":"theme","value":"light","on":false}]},
            {"label":"Key","note":"","buttons":[],"fields":[{"name":"kval","hint":"value","secret":true,"value":""}]},
            {"label":"Pair","note":"","buttons":[],"qr":["101","010","101"],"qrnote":"scan"}]}"#));
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].buttons[0], Btn { label: "Dark".into(), action: "theme".into(), value: "dark".into(), on: true });
        assert!(r[1].fields[0].secret && r[1].fields[0].name == "kval"); assert_eq!(r[2].qr.len(), 3); assert_eq!(r[2].qrnote, "scan");
        assert!(parse(&Value::Null).is_empty());
    }
    #[test] fn a_button_press_is_its_action_and_a_field_change_is_bfield_name_unit_text() {
        match press(&Btn { label: "x".into(), action: "theme".into(), value: "dark".into(), on: false }) { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("theme", "dark")), c => panic!("{c:?}") }
        match field_command("kname", "OPENAI_API_KEY") { Command::Action(a, v) => { assert_eq!(a, "bfield"); assert_eq!(v, "kname\u{1f}OPENAI_API_KEY"); } c => panic!("{c:?}") }
    }
}

//! What sits over the composer for its own commands: the side question and its answer (`thread.btw`), and the masked box (`thread.secretBox`). The hub decides both; the secret's
//! value is the app's own (a vault field), never in the view, and leaves once at Save as `secret.set`.
use super::settings::{Btn, Field, FieldSlot};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

/// Draft.preview: a command of the composer is never drawn as a message. `words` is `thread.commands`, the hub's own table; the rule is the hub's `command_is`.
pub fn is_command(words: &Value, draft: &str) -> bool {
    let lead = draft.trim_start();
    words.as_array().into_iter().flatten().filter_map(|w| w["word"].as_str()).any(|w| lead.strip_prefix(w).map_or(false, |r| r.trim().is_empty() || r.starts_with([' ', '\n'])))
}

/// What the Send button says: the hub's word for this draft (Ask, Give, Save, Steer), Send otherwise; Stop has no button of its own here.
pub fn send_label(thread: &Value) -> String { match string(thread, "send") { s if s.is_empty() || thread["sendAct"] != "send" => "Send".into(), s => s } }

pub fn form_fields(data: &Value) -> Vec<Field> {
    let b = &data["thread"]["secretBox"];
    if !b.is_object() { return vec![] }
    let mut out = vec![];
    if b["fresh"] == true { out.push(Field { name: "sname".into(), hint: "Name".into(), secret: false, value: String::new(), vault: true }); }
    out.push(Field { name: "sval".into(), hint: "Paste the value".into(), secret: true, value: String::new(), vault: true });
    out
}

/// The side question and its answer, until ×.
pub fn btw(cx: &mut Context<Backplane>, light: bool, b: &Value) -> Option<AnyElement> {
    if !b.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    Some(div().id("btw-card").flex().flex_col().gap_1().px_3().py_2().mb_1().max_h(px(240.)).overflow_y_scroll().border_1().border_color(c("line")).bg(c("panel")).text_size(px(13.))
        .child(div().flex().items_center().gap_2()
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("dim")).child(format!("btw \u{b7} {}", string(b, "q"))))
            .child(div().id("btw-close").px_1().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("bad"))).child("\u{d7}")
                .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("btw-close".into(), String::new())); cx.notify(); }))))
        .child(div().child(string(b, "a"))).into_any_element())
}

fn btn(cx: &mut Context<Backplane>, light: bool, id: &'static str, label: &'static str, main: bool, f: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let d = crate::ui::with_rect(div().id(id), || format!("ui:{id}")).px_3().py_1().text_size(px(12.)).cursor_pointer().border_1();
    (if main { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")) }).child(label).on_click(cx.listener(move |this, _, _, cx| { f(this, cx); cx.notify(); }))
}

/// The masked box: `/secret` opened it here, or an agent asked for this secret.
pub fn secret(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, b: &Value) -> Option<AnyElement> {
    if !b.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    let (name, purpose, err, packed) = (string(b, "name"), string(b, "purpose"), string(b, "err"), string(b, "save"));
    let input = |n: &str| fields.get(n).map(|s| div().px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(s.input.clone()));
    let head = if name.is_empty() { "A secret for this chat. Agents use it by name and never see the value.".to_string() } else { format!("Secret: {name}. Agents use it by name and never see the value.") };
    let drop = name.clone();
    let mut row = div().flex().gap_2().child(btn(cx, light, "secret-save", "Save", true, move |this, cx| { super::settings::save_vault(this, &Btn { label: "Save".into(), action: "secret-save".into(), value: packed.clone(), on: true }, cx); }))
        .child(btn(cx, light, "secret-shut", "Cancel", false, |this, _| { let _ = this.tx.try_send(Command::Action("secret-shut".into(), String::new())); }));
    if b["ask"] == true { row = row.child(btn(cx, light, "secret-dismiss", "Not needed", false, move |this, _| { let _ = this.tx.try_send(Command::Action("secret-dismiss".into(), drop.clone())); })); }
    Some(div().id("secret-box").flex().flex_col().gap_2().px_3().py_2().mb_1().border_1().border_color(c("accent")).bg(c("panel")).text_size(px(13.))
        .child(div().child(head)).children((!purpose.is_empty()).then(|| div().text_color(c("dim")).child(purpose)))
        .children(if b["fresh"] == true { input("sname") } else { None }).children(input("sval"))
        .children((!err.is_empty()).then(|| div().text_color(c("bad")).child(err))).child(row).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    use serde_json::json;
    fn words() -> Value { json!([{"word":"/btw"},{"word":"/give"},{"word":"/secret"},{"word":"/intent"}]) }
    #[test] fn a_command_is_the_drafts_first_word() {
        for d in ["/btw why", "/btw", "  /give >x", "/secret\nname", "/intent  "] { assert!(is_command(&words(), d), "{d}"); }
        for d in ["hi /btw", "/btwx", "/compact", ""] { assert!(!is_command(&words(), d), "{d}"); }
        assert!(!is_command(&Value::Null, "/btw why"), "an older hub lists none: nothing changes");
    }
    #[test] fn the_button_says_the_hubs_word() {
        assert_eq!(send_label(&json!({"send":"Ask","sendAct":"send"})), "Ask"); assert_eq!(send_label(&json!({"send":"Stop","sendAct":"interrupt"})), "Send"); assert_eq!(send_label(&json!({})), "Send");
    }
    #[test] fn the_box_asks_a_name_only_when_it_has_none() {
        let names = |b: Value| form_fields(&json!({"thread":{"secretBox":b}})).iter().map(|f| (f.name.clone(), f.secret, f.vault)).collect::<Vec<_>>();
        assert_eq!(names(json!({"fresh":true})), [("sname".to_string(), false, true), ("sval".to_string(), true, true)]);
        assert_eq!(names(json!({"fresh":false})), [("sval".to_string(), true, true)], "every field of the box is the app's: nothing typed rides a view action");
        assert!(form_fields(&json!({"thread":{}})).is_empty());
    }
}

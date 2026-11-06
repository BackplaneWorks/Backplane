//! A thread's own actions, as the hub lists them: the header's tools (`thread.tools`: pin, settle, archive, fork, stop...) and its menu
//! (`thread.menu`: Diff, Terminal, Find file, Snooze with its choices, Delete...), plus the two questions the hub asks back, a rename
//! (`renaming`: a title box) and a delete (`deleting`: confirm). Every press is the action the item names; Delete is only asked
//! for, the hub then raises its own confirmation. Nothing here decides what a thread can do.
use crate::script::Ctl;
use super::settings::{Field, FieldSlot};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

const SEP: char = '\u{1f}';

#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub label: String, pub tip: String, pub action: String, pub value: String, pub on: bool, pub danger: bool, pub options: Vec<(String, String)> }

fn items(v: &Value) -> Vec<Item> {
    v.as_array().into_iter().flatten().map(|t| Item {
        label: string(t, "label"), tip: string(t, "tip"), action: string(t, "action"), value: string(t, "value"), on: t["on"].as_bool().unwrap_or(false), danger: t["danger"].as_bool().unwrap_or(false),
        options: t["options"].as_array().into_iter().flatten().map(|o| (string(o, "label"), string(o, "value"))).collect(),
    }).filter(|i| !i.action.is_empty() || !i.options.is_empty()).collect()
}
pub fn tools(thread: &Value) -> Vec<Item> { items(&thread["tools"]) }
pub fn menu(thread: &Value) -> Vec<Item> { items(&thread["menu"]) }

pub fn press(i: &Item) -> Command { Command::Action(i.action.clone(), i.value.clone()) }
/// A choice of an item with options (a snooze): the item's action with the choice's value.
pub fn choose(i: &Item, value: &str) -> Command { Command::Action(i.action.clone(), value.to_string()) }

/// The rename box's field (`@f.title`, typed as `bfield title`), while the hub has a rename open.
pub fn form_fields(data: &Value) -> Vec<Field> {
    let r = &data["renaming"];
    if r.is_object() { vec![Field { name: "title".into(), hint: "Title".into(), secret: false, value: string(r, "text"), vault: false }] } else { Vec::new() }
}
/// What a form or question asks the window to focus when it opens (see bots::focus_target).
pub fn focus_target(data: &Value) -> &'static str { if data["renaming"].is_object() { "title" } else { "" } }
/// Esc on a question: the action that drops it.
pub fn close_question(data: &Value) -> Option<Command> {
    if data["renaming"].is_object() { Some(Command::Action("rename-no".into(), String::new())) } else if data["deleting"].is_object() { Some(Command::Action("delete-no".into(), String::new())) } else { None }
}
pub fn rename_save(id: &str, text: &str) -> Option<Command> { let t = text.trim(); if t.is_empty() { None } else { Some(Command::Action("rename-save".into(), format!("{id}{SEP}{t}"))) } }

fn small(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, danger: bool, cmd: Command) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (a, v) = match cmd { Command::Action(a, v) => (a, v), _ => (String::new(), String::new()) };
    let d = div().id(SharedString::from(id)).px_2().h_full().flex().items_center().text_size(px(11.)).cursor_pointer().border_l_1().border_color(c("line")).hover(|s| s.bg(c("raise")));
    let d = if danger { d.text_color(c("bad")) } else if on { d.text_color(c("accent")) } else { d.text_color(c("dim")) };
    d.child(label).on_click(cx.listener(move |this, _, _, cx| { super::settings::flush_fields(this); if a == "fork" { this.arm_new(); } let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); })).into_any_element()
}

/// The header's strip: each tool. The rest of the thread's actions are in the header's right-click menu (`context_items`). Returned as elements for the header row's right side.
pub fn strip(cx: &mut Context<Backplane>, light: bool, thread: &Value) -> Vec<AnyElement> {
    if !thread.is_object() { return Vec::new(); }
    let mut out: Vec<AnyElement> = vec![div().flex_1().into_any_element()];
    for (i, t) in tools(thread).into_iter().enumerate() { out.push(small(cx, light, format!("tool-{i}"), t.label.clone(), t.on, t.danger, press(&t))); }
    out
}

/// The header's right-click menu: the hub's thread menu (Diff, Terminal, Find file, Snooze choices, Delete...) as context-menu items.
pub fn context_items(thread: &Value) -> Vec<super::ctxmenu::Item> {
    let mut v = Vec::new();
    for m in menu(thread) {
        if m.options.is_empty() { v.push(super::ctxmenu::Item { label: m.label, action: m.action, value: m.value, confirm: false }); }
        else { for (l, val) in m.options { v.push(super::ctxmenu::Item { label: format!("{} {}", m.label, l.to_lowercase()), action: m.action.clone(), value: val, confirm: false }); } }
    }
    v
}

/// A thread id as the sidebar's rows carry it (`<hub>|<id>`), found in the picture by its raw part; the value a row's own menu sends.
pub fn qualify(data: &Value, raw: &str) -> String {
    fn walk(v: &Value, raw: &str) -> Option<String> {
        match v {
            Value::Object(m) => { if let Some(Value::String(id)) = m.get("id") { if id.rsplit_once('|').map_or(false, |(_, r)| r == raw) { return Some(id.clone()); } } m.values().find_map(|x| walk(x, raw)) }
            Value::Array(a) => a.iter().find_map(|x| walk(x, raw)),
            _ => None,
        }
    }
    walk(&data["projects"], raw).unwrap_or_else(|| raw.to_string())
}

/// The controls this header and its questions draw, for the harness: (id, label, what a click does).
pub fn controls(data: &Value) -> Vec<(String, String, Ctl)> {
    let mut out = Vec::new();
    let thread = &data["thread"];
    for (i, t) in tools(thread).into_iter().enumerate() { out.push((format!("tool-{i}"), t.label.clone(), Ctl::Node(t.action.clone(), t.value.clone()))); }
    if data["renaming"].is_object() {
        out.push(("rename-yes".into(), "Save".into(), Ctl::RenameSave(string(&data["renaming"], "id"))));
        out.push(("rename-no".into(), "Cancel".into(), Ctl::Node("rename-no".into(), String::new())));
    }
    if data["deleting"].is_object() {
        out.push(("delete-yes".into(), "Delete".into(), Ctl::Node("row-delete".into(), string(&data["deleting"], "id"))));
        out.push(("delete-no".into(), "Cancel".into(), Ctl::Node("delete-no".into(), String::new())));
    }
    out
}

fn modal(light: bool, id: &'static str) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    div().id(id).absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().items_start().p_4().gap_3()
}

/// The hub's two questions over the main area: a rename (a title box, Save, Cancel) and a delete (what, Delete, Cancel).
pub fn questions(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, data: &Value) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let btn = |cx: &mut Context<Backplane>, id: &'static str, label: String, primary: bool, danger: bool, action: &'static str, value: String| -> AnyElement {
        let d = div().id(id).px_3().py_1().text_size(px(12.)).cursor_pointer().border_1();
        let d = if primary { d.bg(c(if danger { "bad" } else { "accent" })).text_color(c("on-accent")).border_color(c(if danger { "bad" } else { "accent" })) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
        d.child(label).on_click(cx.listener(move |this, _, _, cx| {
            super::settings::flush_fields(this);
            let v = if action == "rename-save" { // the box's own text, trimmed; nothing sent for a blank title
                let text = this.screens.fields.get("title").map(|s| s.input.read(cx).content.to_string()).unwrap_or_default();
                match rename_save(&value, &text) { Some(Command::Action(_, v)) => v, _ => return }
            } else { value.clone() };
            let _ = this.tx.try_send(Command::Action(action.into(), v)); cx.notify();
        })).into_any_element()
    };
    let r = &data["renaming"]; let d = &data["deleting"];
    if r.is_object() {
        let id = string(r, "id");
        return Some(modal(light, "rename-q").child(div().font_weight(FontWeight::BOLD).child(string(r, "title")))
            .children(fields.get("title").map(|s| div().w_full().max_w(px(520.)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(s.input.clone())))
            .child(div().flex().gap_2().child(btn(cx, "rename-no", string(r, "no"), false, false, "rename-no", String::new())).child(btn(cx, "rename-yes", string(r, "yes"), true, false, "rename-save", id))).into_any_element());
    }
    if d.is_object() {
        let id = string(d, "id");
        return Some(modal(light, "delete-q").child(div().font_weight(FontWeight::BOLD).child(string(d, "title"))).child(div().text_color(c("dim")).child(string(d, "body")))
            .child(div().flex().gap_2().child(btn(cx, "delete-no", string(d, "no"), false, false, "delete-no", String::new())).child(btn(cx, "delete-yes", string(d, "yes"), true, true, "row-delete", id))).into_any_element());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn the_hubs_tools_and_menu_parse_with_options_and_danger() {
        let t = j(r#"{"tools":[{"label":"Pin","tip":"t","action":"pin","value":"","on":false,"danger":false},{"label":"Archive","action":"archive","value":"","on":false,"danger":false}],
            "menu":[{"label":"Diff","action":"diff","value":"","options":[]},{"label":"Snooze","action":"snooze","value":"","options":[{"label":"1 hour","value":"60"},{"label":"Tomorrow","value":"tomorrow"}]},{"label":"Delete","action":"delete","value":"","danger":true,"options":[]}]}"#);
        assert_eq!(tools(&t).iter().map(|i| i.action.as_str()).collect::<Vec<_>>(), ["pin", "archive"]);
        let m = menu(&t); assert_eq!(m.len(), 3); assert_eq!(m[1].options, vec![("1 hour".into(), "60".into()), ("Tomorrow".into(), "tomorrow".into())]); assert!(m[2].danger);
        match choose(&m[1], "60") { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("snooze", "60")), c => panic!("{c:?}") }
        match press(&tools(&t)[0]) { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("pin", "")), c => panic!("{c:?}") }
        assert!(tools(&Value::Null).is_empty());
    }
    #[test] fn the_header_menu_lists_the_hubs_menu_with_snooze_choices() {
        let t = j(r#"{"menu":[{"label":"Diff","action":"diff","value":"","options":[]},{"label":"Snooze","action":"snooze","value":"","options":[{"label":"1 hour","value":"60"}]}]}"#);
        let it = context_items(&t); assert_eq!(it.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Diff", "Snooze 1 hour"]); assert_eq!(it[1].value, "60");
        assert!(context_items(&Value::Null).is_empty());
    }
    #[test] fn a_raw_id_is_qualified_from_the_sidebar_rows() {
        let d = j(r#"{"projects":[{"id":"h|p1","threads":[{"id":"h|t3"},{"id":"h|t1"}]}]}"#);
        assert_eq!(qualify(&d, "t3"), "h|t3"); assert_eq!(qualify(&d, "zz"), "zz");
    }
    #[test] fn delete_yes_sends_row_delete_with_the_asked_id() {
        let d = j(r#"{"thread":{"id":"t1"},"deleting":{"id":"t2"}}"#);
        let c = controls(&d);
        assert!(c.iter().any(|(id, _, ctl)| id == "delete-yes" && matches!(ctl, Ctl::Node(a, v) if a == "row-delete" && v == "t2")));
    }
    #[test] fn rename_and_delete_questions() {
        let d = j(r#"{"renaming":{"id":"t1","title":"Rename","text":"Old title","yes":"Rename","no":"Cancel"}}"#);
        assert_eq!(form_fields(&d)[0].value, "Old title"); assert_eq!(focus_target(&d), "title");
        assert!(matches!(close_question(&d), Some(Command::Action(a, _)) if a == "rename-no"));
        assert!(matches!(close_question(&j(r#"{"deleting":{"id":"t1"}}"#)), Some(Command::Action(a, _)) if a == "delete-no")); assert!(close_question(&Value::Null).is_none());
        match rename_save("t1", "  New name ").unwrap() { Command::Action(a, v) => { assert_eq!(a, "rename-save"); assert_eq!(v, "t1\u{1f}New name"); } c => panic!("{c:?}") }
        assert!(rename_save("t1", "   ").is_none(), "a blank title sends nothing");
    }
}

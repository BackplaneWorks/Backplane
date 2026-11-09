//! A thread's own actions, as the hub lists them: the header's tools (`thread.tools`: pin, settle, archive, fork, stop...) and its menu
//! (`thread.menu`: Diff, Terminal, Find file, Snooze with its choices, Delete...), plus the two questions the hub asks back, a rename
//! (`renaming`: a title box, drawn in the sidebar row: siderow.rs) and a delete (`deleting`: armed by the menu's first click, confirmed by its
//! second: ctxmenu.rs). Every press is the action the item names. Nothing here decides what a thread can do.
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

/// A thread tool whose answer is a new thread the window goes to (Fork thread, Buy it): the window arms for it before sending, as New thread does.
pub fn makes_thread(action: &str) -> bool { matches!(action, "fork" | "fab-buy") }
fn small(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, danger: bool, cmd: Command, tip: (String, String)) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (a, v) = match cmd { Command::Action(a, v) => (a, v), _ => (String::new(), String::new()) };
    let rect = format!("tool:{a}"); // a header tool reports its rectangle (tool:<action>), so a run can press Buy it as a person would
    let d = crate::ui::with_rect(div().id(SharedString::from(id)), move || rect).px_2().h_full().flex().items_center().text_size(px(11.)).cursor_pointer().border_l_1().border_color(c("line")).hover(|s| s.bg(c("raise")));
    let d = if danger { d.text_color(c("bad")) } else if on { d.text_color(c("accent")) } else { d.text_color(c("dim")) };
    d.child(label).tooltip(super::super::keys::tip(&tip.0, &tip.1, light)).on_click(cx.listener(move |this, _, _, cx| { super::settings::flush_fields(this); if makes_thread(&a) { this.arm_new(); } let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); })).into_any_element()
}

/// The header's strip: each tool. The rest of the thread's actions are in the header's right-click menu (`context_items`). Returned as elements for the header row's right side.
pub fn strip(cx: &mut Context<Backplane>, light: bool, thread: &Value, local: &crate::local::Local) -> Vec<AnyElement> {
    if !thread.is_object() { return Vec::new(); }
    let mut out: Vec<AnyElement> = vec![div().flex_1().into_any_element()];
    for (i, t) in tools(thread).into_iter().enumerate() { let k = if t.tip.contains("Ctrl+") { String::new() } else { super::super::keys::hint(local, super::super::keys::id_for(&t.action, &t.value, "")) }; out.push(small(cx, light, format!("tool-{i}"), t.label.clone(), t.on, t.danger, press(&t), (if t.tip.is_empty() { t.label.clone() } else { t.tip.clone() }, k))); }
    out
}

/// The header's right-click menu: the hub's thread menu (Diff, Terminal, Find file, Snooze choices, Delete...) as context-menu items.
pub fn context_items(thread: &Value) -> Vec<super::ctxmenu::Item> {
    let mut v = Vec::new();
    for m in menu(thread) {
        // a dangerous item (Delete) asks in the menu itself: the first click arms it, the second does it
        if m.options.is_empty() { v.push(super::ctxmenu::Item { label: m.label, action: m.action, value: m.value, confirm: m.danger }); }
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

/// The hub's rename, when no sidebar row can show it: a small bar over the top of the main column (siderow::bar). The thread's Delete asks in
/// its menu (ctxmenu), so nothing is drawn for `deleting`: neither question covers the window as a page.
pub fn questions(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, data: &Value, sidebar_open: bool) -> Option<AnyElement> {
    super::siderow::bar(cx, light, fields, data, sidebar_open)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn buy_it_and_fork_make_a_thread_the_window_goes_to() { assert!(makes_thread("fab-buy") && makes_thread("fork")); assert!(!makes_thread("pin") && !makes_thread("settle")); }
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

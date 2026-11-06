//! The sidebar's project search and Add project. Search: a field above the list; every change goes out as the view action `proj-find-q`
//! (the hub filters its rows), Enter is `proj-go` (the first project shown), and the hub's own clear comes back into the field. Add project:
//! the hub's folder picker (`folders`: a path field, an error, rows that each name their action), opened by `picker-open`, typed with
//! `picker-type`, left with `proj-close` (Esc too). The hub decides every row and its action; this only draws and sends.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, script::Ctl, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Default)]
pub struct Projects {
    pub search: Option<Entity<Composer>>, pub folder: Option<Entity<Composer>>,
    sent_q: Vec<String>, sent_f: Vec<String>, echo_q: Option<String>, echo_f: Option<String>,
    /// Set when a box wants the keys on the next frame: "search" or "folder".
    pub focus: Option<&'static str>, folder_open: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub label: String, pub action: String, pub value: String, pub kind: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Folders { pub text: String, pub hint: String, pub error: String, pub items: Vec<Item> }

pub fn folders(data: &Value) -> Option<Folders> {
    let f = data["folders"].as_object()?;
    let s = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let items = f.get("items").and_then(Value::as_array).into_iter().flatten().take(200).map(|i| Item { label: string(i, "label"), action: string(i, "action"), value: string(i, "value"), kind: string(i, "kind") }).collect();
    Some(Folders { text: s("text"), hint: s("hint"), error: s("error"), items })
}
pub fn query(data: &Value) -> String { string(&data["search"], "query") }
pub fn hint(data: &Value) -> String { let h = string(&data["search"], "hint"); if h.is_empty() { "Search projects".into() } else { h } }

pub fn search_cmd(text: &str) -> Command { Command::Action("proj-find-q".into(), text.to_string()) }
pub fn go_cmd() -> Command { Command::Action("proj-go".into(), String::new()) }
pub fn open_cmd() -> Command { Command::Action("picker-open".into(), String::new()) }
pub fn type_cmd(text: &str) -> Command { Command::Action("picker-type".into(), text.to_string()) }
pub fn close_cmd() -> Command { Command::Action("proj-close".into(), String::new()) }
/// Esc leaves the folder picker while it is open.
pub fn close_on_escape(data: &Value) -> Option<Command> { data["folders"].is_object().then(close_cmd) }
/// Whether the hub's text replaces what is in the box: only a real change that is not one of the last texts this box sent.
pub fn adopt(hub: &str, typed: &str, sent: &[String]) -> bool { hub != typed && !sent.iter().any(|s| s == hub) }
fn remember(v: &mut Vec<String>, t: &str) { v.push(t.to_string()); if v.len() > 16 { v.remove(0); } }

pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.projects.search.is_none() {
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        cx.observe(&input, |this, input, cx| {
            let text = input.read(cx).content.to_string(); let p = &mut this.screens.projects;
            if p.echo_q.as_deref() == Some(text.as_str()) { p.echo_q = None; return; }
            if p.sent_q.last() == Some(&text) { return; }
            remember(&mut p.sent_q, &text); let _ = this.tx.try_send(search_cmd(&text));
        }).detach();
        cx.subscribe(&input, |this, _, ev: &ComposerEvent, _| if matches!(ev, ComposerEvent::Submit) { let _ = this.tx.try_send(go_cmd()); }).detach();
        this.screens.projects.search = Some(input);
    }
    let (q, h) = (query(&this.data), hint(&this.data));
    if let Some(input) = this.screens.projects.search.clone() {
        let typed = input.read(cx).content.to_string();
        input.update(cx, |i, _| i.set_placeholder(&h));
        if adopt(&q, &typed, &this.screens.projects.sent_q) { this.screens.projects.echo_q = Some(q.clone()); input.update(cx, |i, cx| i.set(q, cx)); }
    }
    match folders(&this.data) {
        None => { this.screens.projects.folder_open = false; this.screens.projects.sent_f.clear(); }
        Some(f) => {
            if this.screens.projects.folder.is_none() {
                let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
                cx.observe(&input, |this, input, cx| {
                    let text = input.read(cx).content.to_string(); let p = &mut this.screens.projects;
                    if p.echo_f.as_deref() == Some(text.as_str()) { p.echo_f = None; return; }
                    if p.sent_f.last() == Some(&text) { return; }
                    remember(&mut p.sent_f, &text); let _ = this.tx.try_send(type_cmd(&text));
                }).detach();
                this.screens.projects.folder = Some(input);
            }
            if !this.screens.projects.folder_open { this.screens.projects.folder_open = true; this.screens.projects.focus = Some("folder"); }
            if let Some(input) = this.screens.projects.folder.clone() {
                let typed = input.read(cx).content.to_string();
                input.update(cx, |i, _| i.set_placeholder(&f.hint));
                if adopt(&f.text, &typed, &this.screens.projects.sent_f) { this.screens.projects.echo_f = Some(f.text.clone()); input.update(cx, |i, cx| i.set(f.text.clone(), cx)); }
            }
        }
    }
}

/// The sidebar's search box (an empty box before the first picture).
pub fn search_box(light: bool, p: &Projects) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().flex_1().min_w_0().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).children(p.search.clone()).into_any_element()
}

/// The folder picker over the main area, while the hub shows `folders`.
pub fn view(cx: &mut Context<Backplane>, light: bool, p: &Projects, data: &Value) -> Option<AnyElement> {
    let f = folders(data)?;
    let c = |n: &str| theme::color(light, n);
    let mut list = div().id("folders-list").flex().flex_col().w_full().max_w(px(640.)).flex_1().min_h_0().overflow_y_scroll();
    for (i, it) in f.items.iter().enumerate() {
        let (a, v) = (it.action.clone(), it.value.clone()); let live = !a.is_empty();
        let icon = match it.kind.as_str() { "up" => "↰", "dir" => "▸", "add" => "+", _ => " " };
        list = list.child(div().id(SharedString::from(format!("folder-{i}"))).flex().gap_2().px_2().py_1().text_size(px(12.)).text_color(c(if live { "text" } else { "faint" })).when(live, |d| d.cursor_pointer().hover(|s| s.bg(c("raise"))))
            .child(div().w(px(14.)).text_color(c("dim")).child(icon)).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(it.label.clone()))
            .on_click(cx.listener(move |this, _, _, cx| { if !a.is_empty() { if a == "proj-add" || a == "proj-new" { this.arm_new(); } let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); } cx.notify(); })));
    }
    Some(div().id("folders-q").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().items_start().p_4().gap_3()
        .child(div().font_weight(FontWeight::BOLD).child("Add project"))
        .children(p.folder.clone().map(|i| div().w_full().max_w(px(640.)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(i)))
        .when(!f.error.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(c("bad")).child(f.error.clone())))
        .child(list)
        .child(div().id("folders-cancel").px_3().py_1().text_size(px(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).child("Cancel").on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(close_cmd()); cx.notify(); })))
        .into_any_element())
}

/// The controls these draw, for the harness.
pub fn controls(data: &Value) -> Vec<(String, String, Ctl)> {
    let mut out = vec![("ui:Add project".to_string(), "Add project".to_string(), Ctl::Node("picker-open".into(), String::new()))];
    if let Some(f) = folders(data) {
        for (i, it) in f.items.iter().enumerate() { if !it.action.is_empty() { out.push((format!("folder-{i}"), it.label.clone(), Ctl::Node(it.action.clone(), it.value.clone()))); } }
        out.push(("folders-cancel".into(), "Cancel".into(), Ctl::Node("proj-close".into(), String::new())));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn the_hubs_folder_picker_parses_and_closes_on_escape() {
        let d = j(r#"{"folders":{"text":"~/Doc","hint":"Path","error":"","items":[{"label":"Documents","action":"proj-cd","value":"~/Documents/","kind":"dir"},{"label":"Add ~/Doc","action":"proj-add","value":"~/Doc","kind":"add"}]}}"#);
        let f = folders(&d).unwrap(); assert_eq!(f.items.len(), 2); assert_eq!(f.items[1].action, "proj-add"); assert!(matches!(close_on_escape(&d), Some(Command::Action(a, _)) if a == "proj-close"));
        assert!(folders(&Value::Null).is_none() && close_on_escape(&Value::Null).is_none());
        assert_eq!(controls(&d).iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["ui:Add project", "folder-0", "folder-1", "folders-cancel"]);
    }
    #[test] fn the_hubs_text_replaces_a_box_only_when_it_is_news() {
        let sent = vec!["a".to_string(), "ab".to_string()];
        assert!(!adopt("ab", "abc", &sent)); assert!(!adopt("abc", "abc", &sent)); assert!(adopt("", "abc", &sent)); assert!(adopt("zz", "abc", &sent));
    }
    #[test] fn the_search_speaks_the_clients_actions() {
        assert!(matches!(search_cmd("ecc"), Command::Action(a, v) if a == "proj-find-q" && v == "ecc")); assert!(matches!(go_cmd(), Command::Action(a, _) if a == "proj-go"));
        assert!(matches!(open_cmd(), Command::Action(a, _) if a == "picker-open")); assert!(matches!(type_cmd("~/"), Command::Action(a, v) if a == "picker-type" && v == "~/"));
        assert_eq!(hint(&Value::Null), "Search projects"); assert_eq!(query(&j(r#"{"search":{"query":"x"}}"#)), "x");
    }
}

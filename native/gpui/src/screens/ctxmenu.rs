//! The right-click menu on the sidebar's rows: a thread or subthread (Pin, Settle, Snooze, New subthread, Rename, Archive, Delete) and a project (New thread, Remove). Each item is a
//! hub action the hub already answers (commands/actions.rs); Delete and Remove ask twice, the second click does it. The menu opens where the mouse was pressed.
use crate::{hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub label: String, pub action: String, pub value: String, pub confirm: bool }
#[derive(Clone, Debug)]
pub struct Menu { pub at: Point<Pixels>, pub items: Vec<Item>, /// the item whose confirming click is awaited
    pub armed: Option<usize> }

fn s(v: &Value, k: &str) -> String { v[k].as_str().unwrap_or("").to_string() }
fn raw(id: &str) -> &str { id.rsplit('|').next().unwrap_or(id) }
fn item(label: &str, action: &str, value: &str, confirm: bool) -> Item { Item { label: label.into(), action: action.into(), value: value.into(), confirm } }

/// The items for a thread row of the sidebar (`t` is its row, `project` the raw id of the project it is in).
pub fn thread_items(t: &Value, project: &str) -> Vec<Item> {
    let qid = s(t, "id");
    let mut v = vec![];
    v.push(if t["pinned"].as_bool().unwrap_or(false) { item("Unpin", "row-unpin", &qid, false) } else { item("Pin", "row-pin", &qid, false) });
    if let Some(l) = t["lead"].as_array().and_then(|l| l.first()) { v.push(item(&s(l, "label"), &s(l, "action"), &s(l, "value"), false)); }
    for o in t["trail"].as_array().into_iter().flatten().flat_map(|x| x["options"].as_array().into_iter().flatten()) { v.push(item(&format!("Snooze {}", s(o, "label").to_lowercase()), "row-snooze", &s(o, "value"), false)); }
    if t["canChild"].as_bool().unwrap_or(true) { v.push(item("New subthread", "thread-child", &format!("{}|{}", raw(&qid), project), false)); }
    v.push(item("Rename", "row-rename", raw(&qid), false));
    v.push(item("Archive", "row-archive", &qid, false));
    v.push(item("Delete", "row-delete", &qid, true));
    v
}
/// The items for a project's header.
pub fn project_items(pid_raw: &str) -> Vec<Item> { vec![item("New thread", "new-thread", pid_raw, false), item("Remove project", "proj-remove", pid_raw, true)] }

impl Backplane {
    pub(crate) fn ctx_open(&mut self, at: Point<Pixels>, items: Vec<Item>, cx: &mut Context<Self>) { self.screens.ctx = Some(Menu { at, items, armed: None }); cx.notify(); }
    /// Esc or a click outside: true when there was a menu to close.
    pub(crate) fn ctx_close(&mut self, cx: &mut Context<Self>) -> bool { let was = self.screens.ctx.take().is_some(); if was { cx.notify(); } was }
    fn ctx_click(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(m) = self.screens.ctx.as_mut() else { return };
        let Some(it) = m.items.get(i).cloned() else { return };
        if it.confirm && m.armed != Some(i) {
            m.armed = Some(i);
            // a thread's Delete arms the hub's own confirm with the first press, as the other clients do
            if it.action == "row-delete" { let _ = self.tx.try_send(Command::Action(it.action, it.value)); }
            cx.notify(); return;
        }
        self.screens.ctx = None;
        let _ = self.tx.try_send(Command::Action(it.action, it.value));
        cx.notify();
    }
}

/// The menu over everything, with a backdrop that closes it.
pub fn render(cx: &mut Context<Backplane>, light: bool, m: &Menu) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut list = div().absolute().left(m.at.x).top(m.at.y).min_w(px(190.)).flex().flex_col().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).occlude();
    for (i, it) in m.items.iter().enumerate() {
        let armed = m.armed == Some(i);
        let label = if armed { format!("Click again to {}", it.label.to_lowercase()) } else { it.label.clone() };
        let danger = it.confirm;
        list = list.child(div().id(SharedString::from(format!("ctx-{i}"))).px_3().py_1().cursor_pointer().text_color(if armed || danger { c("bad") } else { c("text") })
            .when(armed, |d| d.bg(c("raise")).font_weight(FontWeight::BOLD)).hover(|s| s.bg(c("raise"))).child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.ctx_click(i, cx))));
    }
    div().id("ctx-backdrop").absolute().inset_0().occlude()
        .on_click(cx.listener(|this, _, _, cx| { this.ctx_close(cx); }))
        .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| { this.ctx_close(cx); }))
        .child(list).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn a_thread_row_offers_what_the_hub_does() {
        let t = j(r#"{"id":"http://h:1|t7","pinned":false,"canChild":true,"lead":[{"label":"Settle","action":"row-settle","value":"http://h:1|t7"}],"trail":[{"label":"Delete"},{"label":"Snooze","options":[{"label":"1 hour","value":"http://h:1|t7|1h"}]}]}"#);
        let it = thread_items(&t, "p1");
        let names: Vec<&str> = it.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(names, ["Pin", "Settle", "Snooze 1 hour", "New subthread", "Rename", "Archive", "Delete"]);
        assert_eq!(it[3].value, "t7|p1"); assert_eq!(it[4].value, "t7"); assert!(it.last().unwrap().confirm && !it[0].confirm);
        let pinned = j(r#"{"id":"t1","pinned":true}"#); assert_eq!(thread_items(&pinned, "p")[0].action, "row-unpin");
    }
    #[test] fn a_project_offers_new_thread_and_a_confirmed_remove() { let it = project_items("p1"); assert_eq!((it[0].action.as_str(), it[1].action.as_str(), it[1].confirm), ("new-thread", "proj-remove", true)); }
}

//! "Jump or run a command": one box over the window that finds a thread in any project, or an action, as you type, and does it on Enter
//! or a click. Threads come from the sidebar's rows (`projects`, or `active`), the actions are the window's and the open thread's own (its header tools, the viewer
//! kinds, Settings...). Opened from the bar at the top, the Commands item on the rail or Ctrl+Shift+P; Esc shuts it. The ranking is `rank`: what starts with the text,
//! then a word that does, then the letters in order.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Do { Thread(String), Action(String, String), Settings, Sidebar, Models, Prev, Next, Uber, Shortcuts, Hub(String) }
#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub label: String, pub place: String, pub kind: &'static str, pub what: Do }
/// The key that does what an item does, as the person reads it ("" for none).
pub fn shortcut(it: &Item, current: &str, l: &crate::local::Local) -> String {
    let id = match &it.what {
        Do::Action(a, v) if a == "view-kind" && v != "files" => return crate::keys::rail_letter(v).to_uppercase(),
        Do::Action(a, v) => crate::keys::id_for(a, v, current),
        Do::Sidebar => "sidebar", Do::Models => "models", Do::Prev => "prev", Do::Next => "next", Do::Uber => "uber", _ => "",
    };
    if id.is_empty() { String::new() } else { crate::keys::hint(l, id) }
}

#[derive(Default)]
pub struct Palette { pub focus: bool, pub open: bool, pub input: Option<Entity<Composer>>, pub q: String, pub ix: usize }

/// 0: no match; a prefix 3, a word that starts with it 2, the letters in order 1 (an empty text matches everything a little).
pub fn rank(label: &str, q: &str) -> u8 {
    let (l, q) = (label.to_lowercase(), q.trim().to_lowercase());
    if q.is_empty() { return 1 }
    if l.starts_with(&q) { return 3 }
    if l.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&q)) { return 2 }
    let mut it = l.chars(); if q.chars().all(|c| it.any(|x| x == c)) { 1 } else { 0 }
}

/// Every thing the box can offer now (before the text filters it).
pub fn items(data: &Value) -> Vec<Item> {
    let mut v: Vec<Item> = vec![];
    let thread = &data["thread"];
    if thread.is_object() {
        for t in thread["tools"].as_array().into_iter().flatten().chain(thread["menu"].as_array().into_iter().flatten()) {
            let (label, action, value) = (string(t, "label"), string(t, "action"), string(t, "value"));
            if label.is_empty() || action.is_empty() || action == "delete" || t["options"].as_array().map_or(false, |o| !o.is_empty()) { continue }
            v.push(Item { label, place: String::new(), kind: "action", what: Do::Action(action, value) });
        }
        for (l, k) in [("Board", "board"), ("Schematic", "schematic"), ("3D", "3d"), ("Split", "split"), ("Mech", "mech"), ("Files", "files"), ("Review", "review"), ("Browser", "browser")] {
            v.push(Item { label: format!("Open {l}"), place: "viewer".into(), kind: "action", what: Do::Action("view-kind".into(), k.into()) });
        }
    }
    if thread.is_object() {
        v.push(Item { label: "Agent, model and effort".into(), place: String::new(), kind: "action", what: Do::Models });
        v.push(Item { label: "Previous thread".into(), place: String::new(), kind: "action", what: Do::Prev });
        v.push(Item { label: "Next thread".into(), place: String::new(), kind: "action", what: Do::Next });
    }
    if data["bots"].as_array().map_or(false, |b| b.iter().any(|b| b["uber"] == true)) { v.push(Item { label: "Uberbot chat".into(), place: String::new(), kind: "action", what: Do::Uber }); }
    v.push(Item { label: "Keyboard shortcuts".into(), place: String::new(), kind: "action", what: Do::Shortcuts });
    v.push(Item { label: "Settings".into(), place: String::new(), kind: "action", what: Do::Settings });
    v.push(Item { label: "Fold or unfold the sidebar".into(), place: String::new(), kind: "action", what: Do::Sidebar });
    // always here, whatever the hub sends or fails to send: these do not go through it (hubbar.rs)
    for a in ["restart", "safe", "rollback"] { v.push(Item { label: super::hubbar::label_of(a).into(), place: String::new(), kind: "action", what: Do::Hub(a.into()) }); }
    for p in data["projects"].as_array().into_iter().flatten() {
        let (pid, ptitle) = (string(p, "id"), string(p, "title"));
        let raw = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
        v.push(Item { label: format!("New thread in {ptitle}"), place: String::new(), kind: "action", what: Do::Action("new-thread".into(), raw) });
        for t in p["threads"].as_array().into_iter().flatten().chain(p["settled"].as_array().into_iter().flatten()) {
            v.push(Item { label: string(t, "title"), place: ptitle.clone(), kind: "thread", what: Do::Thread(string(t, "id")) });
        }
    }
    for t in data["active"].as_array().into_iter().flatten().chain(data["activeSettled"]["rows"].as_array().into_iter().flatten()) {
        v.push(Item { label: string(t, "title"), place: string(t, "project"), kind: "thread", what: Do::Thread(string(t, "id")) });
    }
    v
}

/// The items the text keeps, best first (threads before actions among equals), at most `n`.
pub fn found(all: Vec<Item>, q: &str, n: usize) -> Vec<Item> {
    let mut r: Vec<(u8, usize, Item)> = all.into_iter().enumerate().filter_map(|(i, it)| { let s = rank(&it.label, q).max(if it.place.is_empty() { 0 } else { rank(&it.place, q).min(1) }); (s > 0).then_some((s, i, it)) }).collect();
    r.sort_by(|a, b| b.0.cmp(&a.0).then((a.2.kind != "thread").cmp(&(b.2.kind != "thread"))).then(a.1.cmp(&b.1)));
    let mut seen = std::collections::HashSet::new();
    r.into_iter().map(|x| x.2).filter(|it| seen.insert((it.label.clone(), it.place.clone(), it.kind))).take(n).collect()
}

impl Backplane {
    pub(crate) fn palette_open(&mut self, cx: &mut Context<Self>) {
        if self.screens.palette.input.is_none() {
            let input = cx.new(|cx| Composer::with_context(cx, "PickInput"));
            input.update(cx, |i, _| { i.set_placeholder("Jump to a thread or run a command"); });
            cx.observe(&input, |this, input, cx| { let q = input.read(cx).content.to_string(); if q != this.screens.palette.q { this.screens.palette.q = q; this.screens.palette.ix = 0; } cx.notify(); }).detach();
            cx.subscribe(&input, |this, _, ev: &ComposerEvent, cx| match ev {
                ComposerEvent::Submit | ComposerEvent::PopupPick => this.palette_run(None, cx),
                ComposerEvent::PopupMove(d) => { let n = found(items(&this.data), &this.screens.palette.q, 10).len().max(1); this.screens.palette.ix = (this.screens.palette.ix as i32 + d).rem_euclid(n as i32) as usize; cx.notify(); }
                _ => {}
            }).detach();
            self.screens.palette.input = Some(input);
        }
        let input = self.screens.palette.input.clone().unwrap();
        input.update(cx, |i, cx| { i.set(String::new(), cx); i.popup_open = true; });
        self.screens.palette.q.clear(); self.screens.palette.ix = 0; self.screens.palette.open = true;
        self.screens.palette.focus = true; cx.notify();
    }
    pub(crate) fn palette_close(&mut self, cx: &mut Context<Self>) -> bool {
        let was = self.screens.palette.open; self.screens.palette.open = false;
        if was { self.want_focus = true; cx.notify(); } was
    }
    /// Do the highlighted item (or the one at `i`) and shut the box.
    fn palette_run(&mut self, i: Option<usize>, cx: &mut Context<Self>) {
        let list = found(items(&self.data), &self.screens.palette.q, 10);
        let Some(it) = list.get(i.unwrap_or(self.screens.palette.ix)).cloned() else { return };
        self.screens.palette.open = false; self.want_focus = true;
        match it.what {
            Do::Thread(id) => { let _ = self.tx.try_send(Command::Action("select".into(), id)); }
            Do::Action(a, v) => { if super::tools::makes_thread(&a) { self.arm_new(); } let _ = self.tx.try_send(Command::Action(a, v)); }
            Do::Settings => self.open_settings(cx),
            Do::Sidebar => self.sidebar_toggle(cx),
            Do::Models => self.models_pending = true,
            Do::Prev | Do::Next => { let n = crate::keys::neighbour(&self.data, &self.selected, it.what == Do::Next); if let Some(id) = n { self.select_id(id, cx); } }
            Do::Uber => { let id = self.data["bots"].as_array().into_iter().flatten().find(|b| b["uber"] == true).map(|b| crate::string(b, "id")); if let Some(id) = id { let _ = self.tx.try_send(Command::Action("bot".into(), id)); } }
            Do::Shortcuts => self.open_settings(cx),
            Do::Hub(a) => self.hub_click(&a, cx),
        }
        cx.notify();
    }
}

pub fn view(cx: &mut Context<Backplane>, light: bool, data: &Value, p: &Palette, l: &crate::local::Local, sel: &str) -> Option<AnyElement> {
    if !p.open { return None }
    let c = |n: &str| theme::color(light, n);
    let list = found(items(data), &p.q, 10);
    let ix = p.ix.min(list.len().saturating_sub(1));
    let current = crate::keys::project_of(data, sel);
    let mut col = div().flex().flex_col().py_1();
    if list.is_empty() { col = col.child(div().px_3().py_1().text_color(c("faint")).child("Nothing matches")); }
    for (i, it) in list.iter().enumerate() {
        col = col.child(div().id(SharedString::from(format!("pal-{i}"))).flex().items_center().gap_3().px_3().py_1().cursor_pointer().when(i == ix, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(it.label.clone()))
            .child(div().text_size(px(11.)).text_color(c("dim")).child(shortcut(it, &current, l)))
            .child(div().text_size(px(11.)).text_color(c("faint")).child(if it.place.is_empty() { it.kind.to_string() } else { it.place.clone() }))
            .on_click(cx.listener(move |this, _, _, cx| this.palette_run(Some(i), cx))));
    }
    let input = p.input.clone().map(|s| div().px_3().py_2().border_b_1().border_color(c("line")).child(s));
    Some(div().id("palette").absolute().inset_0().occlude().flex().justify_center().pt(px(90.)).bg(gpui::hsla(0., 0., 0., 0.4))
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.palette_close(cx); }))
        .child(div().id("palette-box").w(px(560.)).max_w_full().h_auto().max_h(px(420.)).flex().flex_col().border_1().border_color(c("line")).bg(c("panel")).text_size(px(13.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).children(input).child(col)).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use core::prelude::v1::test;
    fn data() -> Value { json!({"thread": {"tools": [{"label": "Pin", "action": "pin"}, {"label": "Delete", "action": "delete"}], "menu": []},
        "projects": [{"id": "h|p1", "title": "Demo", "threads": [{"id": "h|t1", "title": "Board layout review"}, {"id": "h|t2", "title": "Power budget"}], "settled": [{"id": "h|t3", "title": "Old notes"}]}]}) }
    #[test] fn a_prefix_beats_a_word_beats_letters_in_order() { assert_eq!((rank("Board layout", "boa"), rank("Board layout", "lay"), rank("Board layout", "blt"), rank("Board layout", "zz"), rank("x", "")), (3, 2, 1, 0, 1)); }
    #[test] fn threads_and_actions_are_found_by_what_is_typed_and_a_delete_is_never_offered() {
        let all = items(&data());
        assert!(all.iter().any(|i| i.what == Do::Thread("h|t3".into())) && all.iter().any(|i| i.what == Do::Action("pin".into(), String::new())) && all.iter().any(|i| i.what == Do::Settings));
        assert!(!all.iter().any(|i| i.what == Do::Action("delete".into(), String::new())), "Delete asks first and is not a palette action");
        let r = found(all.clone(), "pow", 10); assert_eq!(r[0].label, "Power budget");
        assert!(found(all.clone(), "zzzz", 10).is_empty());
        let r = found(all, "new thr", 10); assert_eq!(r[0].what, Do::Action("new-thread".into(), "p1".into()), "the project's raw id");
    }
    #[test] fn the_list_is_bounded_and_each_thing_is_listed_once() {
        let mut d = data(); d["active"] = json!([{"id": "h|t1", "title": "Board layout review", "project": "Demo"}]);
        let r = found(items(&d), "", 100); assert_eq!(r.iter().filter(|i| i.label == "Board layout review").count(), 1, "the same thread from two lists shows once");
        assert!(found(items(&d), "", 3).len() == 3);
    }
}

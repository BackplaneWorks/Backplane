//! The right-click menu on the sidebar's rows: a thread or subthread (Pin, Settle, Snooze, New subthread, Rename, Archive, Delete) and a project (New thread, Remove). Each item is a
//! hub action the hub already answers (commands/actions.rs); Delete and Remove ask in the menu itself, as 0.14 did (law menu_delete_asks): the first
//! click turns the item into "Click again to delete", the second does it; nothing is drawn over the window. A thread's Delete arms the hub's own
//! confirm with the first click (`@del`), and a menu shut with it armed disarms it (`delete-no`). The menu opens where the mouse was pressed.
use crate::{hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub label: String, pub action: String, pub value: String, pub confirm: bool }
#[derive(Clone, Debug)]
pub struct Menu { pub at: Point<Pixels>, pub items: Vec<Item>, /// the item whose confirming click is awaited
    pub armed: Option<usize> }

/// The ▾ menu's "Find in thread": the window's own (the find bar over that thread, find.rs).
pub const LOCAL_SEARCH: &str = "local:search-thread";
/// A project's "Intent…" (0.14): its page, which shows what the project is for and edits it. The window's own, nothing sent.
pub const LOCAL_INTENT: &str = "local:intent";
/// A thread's "Move under…": the window's own second menu, the threads it may go under (`parent_choices`); each sends `row-reparent`.
pub const LOCAL_MOVE: &str = "local:move";
fn s(v: &Value, k: &str) -> String { v[k].as_str().unwrap_or("").to_string() }
fn raw(id: &str) -> &str { id.rsplit('|').next().unwrap_or(id) }
fn item(label: &str, action: &str, value: &str, confirm: bool) -> Item { Item { label: label.into(), action: action.into(), value: value.into(), confirm } }

/// The items for a thread row of the sidebar (`t` is its row, `project` the raw id of the project it is in).
pub fn thread_items(t: &Value, project: &str) -> Vec<Item> {
    let qid = s(t, "id");
    let mut v = vec![];
    /* i18n-ok: menu labels, tested; drawn through trs in render */ v.push(if t["pinned"].as_bool().unwrap_or(false) { item("Unpin", "row-unpin", &qid, false) } else { item("Pin", "row-pin", &qid, false) });
    if let Some(l) = t["lead"].as_array().and_then(|l| l.first()) { v.push(item(&s(l, "label"), &s(l, "action"), &s(l, "value"), false)); }
    /* i18n-ok: menu labels, tested; drawn through trs in render */ for o in t["trail"].as_array().into_iter().flatten().flat_map(|x| x["options"].as_array().into_iter().flatten()) { v.push(item(&format!("Snooze {}", s(o, "label").to_lowercase()), "row-snooze", &s(o, "value"), false)); }
    /* i18n-ok: menu labels, tested; drawn through trs in render */ if t["canChild"].as_bool().unwrap_or(true) { v.push(item("New subthread", "thread-child", &format!("{}|{}", raw(&qid), project), false)); }
    /* i18n-ok: menu labels, tested; drawn through trs in render */ if t["depth"].as_u64().unwrap_or(0) > 0 { v.push(item("Move to top level", "row-detach", raw(&qid), false)); }
    /* i18n-ok: menu labels, tested; drawn through trs in render */ if t["depth"].is_u64() && !project.is_empty() { v.push(item("Move under\u{2026}", LOCAL_MOVE, &format!("{}|{}", raw(&qid), project), false)); }
    /* i18n-ok: menu labels, tested; drawn through trs in render */ v.push(item("Rename", "row-rename", raw(&qid), false));
    /* i18n-ok: menu labels, tested; drawn through trs in render */ v.push(item("Regenerate title", "row-retitle", raw(&qid), false));
    // an archived row's lead (the hub's Unarchive) is already above
    let archived = t["lead"].as_array().map_or(false, |l| l.iter().any(|x| x["action"] == "row-unarchive"));
    /* i18n-ok: menu labels, tested; drawn through trs in render */ if !archived { v.push(item("Archive", "row-archive", &qid, false)); }
    /* i18n-ok: menu labels, tested; drawn through trs in render */ v.push(item("Delete", "row-delete", &qid, true));
    v
}
/// The threads a thread may go under (Move under…): the project's rows as the sidebar shows them (live, then settled), not the thread itself,
/// not a row of its own subtree (the rows after it that stand deeper) and not the parent it has now; `(title, row-reparent value)` each.
/// The hub checks the move again (same project, no cycle).
pub fn parent_choices(data: &Value, project: &str, tid: &str) -> Vec<Item> {
    let Some(p) = data["projects"].as_array().into_iter().flatten().find(|p| raw(p["id"].as_str().unwrap_or("")) == project) else { return vec![] };
    let rows: Vec<&Value> = ["threads", "settled"].iter().flat_map(|k| p[*k].as_array().into_iter().flatten()).collect();
    let depth = |r: &Value| r["depth"].as_u64().unwrap_or(0);
    let Some(at) = rows.iter().position(|r| raw(&s(r, "id")) == tid) else { return vec![] };
    let d = depth(rows[at]);
    let subtree_end = rows[at + 1..].iter().position(|r| depth(r) <= d).map_or(rows.len(), |n| at + 1 + n);
    let parent = (d > 0).then(|| rows[..at].iter().rev().find(|r| depth(r) < d).map(|r| raw(&s(r, "id")).to_string())).flatten();
    rows.iter().enumerate().filter(|(i, r)| (*i < at || *i >= subtree_end) && parent.as_deref() != Some(raw(&s(r, "id"))))
        .map(|(_, r)| item(&s(r, "title"), "row-reparent", &format!("{tid}|{}", raw(&s(r, "id"))), false)).collect()
}
/// A thread's row in the sidebar's picture (top level, settled or a subthread under another), by its raw id.
fn find_row<'a>(v: &'a Value, rid: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => { if m.get("id").and_then(Value::as_str).map_or(false, |id| raw(id) == rid) && m.contains_key("title") { return Some(v) } m.values().find_map(|x| find_row(x, rid)) }
        Value::Array(a) => a.iter().find_map(|x| find_row(x, rid)),
        _ => None,
    }
}
/// The thread's own menu: the ▾ beside its name in the top bar, and a right click on the name (the old header's thread menu). Rename and the
/// thread's place (New subthread, Move to top level), then the hub's thread menu (Diff, Terminal, Find file, Snooze, Delete); the header's
/// tools (Pin, Settle, Archive, Fork) are on the header itself. Empty with no thread.
pub fn title_items(data: &Value) -> Vec<Item> {
    let thread = &data["thread"];
    let rid = raw(thread["id"].as_str().unwrap_or("")).to_string();
    if rid.is_empty() { return vec![] }
    let row = find_row(&data["projects"], &rid);
    let project = row.map(|_| crate::keys::project_of(data, &rid)).unwrap_or_default();
    let mut v = vec![item("Find in thread", LOCAL_SEARCH, &rid, false), item("Rename", "row-rename", &rid, false), item("Regenerate title", "row-retitle", &rid, false)];
    if !project.is_empty() && row.map_or(false, |r| r["canChild"].as_bool().unwrap_or(true)) { v.push(item("New subthread", "thread-child", &format!("{rid}|{project}"), false)); }
    if row.map_or(false, |r| r["depth"].as_u64().unwrap_or(0) > 0) { v.push(item("Move to top level", "row-detach", &rid, false)); }
    v.extend(super::tools::context_items(thread));
    v
}
/// The items for a project's header.
/// A project that is also on linked machines (the hub's `newOn`): one New thread per machine, this one first, each naming its machine.
pub fn new_items(pid_raw: &str, new_on: &Value) -> Vec<Item> {
    let on: Vec<Item> = new_on.as_array().into_iter().flatten().map(|n| item(&s(n, "label"), "new-thread", &s(n, "value"), false)).filter(|i| !i.label.is_empty() && !i.value.is_empty()).collect();
    if on.len() > 1 { on } else { vec![item("New thread", "new-thread", pid_raw, false)] }
}
pub fn project_items(pid_raw: &str) -> Vec<Item> { project_items_on(pid_raw, &Value::Null) }
pub fn project_items_on(pid_raw: &str, new_on: &Value) -> Vec<Item> { let mut v = new_items(pid_raw, new_on); v.extend(project_rest(pid_raw)); v }
fn project_rest(pid_raw: &str) -> Vec<Item> { vec![item("Intent…", LOCAL_INTENT, pid_raw, false), item("Rename", "proj-rename", pid_raw, false), item("Badge", "look", pid_raw, false), item("Remove project", "proj-remove", pid_raw, true)] }

impl Backplane {
    pub(crate) fn ctx_open(&mut self, at: Point<Pixels>, items: Vec<Item>, cx: &mut Context<Self>) {
        if let Some(c) = self.screens.ctx.as_ref().and_then(disarm) { let _ = self.tx.try_send(c); }
        self.screens.ctx = Some(Menu { at, items, armed: None }); cx.notify();
    }
    /// Esc or a click outside: true when there was a menu to close.
    pub(crate) fn ctx_close(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(m) = self.screens.ctx.take() else { return false };
        if let Some(c) = disarm(&m) { let _ = self.tx.try_send(c); }
        cx.notify(); true
    }
    fn ctx_click(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(m) = self.screens.ctx.as_mut() else { return };
        match m.items.get(i).map(|it| (it.action.clone(), it.value.clone())) {
            // the window's own items: the menu shuts as Esc shuts it (an armed Delete is disarmed), then the item opens its place
            Some((a, v)) if a == LOCAL_SEARCH => { self.ctx_close(cx); self.find_open_for(&v, cx); return }
            Some((a, v)) if a == LOCAL_INTENT => { self.ctx_close(cx); self.open_project(v, cx); return }
            // the second menu, where the first was: the threads this one may go under
            Some((a, v)) if a == LOCAL_MOVE => {
                let (tid, project) = v.split_once('|').unwrap_or((v.as_str(), ""));
                let items = parent_choices(&self.data, project, tid);
                let at = m.at; self.screens.ctx = (!items.is_empty()).then(|| Menu { at, items, armed: None }); cx.notify(); return
            }
            _ => {}
        }
        if m.items.get(i).map_or(false, |it| it.action == "new-thread") { self.arm_new(); }
        let Some(m) = self.screens.ctx.as_mut() else { return };
        let step = press(m, i, &self.data);
        if let Some(c) = step.send { let _ = self.tx.try_send(c); }
        if step.close { self.screens.ctx = None; }
        cx.notify();
    }
}

/// What a click on item `i` does: what it sends and whether the menu shuts. A confirming item's first click only arms it (a thread's Delete
/// also arms the hub's, which keeps the thread's `deleting`); the second does it, and a Delete whose hub confirm went away (the sidebar paged
/// in between) arms it again instead of shutting with nothing deleted.
pub struct Step { pub send: Option<Command>, pub close: bool }
pub fn press(m: &mut Menu, i: usize, data: &Value) -> Step {
    let Some(it) = m.items.get(i).cloned() else { return Step { send: None, close: false } };
    let send = Some(Command::Action(it.action.clone(), it.value.clone()));
    if it.confirm && m.armed != Some(i) {
        m.armed = Some(i);
        return Step { send: if it.action == "row-delete" { send } else { None }, close: false };
    }
    if it.action == "row-delete" && raw(&s(&data["deleting"], "id")) != raw(&it.value) { return Step { send, close: false } }
    Step { send, close: true }
}
/// A menu shut with a thread's Delete armed: the hub's confirm goes too, so a later Delete asks again.
pub fn disarm(m: &Menu) -> Option<Command> {
    let it = m.items.get(m.armed?)?;
    (it.action == "row-delete").then(|| Command::Action("delete-no".into(), String::new()))
}
/// What an armed item says (0.14's words).
pub fn armed_label(it: &Item) -> String {
    /* i18n-ok: menu labels, tested; drawn through trs in render */ match it.action.as_str() { "row-delete" => "Click again to delete".into(), "proj-remove" => "Click again to remove".into(), _ => format!("Click again to {}", it.label.to_lowercase()) }
}

/// `armed_label` as drawn: the catalog's words for it, the item's own label translated inside.
pub fn shown_armed_label(it: &Item) -> String {
    match it.action.as_str() { "row-delete" | "proj-remove" => crate::i18n::trs(&armed_label(it)), _ => crate::i18n::tf("gpui.ctx.click.again.to", &[("what", crate::i18n::trs(&it.label).to_lowercase().into())]) }
}

/// The menu over everything, with a backdrop that closes it.
pub fn render(cx: &mut Context<Backplane>, light: bool, m: &Menu) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut list = div().absolute().left(m.at.x).top(m.at.y).min_w(px(190.)).flex().flex_col().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).occlude();
    for (i, it) in m.items.iter().enumerate() {
        let armed = m.armed == Some(i);
        let label = if armed { shown_armed_label(it) } else { crate::i18n::trs(&it.label) };
        let danger = it.confirm;
        list = list.child(crate::ui::with_rect(div().id(SharedString::from(format!("ctx-{i}"))), { let l = it.label.clone(); move || format!("ctx:{l}") }).px_3().py_1().cursor_pointer().text_color(if armed || danger { c("bad") } else { c("text") })
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
    #[test] fn the_thread_names_menu_offers_rename_its_place_and_the_hubs_thread_menu() {
        let d = j(r#"{"thread":{"id":"http://h:1|t7","menu":[{"label":"Diff","action":"diff","value":""},{"label":"Snooze","action":"snooze","value":"","options":[{"label":"1 hour","value":"1h"}]},{"label":"Delete","action":"row-delete","value":"t7","danger":true}]},
            "projects":[{"id":"http://h:1|p1","threads":[{"id":"http://h:1|t7","title":"x","canChild":true,"depth":0}]}]}"#);
        let it = title_items(&d);
        assert_eq!(it.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Find in thread", "Rename", "Regenerate title", "New subthread", "Diff", "Snooze 1 hour", "Delete"]);
        assert_eq!(it[0].action, LOCAL_SEARCH); assert_eq!((it[1].action.as_str(), it[1].value.as_str()), ("row-rename", "t7")); assert_eq!(it[3].value, "t7|p1");
        assert!(title_items(&j(r#"{"thread":null}"#)).is_empty(), "no thread, no menu");
        let sub = j(r#"{"thread":{"id":"h|c1"},"projects":[{"id":"h|p1","threads":[{"id":"h|t1","title":"a","children":[{"id":"h|c1","title":"b","depth":1,"canChild":false}]}]}]}"#);
        let names: Vec<String> = title_items(&sub).into_iter().map(|i| i.label).collect(); assert!(names.contains(&"Move to top level".to_string()) && !names.contains(&"New subthread".to_string()), "{names:?}");
    }
    #[test] fn a_thread_row_offers_what_the_hub_does() {
        let t = j(r#"{"id":"http://h:1|t7","pinned":false,"canChild":true,"lead":[{"label":"Settle","action":"row-settle","value":"http://h:1|t7"}],"trail":[{"label":"Delete"},{"label":"Snooze","options":[{"label":"1 hour","value":"http://h:1|t7|1h"}]}]}"#);
        let it = thread_items(&t, "p1");
        let names: Vec<&str> = it.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(names, ["Pin", "Settle", "Snooze 1 hour", "New subthread", "Rename", "Regenerate title", "Archive", "Delete"]);
        assert_eq!(it[3].value, "t7|p1"); assert_eq!(it[4].value, "t7"); assert!(it.last().unwrap().confirm && !it[0].confirm);
        let pinned = j(r#"{"id":"t1","pinned":true}"#); assert_eq!(thread_items(&pinned, "p")[0].action, "row-unpin");
    }
    fn sent(c: &Option<Command>) -> Option<(String, String)> { match c { Some(Command::Action(a, v)) => Some((a.clone(), v.clone())), _ => None } }
    #[test] fn delete_asks_in_the_menu_and_the_second_click_deletes() {
        let t = j(r#"{"id":"http://h:1|t7","pinned":false}"#);
        let mut m = Menu { at: Point::default(), items: thread_items(&t, "p1"), armed: None };
        let del = m.items.iter().position(|i| i.action == "row-delete").unwrap();
        let s1 = press(&mut m, del, &Value::Null);
        assert_eq!(sent(&s1.send), Some(("row-delete".into(), "http://h:1|t7".into())), "the first click arms the hub's confirm"); assert!(!s1.close && m.armed == Some(del));
        assert_eq!(armed_label(&m.items[del]), "Click again to delete");
        let armed = j(r#"{"deleting":{"id":"http://h:1|t7"}}"#);
        let s2 = press(&mut m, del, &armed); assert_eq!(sent(&s2.send), Some(("row-delete".into(), "http://h:1|t7".into()))); assert!(s2.close, "the second click deletes and shuts the menu");
        // the hub forgot the armed delete in between (a sidebar page): the click arms it again, the menu stays
        let mut m2 = Menu { at: Point::default(), items: thread_items(&t, "p1"), armed: Some(del) };
        let s3 = press(&mut m2, del, &Value::Null); assert!(!s3.close && sent(&s3.send).is_some());
        // shut with Delete armed: the hub's confirm goes too; shut with nothing armed sends nothing
        assert_eq!(sent(&disarm(&m2)), Some(("delete-no".into(), String::new()))); m2.armed = None; assert!(disarm(&m2).is_none());
        // a project's Remove asks in the menu, the hub hears only the second click
        let mut p = Menu { at: Point::default(), items: project_items("p1"), armed: None };
        let s = press(&mut p, 4, &Value::Null); assert!(s.send.is_none() && !s.close); assert_eq!(armed_label(&p.items[4]), "Click again to remove"); assert!(disarm(&p).is_none());
        let s = press(&mut p, 4, &Value::Null); assert_eq!(sent(&s.send), Some(("proj-remove".into(), "p1".into()))); assert!(s.close);
        // any other item does what it says at once
        let mut m3 = Menu { at: Point::default(), items: thread_items(&t, "p1"), armed: None }; let s = press(&mut m3, 0, &Value::Null); assert!(s.close && sent(&s.send).unwrap().0 == "row-pin");
    }
    #[test] fn the_thread_names_menu_asks_before_its_delete_too() {
        let d = j(r#"{"thread":{"id":"http://h:1|t7","menu":[{"label":"Diff","action":"diff","value":""},{"label":"Delete","action":"row-delete","value":"t7","danger":true}]},"projects":[]}"#);
        let it = title_items(&d); assert!(it.iter().find(|i| i.label == "Delete").unwrap().confirm && !it.iter().find(|i| i.label == "Diff").unwrap().confirm);
    }
    #[test] fn a_project_offers_new_thread_rename_badge_and_a_confirmed_remove() {
        let on = serde_json::json!([{"value":"p1","label":"New thread on desk"},{"value":"L~p9","label":"New thread on lap (away)"}]);
        let it = project_items_on("p1", &on); assert_eq!(it.iter().map(|i| (i.label.as_str(), i.value.as_str())).take(3).collect::<Vec<_>>(), [("New thread on desk", "p1"), ("New thread on lap (away)", "L~p9"), ("Intent…", "p1")]);
        assert_eq!(it[1].action, "new-thread");
        let it = project_items("p1"); assert_eq!(it.iter().map(|i| i.action.as_str()).collect::<Vec<_>>(), ["new-thread", LOCAL_INTENT, "proj-rename", "look", "proj-remove"]); assert!(it[4].confirm && !it[2].confirm && it[2].value == "p1" && it[1].value == "p1");
    }
    #[test] fn an_archived_row_offers_unarchive_not_archive() {
        let t = j(r#"{"id":"h|t7","lead":[{"label":"Unarchive","action":"row-unarchive","value":"h|t7"}]}"#);
        let it = thread_items(&t, "p1"); let acts: Vec<&str> = it.iter().map(|i| i.action.as_str()).collect();
        assert!(acts.contains(&"row-unarchive") && !acts.contains(&"row-archive"), "{acts:?}");
    }
    #[test] fn move_under_offers_every_row_but_itself_its_subtree_and_its_parent() {
        let d = j(r#"{"projects":[{"id":"h|p1","threads":[{"id":"h|a","title":"A","depth":0},{"id":"h|b","title":"B","depth":1},{"id":"h|c","title":"C","depth":2},{"id":"h|d","title":"D","depth":0}],"settled":[{"id":"h|e","title":"E","depth":0}]},{"id":"h|p2","threads":[{"id":"h|x","title":"X","depth":0}]}]}"#);
        let vals = |tid: &str| parent_choices(&d, "p1", tid).into_iter().map(|i| i.value).collect::<Vec<_>>();
        assert_eq!(vals("b"), ["b|d", "b|e"], "not itself, not its child C, not its parent A, never another project's X");
        assert_eq!(vals("a"), ["a|d", "a|e"]); assert_eq!(vals("d"), ["d|a", "d|b", "d|c", "d|e"]);
        assert!(parent_choices(&d, "p1", "zz").is_empty() && parent_choices(&d, "p9", "a").is_empty());
        let it = thread_items(&j(r#"{"id":"h|b","depth":1}"#), "p1"); let m = it.iter().find(|i| i.action == LOCAL_MOVE).unwrap(); assert_eq!(m.value, "b|p1");
    }
    #[test] fn an_armed_item_reads_in_the_language_in_force() {
        let t = j(r#"{"id":"h|t7"}"#); let items = thread_items(&t, "p1");
        let del = items.iter().find(|i| i.action == "row-delete").unwrap(); let arch = Item { confirm: true, ..items.iter().find(|i| i.action == "row-archive").unwrap().clone() };
        assert_eq!((shown_armed_label(del), shown_armed_label(&arch)), ("Click again to delete".to_string(), "Click again to archive".to_string()));
        crate::i18n::with_lang("ja", || {
            assert_eq!(shown_armed_label(del), crate::i18n::catalog().japanese("Click again to delete").unwrap());
            assert!(!shown_armed_label(&arch).contains("Archive") && !shown_armed_label(&arch).contains("archive"), "{}", shown_armed_label(&arch));
        });
    }
    #[test] fn only_a_subthread_offers_to_move_to_the_top_level() {
        let sub = j(r#"{"id":"http://h:1|t8","depth":1,"canChild":true}"#); let it = thread_items(&sub, "p1");
        let m = it.iter().find(|i| i.label == "Move to top level").unwrap(); assert_eq!((m.action.as_str(), m.value.as_str()), ("row-detach", "t8"));
        assert!(thread_items(&j(r#"{"id":"t1","depth":0}"#), "p1").iter().all(|i| i.action != "row-detach") && thread_items(&j(r#"{"id":"t1"}"#), "p1").iter().all(|i| i.action != "row-detach"));
    }
}

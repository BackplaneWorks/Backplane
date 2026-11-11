//! The sidebar's sections (docs/ALWAYS-ON.md section 8), as the hub presents them (`sections`: present/sections.rs): Needs you, Pinned,
//! Active, the groups and Quiet. Drawn generically: a section is a head (its title, a count, a chevron when it folds) and its rows, a thread
//! row (`kind` "threads") or a project row with its visible threads under it ("projects"). Bub is drawn above by the window as before.
//! The hub decides everything shown; this only draws and sends `side-section`, `project-page`, `project-pin`, `project-fold`, `group-rename-start`, `group-move`.
//! A project row's chevron (and Left/Right while the sidebar has the keyboard) folds its threads away; the hub keeps the fold per project.
use crate::i18n::{tf, trs};
use crate::script::{Ctl, CtlEntry};
use crate::{string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

/// Whether the hub sent sections (an older hub sends none: the window draws its projects as before).
pub fn on(data: &Value) -> bool { data["sections"].is_array() }

/// Every project row of the sections, for the project page and the keys (the rows' `id` is `<hub>|<project id>`; `nav` the project's key).
pub fn projects(data: &Value) -> Vec<&Value> {
    data["sections"].as_array().into_iter().flatten().filter(|s| s["kind"] == "projects").flat_map(|s| s["rows"].as_array().into_iter().flatten()).collect()
}
/// A project row of the sections by its raw id.
pub fn find<'a>(data: &'a Value, pid: &str) -> Option<&'a Value> {
    let page = &data["projectPage"]["project"];
    projects(data).into_iter().find(|p| string(p, "id").rsplit('|').next() == Some(pid)).or_else(|| (page.is_object() && string(page, "id").rsplit('|').next() == Some(pid)).then_some(page))
}

/// The head of a section: its title, its count when it folds or lists threads, a chevron when it folds.
fn head(cx: &mut Context<Backplane>, light: bool, s: &Value, ctl: &mut Vec<CtlEntry>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (id, title, fold, open) = (string(s, "id"), string(s, "title"), s["fold"] == true, s["open"] == true);
    let n = s["count"].as_u64().unwrap_or(0);
    let toggle = string(s, "toggle");
    let label = if fold || s["kind"] == "threads" { format!("{} {n}", trs(&title).to_uppercase()) } else { trs(&title).to_uppercase() };
    if fold { ctl.push(CtlEntry { id: format!("section:{id}"), label: title.clone(), ctl: Ctl::Node("side-section".into(), toggle.clone()) }); }
    let gid = string(s, "group");
    let needs = id == "needs";
    let mut d = crate::ui::with_rect(div().id(SharedString::from(format!("sec-{id}"))), { let id = id.clone(); move || format!("section:{id}") })
        .flex().items_center().gap_2().pl(px(6.)).pr_3().pt_3().pb_1().text_size(px(11.)).text_color(if needs { c("bad") } else { c("dim") })
        .children(fold.then(|| crate::ui::ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim")))
        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().child(label));
    if fold {
        d = d.cursor_pointer().hover(|s| s.text_color(c("text"))).on_click(cx.listener(move |this, _, _, cx| { this.go("side-section", toggle.clone(), cx); }));
    }
    if s["rename"] == true && !gid.is_empty() {
        /* i18n-ok: a ctl label */ ctl.push(CtlEntry { id: format!("group-rename:{gid}"), label: "Rename group".into(), ctl: Ctl::Node("group-rename-start".into(), gid.clone()) });
        d = d.on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
            /* i18n-ok: menu labels, drawn through trs */ let items = vec![crate::screens::ctxmenu::Item { label: "Rename group".into(), action: "group-rename-start".into(), value: gid.clone(), confirm: false }];
            this.ctx_open(e.position, items, cx);
        }));
    }
    d.into_any_element()
}

/// A project row: its badge and name, the machines' dots, the running count; a press opens its page, the menu pins it or moves it to a group.
fn project(this: &Backplane, cx: &mut Context<Backplane>, light: bool, p: &Value, faded: bool, ctl: &mut Vec<CtlEntry>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let qid = string(p, "id"); let pid = qid.rsplit_once('|').map_or(qid.as_str(), |(_, r)| r).to_string();
    let key = string(p, "nav");
    let on_page = this.project_page.as_deref() == Some(pid.as_str());
    ctl.push(CtlEntry { id: format!("project:{pid}"), label: string(p, "title"), ctl: Ctl::Home(pid.clone()) });
    let running = p["running"].as_u64().unwrap_or(0);
    let dots: Vec<AnyElement> = p["dots"].as_array().into_iter().flatten().filter(|_| p["dots"].as_array().map_or(0, |d| d.len()) > 1).map(|m| {
        div().size(px(6.)).rounded_full().bg(c(if m["dim"] == true { "faint" } else if m["here"] == true { "accent" } else { "dim" })).into_any_element()
    }).collect();
    let pinned = p["pinned"] == true;
    let groups = this.data["groupsTo"].clone();
    let menu = p["menu"].clone();
    let (pid2, key2, key3, pid3) = (pid.clone(), key.clone(), key.clone(), pid.clone());
    // the fold (the hub keeps it per project): a chevron while something is under the row or it is folded, an empty slot otherwise
    let (folded, under, needs_n) = (p["folded"] == true, p["under"].as_u64().unwrap_or(0), p["needsCount"].as_u64().unwrap_or(0));
    let fold_action = string(p, "fold");
    let can_fold = !fold_action.is_empty() && (folded || under > 0);
    /* i18n-ok: a ctl label */ if can_fold { ctl.push(CtlEntry { id: format!("project-fold:{pid}"), label: format!("{} {}", if folded { "Unfold" } else { "Fold" }, string(p, "title")), ctl: Ctl::Node(fold_action.clone(), key.clone()) }); }
    let twisty = crate::ui::with_rect(div().id(SharedString::from(format!("pfold-{pid}"))), { let p = pid.clone(); move || format!("project-fold:{p}") })
        .w(px(16.)).h(px(30.)).flex_shrink_0().flex().items_center().justify_center()
        .when(can_fold, |d| { let (a, k) = (fold_action.clone(), key.clone()); let (dim, text, line) = (c("dim"), c("text"), c("line")); let g = SharedString::from(format!("pfold-{pid}"));
            // the chevron: dim, lit with a square behind it under the pointer
            d.cursor_pointer().group(g.clone())
            .child(div().size(px(16.)).flex().items_center().justify_center().group_hover(g.clone(), move |s| s.bg(line))
                .child(svg().path(SharedString::from(format!("icon/{}.svg", if folded { "chevron-right" } else { "chevron-down" }))).size(px(12.)).text_color(dim).group_hover(g, move |s| s.text_color(text))))
            .on_click(cx.listener(move |this, _, w, cx| { cx.stop_propagation(); w.focus(&this.screens.side_focus); this.go(&a, k.clone(), cx); })) });
    /* i18n-ok: a ctl label */ ctl.push(CtlEntry { id: format!("project-pin:{pid}"), label: if pinned { "Unpin project".into() } else { "Pin project".into() }, ctl: Ctl::Node("project-pin".into(), key.clone()) });
    crate::ui::with_rect(div().id(SharedString::from(format!("sproj-{pid}"))), { let p = pid.clone(); move || format!("project:{p}") })
        .when(faded || p["off"] == true, |d| d.opacity(0.55)).flex().items_center().gap_2().pl(px(6.)).pr_3().h(px(30.)).text_size(px(13.))
        .border_l_2().border_color(if on_page { c("accent") } else { c("panel") }).when(on_page, |d| d.bg(c("raise"))).cursor_pointer().hover(|s| s.bg(c("raise")))
        .on_hover({ let p = pid.clone(); cx.listener(move |this, on: &bool, _, _| { if *on { this.screens.side_hover = Some(p.clone()) } else if this.screens.side_hover.as_deref() == Some(p.as_str()) { this.screens.side_hover = None } }) })
        .on_click(cx.listener(move |this, _, w, cx| { w.focus(&this.screens.side_focus); this.go("project-page", key2.clone(), cx); this.open_project(pid2.clone(), cx); }))
        .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
            // the hub's menu (Pin, New swarm, Rename, Badge, Open folder, Move to group ›, Archive, Remove project); an older hub sends none
            let hub_menu = crate::screens::ctxmenu::section_items(&menu);
            if !hub_menu.is_empty() { this.ctx_open(e.position, hub_menu, cx); return }
            /* i18n-ok: menu labels, drawn through trs */ let mut items = vec![crate::screens::ctxmenu::Item { label: if pinned { "Unpin project".into() } else { "Pin project".into() }, action: "project-pin".into(), value: key3.clone(), confirm: false },
                /* i18n-ok: a menu label, drawn through trs */ crate::screens::ctxmenu::Item { label: "New swarm".into(), action: "swarm-new".into(), value: pid3.clone(), confirm: false }];
            for g in groups.as_array().into_iter().flatten() {
                items.push(crate::screens::ctxmenu::Item { label: tf("gpui.sections.move.to", &[("group", string(g, "label").into())]), action: "group-move".into(), value: format!("{}\u{1f}{}", key3, string(g, "value")), confirm: false });
            }
            this.ctx_open(e.position, items, cx);
        }))
        .child(twisty)
        .child(crate::screens::badge::mark(light, p))
        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(string(p, "title")))
        .children((!dots.is_empty()).then(|| div().flex().gap_1().items_center().children(dots)))
        .children((running > 0).then(|| div().text_size(px(11.)).text_color(c("accent")).child(tf("gpui.sections.running", &[("n", running.into())]))))
        // what of it waits on the person (its rows are in Needs you, folded or not)
        .children((needs_n > 0).then(|| div().px(px(5.)).h(px(16.)).flex().items_center().bg(c("bad")).text_size(px(10.)).text_color(c("bg")).child(needs_n.to_string())))
        .when(pinned, |d| d.child(crate::ui::ic(light, "pin", 12., "dim")))
        // New thread in this project (a project also on linked machines: the + asks which machine)
        /* i18n-ok: a ctl id the drivers press */ .child(crate::ui::with_rect(div().id(SharedString::from(format!("new-{pid}"))), || "ui:New thread".to_string()).cursor_pointer().tooltip(crate::keys::tip("New thread", "", light)).on_click({ let (new_on, pidn) = (p["newOn"].clone(), pid.clone()); cx.listener(move |this, e: &ClickEvent, _, cx| { cx.stop_propagation();
            let items = crate::screens::ctxmenu::new_items(&pidn, &new_on); if items.len() > 1 { this.ctx_open(e.position(), items, cx); return }
            this.arm_new(); let _ = this.tx.try_send(crate::hub::Command::Action("new-thread".into(), pidn.clone())); cx.notify(); }) }).child(crate::ui::ic(light, "plus", 16., "dim")))
        .into_any_element()
}

/// The project row Left and Right fold and unfold in the sidebar: the one under the pointer, else the project page on show, else the open
/// thread's project. (the action, the project's key, whether it is folded now), None when that row has nothing to fold.
pub fn fold_target(data: &Value, hover: Option<&str>, page: Option<&str>, sel: &str) -> Option<(String, String, bool)> {
    let pid = hover.map(String::from).or_else(|| page.map(String::from)).unwrap_or_else(|| crate::keys::project_of(data, sel));
    let p = find(data, &pid)?;
    let (action, folded) = (string(p, "fold"), p["folded"] == true);
    (!action.is_empty() && (folded || p["under"].as_u64().unwrap_or(0) > 0)).then(|| (action, string(p, "nav"), folded))
}

impl Backplane {
    /// Left (fold: true) or Right on the sidebar: fold or unfold the project `fold_target` names; a row already so is left alone.
    pub(crate) fn key_fold(&mut self, fold: bool, cx: &mut Context<Self>) {
        let Some((action, key, folded)) = fold_target(&self.data, self.screens.side_hover.as_deref(), self.project_page.as_deref(), &self.selected) else { return };
        if folded != fold { self.go(&action, key, cx); }
    }
    /// The sections' rows, in the hub's order (Bub is drawn above them by the window).
    pub(crate) fn section_rows(&self, cx: &mut Context<Self>, light: bool, rows: &mut Vec<AnyElement>, ctl: &mut Vec<CtlEntry>) {
        let sections: Vec<Value> = self.data["sections"].as_array().cloned().unwrap_or_default();
        // the swarm page on show (its room), for the row's highlight (screens/swarm.rs draws the swarm rows)
        let open_room = if self.data["bot"]["kind"] == "room" { string(&self.data["bot"], "id") } else { String::new() };
        for s in &sections {
            match s["kind"].as_str().unwrap_or("") {
                "bub" => continue,
                "threads" => {
                    rows.push(head(cx, light, s, ctl));
                    for t in s["rows"].as_array().into_iter().flatten() {
                        // a swarm that waits on the person: its own row (screens/swarm.rs), which opens the swarm's page
                        if t["kind"] == "swarm" { rows.extend(crate::screens::swarm::side_rows(cx, light, &serde_json::json!({"swarms": [t]}), &open_room, ctl)); continue }
                        let pid = string(t, "projectKey");
                        // no project name after the title: the sidebar is narrow and the title is what the person reads (sub " ": an Active-view row, so no tree chevron and no drag)
                        // a stop says so by its dot alone (the right column is the title's); an ask keeps its red word (approval, question)
                        let mut t = t.clone(); if matches!(t["want"].as_str(), Some("failed" | "stopped")) { t["want"] = serde_json::json!(""); }
                        rows.push(self.side_thread(cx, light, &t, "", " ", t["off"] == true, ctl));
                    }
                }
                _ => {
                    let empty = s["rows"].as_array().map_or(true, |r| r.is_empty());
                    if s["id"] == "active" && empty && s["fold"] != true { continue }
                    rows.push(head(cx, light, s, ctl));
                    let faded = s["id"] == "quiet";
                    for p in s["rows"].as_array().into_iter().flatten() {
                        rows.push(project(self, cx, light, p, faded, ctl));
                        // its live swarms, above its threads (their members have no rows: the swarm stands for them)
                        rows.extend(crate::screens::swarm::side_rows(cx, light, p, &open_room, ctl));
                        let qid = string(p, "id"); let pid = qid.rsplit_once('|').map_or(qid.as_str(), |(_, r)| r).to_string();
                        for t in p["threads"].as_array().into_iter().flatten() { rows.push(self.side_thread(cx, light, t, &pid, "", faded, ctl)); }
                    }
                }
            }
        }
    }
}

/// The project page's thread lists from the hub (`projectPage`: running, needs you, recent, done behind "Done (n)"), or None when the hub
/// sent none for this project.
pub fn page_lists(cx: &mut Context<Backplane>, light: bool, data: &Value, pid: &str, ctl: &mut Vec<CtlEntry>) -> Option<Vec<AnyElement>> {
    let pg = &data["projectPage"];
    let p = find(data, pid)?;
    if !pg.is_object() || pg["key"] != p["nav"] { return None }
    let c = |n: &str| theme::color(light, n);
    let mut out = vec![];
    for s in pg["sections"].as_array().into_iter().flatten() {
        let rows = s["rows"].as_array().cloned().unwrap_or_default();
        let fold = s["fold"] == true;
        if rows.is_empty() && !fold { continue }
        if fold && s["count"].as_u64().unwrap_or(0) == 0 { continue }
        let (id, title, open, toggle) = (string(s, "id"), string(s, "title"), s["open"] == true, string(s, "toggle"));
        if fold { ctl.push(CtlEntry { id: format!("page-section:{id}"), label: title.clone(), ctl: Ctl::Node("side-section".into(), toggle.clone()) }); }
        out.push(div().id(SharedString::from(format!("page-sec-{id}"))).pt_3().flex().items_center().gap_1().text_size(px(11.)).text_color(c("faint"))
            .children(fold.then(|| crate::ui::ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim"))).child(trs(&title).to_uppercase())
            .when(fold, |d| d.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| { this.go("side-section", toggle.clone(), cx); }))).into_any_element());
        for t in &rows {
            let id = string(t, "id"); let raw = id.rsplit('|').next().unwrap_or(&id).to_string();
            if t["off"] != true { ctl.push(CtlEntry { id: format!("page-thread:{raw}"), label: string(t, "title"), ctl: Ctl::Select(id.clone()) }); }
            let off = t["off"] == true;
            if t["kind"] == "swarm" { out.extend(crate::screens::swarm::side_rows(cx, light, &serde_json::json!({"swarms": [t]}), "", ctl)); continue }
            out.push(div().id(SharedString::from(format!("page-thread-{raw}"))).flex().items_center().gap_2().px_2().h(px(30.)).border_b_1().border_color(c("line")).when(off, |d| d.opacity(0.55))
                .when(!off, |d| d.cursor_pointer().hover(|s| s.bg(c("raise"))))
                .child(super::siderow::dot(light, t)).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(string(t, "title")))
                .children(crate::screens::badge::machine_mark(light, &t["mark"], false, &id))
                .child(div().text_size(px(11.)).text_color(c("faint")).child(if string(t, "say").is_empty() { trs(&string(t, "ago")) } else { trs(&string(t, "say")) }))
                .when(!off, |d| d.on_click(cx.listener(move |this, _, window, cx| this.select(id.clone(), window, cx)))).into_any_element());
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*; use serde_json::json; use core::prelude::v1::test;
    #[test] fn left_and_right_fold_the_hovered_row_else_the_page_else_the_open_threads_project() {
        let d = json!({"sections":[{"id":"active","kind":"projects","rows":[
                {"id":"h|p1","nav":"k1","fold":"project-fold","folded":false,"under":2},
                {"id":"h|p2","nav":"k2","fold":"project-fold","folded":true,"under":0},
                {"id":"h|p3","nav":"k3","fold":"project-fold","folded":false,"under":0},
                {"id":"h|p4","nav":"k4"}]}],
            "projects":[{"id":"h|p1","threads":[{"id":"h|t1"}]}]});
        assert_eq!(fold_target(&d, Some("p2"), Some("p1"), "t1"), Some(("project-fold".into(), "k2".into(), true)), "the row under the pointer first");
        assert_eq!(fold_target(&d, None, Some("p1"), ""), Some(("project-fold".into(), "k1".into(), false)), "then the page on show");
        assert_eq!(fold_target(&d, None, None, "t1"), Some(("project-fold".into(), "k1".into(), false)), "then the open thread's project");
        assert_eq!(fold_target(&d, Some("p3"), None, ""), None, "nothing under it, open: nothing to fold");
        assert_eq!(fold_target(&d, Some("p4"), None, ""), None, "an older hub sends no fold");
        assert_eq!(fold_target(&d, None, None, ""), None);
    }
    #[test] fn a_project_is_found_in_any_section_by_its_raw_id() {
        let d = json!({"sections":[{"id":"needs","kind":"threads","rows":[{"id":"h|t1"}]},{"id":"active","kind":"projects","rows":[{"id":"h|p1","nav":"k1"}]},{"id":"quiet","kind":"projects","rows":[{"id":"h|L~p2","nav":"k2"}]}]});
        assert!(on(&d) && !on(&json!({})));
        assert_eq!(string(find(&d, "p1").unwrap(), "nav"), "k1"); assert_eq!(string(find(&d, "L~p2").unwrap(), "nav"), "k2"); assert!(find(&d, "t1").is_none());
        assert_eq!(projects(&d).len(), 2);
    }
}

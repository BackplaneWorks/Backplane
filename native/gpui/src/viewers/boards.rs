//! The Boards menu (0.14's board-selector, view.bend View.boards.selector): on Board, Schematic, 3D and Split of a project with several boards,
//! a button naming the board on show (its name over its folder, a chevron) opens a menu: "Find a board", the folder the paths are from, and a row per
//! board (its name, its path, "Default board" on the project's own), the one on show lit. Each opening reads the project's boards again
//! (`view-boards`, 0.14's data-on-open); a row shows that board (`view-board`), which the hub keeps for the thread's other pages.
use crate::{hub::Command, i18n::{tf, trs}, screens::composer::{Composer, ComposerEvent}, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Board { pub label: String, pub value: String, pub on: bool, pub default: bool }

/// The menu's state (held once, on the Board/Schematic page's camera): open or not, and its search field.
#[derive(Default)]
pub struct BoardsUi { pub open: bool, pub find: Option<Entity<Composer>> }

pub fn list(viewer: &Value) -> Vec<Board> {
    viewer["boards"].as_array().into_iter().flatten().map(|b| Board { label: s(b, "label"), value: s(b, "value"), on: b["on"] == true, default: b["default"] == true }).collect()
}
fn s(v: &Value, k: &str) -> String { v[k].as_str().unwrap_or("").to_string() }
/// A board's name: its file's stem (MC.Mech.stem).
pub fn stem(p: &str) -> String { let f = p.rsplit('/').next().unwrap_or(p); f.strip_suffix(".kicad_pcb").unwrap_or(f).to_string() }
/// The folder it is in, "" at the project's top (MC.Mech.parent).
pub fn parent(p: &str) -> String { p.rsplit_once('/').map(|x| x.0.to_string()).unwrap_or_default() }
/// The rows a search keeps: a case-insensitive part of the name or the path; an empty search keeps all.
pub fn matching<'a>(all: &'a [Board], q: &str) -> Vec<&'a Board> {
    let q = q.trim().to_lowercase();
    all.iter().filter(|b| q.is_empty() || b.value.to_lowercase().contains(&q) || stem(&b.value).to_lowercase().contains(&q)).collect()
}

fn find_text(ui: &BoardsUi, cx: &App) -> String { ui.find.as_ref().map(|f| f.read(cx).content.to_string()).unwrap_or_default() }

/// Open or shut the menu; opening reads the boards again and puts the keys in the search field.
pub fn toggle(this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) {
    let ui = &mut this.screens.plotview.boards;
    ui.open = !ui.open;
    if !ui.open { cx.notify(); return }
    if ui.find.is_none() {
        let f = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        f.update(cx, |i, _| i.set_placeholder(&trs("Name or path")));
        cx.observe(&f, |_, _, cx| cx.notify()).detach();
        // Enter shows the first board the search keeps
        cx.subscribe(&f, |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) {
            let all = list(&this.data["thread"]["viewer"]); let q = find_text(&this.screens.plotview.boards, cx);
            if let Some(b) = matching(&all, &q).first() { pick(this, b.value.clone(), cx); }
        }).detach();
        this.screens.plotview.boards.find = Some(f);
    }
    if let Some(f) = this.screens.plotview.boards.find.clone() { f.update(cx, |i, cx| i.set(String::new(), cx)); window.focus(&f.read(cx).focus_handle(cx)); }
    let _ = this.tx.try_send(Command::Action("view-boards".into(), String::new()));
    cx.notify();
}
fn pick(this: &mut Backplane, value: String, cx: &mut Context<Backplane>) {
    this.screens.plotview.boards.open = false;
    let _ = this.tx.try_send(Command::Action("view-board".into(), value));
    cx.notify();
}
/// Escape while the menu is open shuts it (main.rs's ClosePopup asks here first). True when it took the key.
pub fn escape(this: &mut Backplane, cx: &mut Context<Backplane>) -> bool {
    if !this.screens.plotview.boards.open { return false }
    this.screens.plotview.boards.open = false; cx.notify(); true
}

/// The button and, while open, its menu; None for a project with one board (the hub lists two or more).
pub fn picker(cx: &mut Context<Backplane>, light: bool, viewer: &Value, ui: &BoardsUi) -> Option<AnyElement> {
    let all = list(viewer);
    if all.len() < 2 { return None }
    let c = |n: &str| theme::color(light, n);
    let cur = all.iter().find(|b| b.on).map(|b| b.value.clone()).unwrap_or_default();
    let (name, folder) = if cur.is_empty() { (trs("Select board"), String::new()) } else { (stem(&cur), parent(&cur)) };
    let btn = div().id("boards-btn").flex().items_center().gap_2().h(crate::scale::tpx(28.)).px_2().w(px(260.)).min_w(px(96.)).flex_shrink().border_1().border_color(c(if ui.open { "accent" } else { "line" })).bg(c("raise"))
        .cursor_pointer().hover(|s| s.bg(c("panel"))).when(!ui.open, |d| d.tooltip(crate::keys::tip(&if cur.is_empty() { trs("Select board") } else { cur.clone() }, "", light)))
        .child(crate::art::icon("circuit-board", 14., c("dim")))
        .child(div().flex_1().min_w_0().flex().flex_col().justify_center().line_height(crate::scale::tpx(13.))
            .child(div().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(crate::scale::tpx(12.)).child(name))
            .when(!folder.is_empty(), |d| d.child(div().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(crate::scale::tpx(10.)).text_color(c("dim")).child(folder))))
        .child(crate::art::icon("chevron-down", 12., c("dim")))
        .on_click(cx.listener(|this, _, window, cx| toggle(this, window, cx)));
    let mut wrap = div().relative().flex().flex_shrink().min_w_0().child(btn);
    if ui.open {
        let q = find_text(ui, cx);
        let rows = matching(&all, &q);
        let mut menu = div().id("boards-menu").mt(px(4.)).w(px(420.)).max_h(px(480.)).overflow_y_scroll().flex().flex_col().gap(px(3.)).p(px(7.))
            .bg(c("panel")).border_1().border_color(c("line")).shadow_lg().text_size(crate::scale::tpx(12.)).occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| { this.screens.plotview.boards.open = false; cx.notify(); }))
            .child(div().px_1().text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(trs("Find a board")));
        if let Some(f) = ui.find.clone() { menu = menu.child(div().id("boards-find").px_2().py_1().bg(c("bg")).border_1().border_color(c("line")).child(f)); }
        let root = viewer["boardsRoot"].as_str().unwrap_or("");
        if !root.is_empty() { menu = menu.child(div().px_2().py_1().text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(tf("gpui.c.boards.folder", &[("folder", root.into())]))); }
        for (i, b) in rows.iter().enumerate() {
            let v = b.value.clone();
            menu = menu.child(div().id(SharedString::from(format!("board-opt-{i}"))).flex().flex_col().gap(px(2.)).px(px(10.)).py(px(6.)).cursor_pointer()
                .when(b.on, |d| d.bg(c("raise")).border_l_2().border_color(c("accent"))).hover(|s| s.bg(c("raise")))
                .child(div().text_color(c("text")).child(stem(&b.value)))
                .child(div().text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(b.value.clone()))
                .when(b.default, |d| d.child(div().text_size(crate::scale::tpx(11.)).text_color(c("accent")).child(trs("Default board"))))
                .on_click(cx.listener(move |this, _, _, cx| pick(this, v.clone(), cx))));
        }
        if rows.is_empty() { menu = menu.child(div().px_2().py_1().text_color(c("dim")).child(trs("No matching boards"))); }
        wrap = wrap.child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(menu)).with_priority(3));
    }
    Some(wrap.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_board_is_named_by_its_file_and_found_by_name_or_path() {
        assert_eq!((stem("docs/ref/CM5IO.kicad_pcb"), parent("docs/ref/CM5IO.kicad_pcb")), ("CM5IO".to_string(), "docs/ref".to_string()));
        assert_eq!((stem("top.kicad_pcb"), parent("top.kicad_pcb")), ("top".to_string(), String::new()));
        let v: Value = serde_json::from_str(r#"{"boards":[{"label":"cm5-carrier","value":"cm5-carrier.kicad_pcb","on":true,"default":true},{"label":"docs/ref/CM5IO","value":"docs/ref/CM5IO.kicad_pcb","on":false}]}"#).unwrap();
        let all = list(&v);
        assert!(all[0].on && all[0].default && !all[1].default);
        assert_eq!(matching(&all, "").len(), 2);
        assert_eq!(matching(&all, "cm5io")[0].value, "docs/ref/CM5IO.kicad_pcb");
        assert_eq!(matching(&all, "DOCS/").len(), 1);
        assert!(matching(&all, "nope").is_empty());
    }
}

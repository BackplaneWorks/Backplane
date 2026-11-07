//! Setup, drawn from the hub's `tour` (present/onboard.rs): the step list on the left, the step on the right. Every word and every decision is the hub's; rows
//! are the Settings rows, drawn by the same code. The accounts panel is the Accounts page's own, for the provider the step names.
use super::settings::{self, Field, FieldSlot};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Default)]
pub struct Setup {
    /// The picture has shown setup since it last went away: its checks were asked for and the keys were moved here.
    pub up: bool,
    /// The step last drawn, so a new one scrolls to its top and takes the keys.
    pub step: String,
    /// The connected accounts last seen, so a new sign-in runs the checks again.
    pub connected: usize,
    pub want_focus: bool,
    pub scroll: ScrollHandle,
}

/// The fields of the step on screen, kept with the other forms' (settings::sync).
pub fn form_fields(data: &Value) -> Vec<Field> { settings::parse(&serde_json::json!({"rows": data["tour"]["rows"]})).into_iter().flat_map(|r| r.fields).collect() }

/// Enter in a field presses its row's first button, as a click would.
pub fn submit(this: &mut Backplane, name: &str, cx: &mut Context<Backplane>) -> bool {
    let rows = settings::parse(&serde_json::json!({"rows": this.data["tour"]["rows"]}));
    let Some(b) = rows.iter().find(|r| r.fields.iter().any(|f| f.name == name)).and_then(|r| r.buttons.first().cloned()) else { return false };
    settings::flush_fields(this); let _ = this.tx.try_send(settings::press(&b)); cx.notify(); true
}

/// Keeps the client's own part of setup in step with the picture.
pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let t = &this.data["tour"];
    if !t.is_object() { this.screens.setup.up = false; this.screens.setup.step.clear(); return }
    let id = string(t, "id"); let panel = string(t, "panel");
    if !this.screens.setup.up { this.screens.setup.up = true; let _ = this.tx.try_send(Command::Action("ob-check".into(), String::new())); }
    if this.screens.setup.step != id {
        this.screens.setup.step = id; this.screens.setup.want_focus = true; this.screens.setup.scroll.set_offset(point(px(0.), px(0.)));
        if !panel.is_empty() { this.screens.acct.add_provider = panel; super::accounts::open_page(this, cx); }
    }
}

/// A sign-in finished while setup is up: the checks run again, so the step turns done.
pub fn accounts_changed(this: &mut Backplane) {
    let n = super::accounts::profiles(&this.screens.acct.snap).iter().filter(|p| p["state"] == "connected").count();
    if n != this.screens.setup.connected { this.screens.setup.connected = n; if this.data["tour"].is_object() { let _ = this.tx.try_send(Command::Action("ob-check".into(), String::new())); } }
}

/// The presses the client answers itself, before or beside the hub. True when nothing more is to be sent.
pub fn press(this: &mut Backplane, b: &settings::Btn, cx: &mut Context<Backplane>) -> bool {
    match (b.action.as_str(), b.value.as_str()) {
        ("copy", v) => { cx.write_to_clipboard(ClipboardItem::new_string(v.to_string())); true }
        ("ob-home", _) => { crate::launch::forget(); if let Ok(me) = std::env::current_exe() { let _ = std::process::Command::new(me).args(std::env::args().skip(1)).spawn(); } cx.quit(); true }
        ("ob-try", "palette") => { let _ = this.tx.try_send(settings::press(b)); this.palette_open(cx); true }
        _ => false,
    }
}

fn foot(cx: &mut Context<Backplane>, light: bool, id: &'static str, label: String, action: &'static str, primary: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(id).h(px(30.)).px_4().flex().items_center().text_size(px(13.)).cursor_pointer();
    let d = if primary { d.bg(c("accent")).text_color(c("on-accent")) } else { d.bg(c("raise")).hover(|s| s.bg(c("line"))) };
    d.child(label).on_click(cx.listener(move |this, _, _, cx| { settings::flush_fields(this); let _ = this.tx.try_send(Command::Action(action.into(), String::new())); cx.notify(); })).into_any_element()
}

/// The setup screen in a window `w` px wide: the step list's width, and whether a row's controls go under its words (as in Settings).
pub fn fit(w: f32) -> (f32, bool) { let rail = if w < 900. { 170. } else { 230. }; (rail, (w - rail - 48.).min(760.) < 620.) }
pub fn view(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, tour: &Value, acct: &super::accounts::Accounts, focus: &FocusHandle, scroll: &ScrollHandle, w: f32) -> Option<AnyElement> {
    if !tour.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    // the list of steps, under their groups
    let (rail_w, stack) = fit(w);
    let mut rail = div().id("setup-rail").flex().flex_col().w(px(rail_w)).flex_shrink_0().min_h_0().overflow_x_hidden().p_3().gap(px(1.)).overflow_y_scroll().border_r_1().border_color(c("line"));
    let (mut group, on) = (String::new(), string(tour, "id"));
    for (i, s) in tour["steps"].as_array().into_iter().flatten().enumerate() {
        let g = string(s, "group");
        if g != group { rail = rail.child(div().pt_3().pb_1().text_size(px(11.)).text_color(c("faint")).child(g.clone())); group = g; }
        let (id, state) = (string(s, "id"), string(s, "state"));
        let here = id == on;
        let mark = match state.as_str() { "done" => div().size(px(8.)).bg(c("ok")), "skipped" => div().w(px(8.)).h(px(2.)).bg(c("faint")), _ => div().size(px(8.)).border_1().border_color(c("faint")) };
        let row = div().id(SharedString::from(format!("setup-step-{i}"))).flex().items_center().gap_2().px_2().py(px(3.)).text_size(px(12.)).cursor_pointer().text_color(c(if here { "text" } else { "dim" }));
        let row = if here { row.bg(c("raise")) } else { row.hover(|x| x.bg(c("raise"))) };
        rail = rail.child(row.child(div().w(px(10.)).flex().justify_center().child(mark)).child(string(s, "title")).on_click(cx.listener(move |this, _, _, cx| { settings::flush_fields(this); let _ = this.tx.try_send(Command::Action("ob-go".into(), id.clone())); cx.notify(); })));
    }
    // the step
    let rows = settings::parse(&serde_json::json!({"rows": tour["rows"]}));
    let mut body = div().flex().flex_col().flex_shrink_0().gap_2().w_full().max_w(px(760.));
    body = body.child(div().text_size(px(12.)).text_color(c("faint")).child(format!("{} · {}", string(tour, "counter"), string(tour, "progress"))));
    body = body.child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).child(string(tour, "title")));
    body = body.child(div().flex_shrink_0().w_full().text_size(px(14.)).text_color(c("dim")).line_height(px(21.)).child(string(tour, "body"))); // measured at the width it is drawn at, so its rows start below its last line
    for (i, r) in rows.iter().enumerate() { body = body.child(settings::row_el(cx, light, fields, r, 1000 + i, stack)); }
    let panel = string(tour, "panel");
    if !panel.is_empty() { body = body.child(div().pt_3().flex().flex_col().gap_2().children(super::accounts::panel(cx, light, acct, &panel))); }
    for (i, ch) in tour["choices"].as_array().into_iter().flatten().enumerate() {
        let (a, v) = (string(ch, "action"), string(ch, "value"));
        body = body.child(div().pt_2().flex().child(div().id(SharedString::from(format!("setup-choice-{i}"))).h(px(30.)).px_4().flex().items_center().text_size(px(13.)).cursor_pointer().bg(c("raise")).hover(|x| x.bg(c("line"))).child(string(ch, "label"))
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); }))));
    }
    let mut left = div().flex().gap_2();
    if tour["first"] != true { left = left.child(foot(cx, light, "setup-back", string(tour, "back"), "ob-back", false)); }
    let footer = div().flex().justify_between().items_center().gap_3().px_6().py_3().border_t_1().border_color(c("line")).child(left)
        .child(div().flex().gap_2().child(foot(cx, light, "setup-end", string(tour, "end"), "ob-end", false)).child(foot(cx, light, "setup-later", string(tour, "skip"), "ob-skip", false)).child(foot(cx, light, "setup-next", string(tour, "next"), "ob-next", true)));
    let main = div().flex().flex_col().flex_1().min_w_0().child(div().id("setup-main").flex_1().min_h_0().overflow_y_scroll().track_scroll(scroll).px_6().py_5().child(body)).child(footer);
    Some(div().id("setup").absolute().top_0().left_0().size_full().flex().bg(c("bg")).text_color(c("text")).occlude().key_context("Setup").track_focus(focus)
        .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| { if e.keystroke.key == "enter" && !e.keystroke.modifiers.modified() { settings::flush_fields(this); let _ = this.tx.try_send(Command::Action("ob-next".into(), String::new())); cx.notify(); } }))
        .child(rail).child(main).into_any_element())
}

/// The sidebar's Finish setup row, while setup waits.
pub fn resume_row(cx: &mut Context<Backplane>, light: bool, data: &Value) -> Option<AnyElement> {
    let r = &data["setup"]; if !r.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    let (a, v) = (string(r, "action"), string(r, "value"));
    Some(div().id("setup-resume").mx_2().my_1().px_3().py_2().bg(c("raise")).text_size(px(12.)).cursor_pointer().hover(|x| x.bg(c("line"))).child(string(r, "label")).on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); })).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn setup_fits_a_small_window() {
        assert_eq!(fit(1400.), (230., false)); assert_eq!(fit(1024.), (230., false)); assert_eq!(fit(860.), (170., false)); assert_eq!(fit(800.), (170., true), "a narrow one stacks the controls under the words");
    }
    use core::prelude::v1::test;
    #[test] fn only_the_steps_own_fields_are_kept() {
        let d = serde_json::json!({"tour": {"rows": [{"label": "x", "fields": [{"name": "obproject", "hint": "Folder", "secret": false, "value": "/a"}]}, {"label": "y"}]}});
        let f = form_fields(&d); assert_eq!(f.len(), 1); assert_eq!(f[0].name, "obproject"); assert_eq!(f[0].value, "/a");
        assert!(form_fields(&serde_json::json!({})).is_empty());
    }
}

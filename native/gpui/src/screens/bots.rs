//! Bots and rooms (docs/bots.md) from the hub's screen model: the sidebar rows, the new-bot and new-room forms, a bot's tab strip and
//! its memory, routines and hooks pages, a room's or a remote bot's posts. Every press is the view action the phones send
//! (`bot`, `room`, `bot-tab`, `bot-new`, `bot-create`, `room-pick`, `room-save`, `form-close`, `room-post`, `routine-*`, `hook-*`,
//! `mem-forget`); typed fields are the form fields of screens::settings (`bfield`). Nothing here decides what a bot is or does.
//! Not drawn yet: a bot's Status plane, Space, Browser, Tasks, Mail and Settings pages (they say so).
use super::settings::{Field, FieldSlot};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::collections::HashMap;

const SEP: char = '\u{1f}';

#[derive(Clone, Debug, PartialEq)]
pub struct Tab { pub id: String, pub label: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Post { pub from: String, pub text: String, pub ago: String, pub mine: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Pick { pub id: String, pub name: String, pub on: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Provider { pub label: String, pub provider: String, pub on: bool }

fn arr<'a>(v: &'a Value, k: &str) -> impl Iterator<Item = &'a Value> { v[k].as_array().into_iter().flatten() }
pub fn tabs(bot: &Value, key: &str) -> Vec<Tab> { arr(bot, key).map(|t| Tab { id: string(t, "id"), label: string(t, "label") }).collect() }
pub fn posts(bot: &Value) -> Vec<Post> { arr(bot, "posts").map(|p| Post { from: string(p, "from"), text: string(p, "text"), ago: string(p, "ago"), mine: p["mine"].as_bool().unwrap_or(false) }).collect() }
pub fn picks(form: &Value) -> Vec<Pick> { arr(form, "picks").map(|p| Pick { id: string(p, "id"), name: string(p, "name"), on: p["on"].as_bool().unwrap_or(false) }).collect() }
pub fn providers(form: &Value) -> Vec<Provider> { arr(form, "providers").map(|p| Provider { label: string(p, "label"), provider: string(p, "provider"), on: p["on"].as_bool().unwrap_or(false) }).collect() }

/// The typed fields these screens need, so screens::settings::sync keeps one input for each: the new-bot form's name and persona, the
/// new-room form's name, a room's post box and a remote bot's message box.
pub fn form_fields(data: &Value) -> Vec<Field> {
    let f = |name: &str, hint: &str, value: String| Field { name: name.into(), hint: hint.into(), secret: false, value, vault: false };
    let mut out = Vec::new();
    let nb = &data["newBot"];
    if nb.is_object() { out.push(f("name", &string(nb, "hint"), string(nb, "name"))); out.push(f("persona", "Persona", string(nb, "persona"))); }
    let nr = &data["newRoom"];
    if nr.is_object() { out.push(f("rname", "Room name", string(nr, "name"))); }
    let b = &data["bot"];
    match b["kind"].as_str() { Some("room") => out.push(f("rpost", "Post to the room", String::new())), Some("remote") => out.push(f("rtext", "Message", String::new())), _ => {} }
    out
}

pub fn act(a: &str, v: &str) -> Command { Command::Action(a.into(), v.into()) }
/// A provider choice is a form field change: "provider<unit>value".
pub fn provider_choice(p: &Provider) -> Command { Command::Action("bfield".into(), format!("provider{SEP}{}", p.provider)) }

fn btn(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, cmd: fn(&str) -> Command, arg: String) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).px_2().py_1().text_size(px(12.)).cursor_pointer().border_1();
    let d = if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
    d.child(label).on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(cmd(&arg)); cx.notify(); })).into_any_element()
}
fn tab_cmd(id: &str) -> Command { act("bot-tab", id) }
fn pick_cmd(id: &str) -> Command { act("room-pick", id) }
fn forget_cmd(id: &str) -> Command { act("mem-forget", id) }
fn routine_toggle(id: &str) -> Command { act("routine-toggle", id) }
fn routine_run(id: &str) -> Command { act("routine-run", id) }
fn routine_delete(id: &str) -> Command { act("routine-delete", id) }
fn hook_revoke(id: &str) -> Command { act("hook-revoke", id) }
fn create_bot(_: &str) -> Command { act("bot-create", "") }
fn save_room(_: &str) -> Command { act("room-save", "") }
fn close_bot_form(_: &str) -> Command { act("form-close", "@bnew") }
fn close_room_form(_: &str) -> Command { act("form-close", "@rnew") }
fn send_room(_: &str) -> Command { act("room-post", "") }
fn send_remote(_: &str) -> Command { act("remote-send", "") }
fn provider_cmd(v: &str) -> Command { Command::Action("bfield".into(), format!("provider{SEP}{v}")) }

fn field_box(light: bool, fields: &HashMap<String, FieldSlot>, name: &str) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    fields.get(name).map(|s| div().w_full().max_w(px(520.)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(s.input.clone()).into_any_element())
}

/// A modal-ish form over the main area (new bot, new room): title, fields, choices, Cancel and the action.
pub fn forms(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, data: &Value) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let nb = &data["newBot"]; let nr = &data["newRoom"];
    let (title, body, close, go, go_label, ok): (&str, Vec<AnyElement>, fn(&str) -> Command, fn(&str) -> Command, &str, bool);
    if nb.is_object() {
        let mut els: Vec<AnyElement> = Vec::new();
        for n in ["name", "persona"] { if let Some(b) = field_box(light, fields, n) { els.push(b); } }
        let provs = providers(nb);
        if !provs.is_empty() { els.push(div().flex().flex_wrap().gap_2().children(provs.iter().enumerate().map(|(i, p)| btn(cx, light, format!("prov-{i}"), p.label.clone(), p.on, provider_cmd, p.provider.clone())).collect::<Vec<_>>()).into_any_element()); }
        (title, body, close, go, go_label, ok) = ("New bot", els, close_bot_form, create_bot, "Create", !string(nb, "name").trim().is_empty());
    } else if nr.is_object() {
        let mut els: Vec<AnyElement> = Vec::new();
        if let Some(b) = field_box(light, fields, "rname") { els.push(b); }
        els.push(div().text_size(px(11.)).text_color(c("dim")).child("Members").into_any_element());
        els.push(div().flex().flex_wrap().gap_2().children(picks(nr).iter().enumerate().map(|(i, p)| btn(cx, light, format!("pick-{i}"), p.name.clone(), p.on, pick_cmd, p.id.clone())).collect::<Vec<_>>()).into_any_element());
        (title, body, close, go, go_label, ok) = ("New room", els, close_room_form, save_room, "Save", picks(nr).iter().any(|p| p.on));
    } else { return None; }
    Some(div().id("bot-form").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().p_4().gap_3()
        .child(div().font_weight(FontWeight::BOLD).text_size(px(15.)).child(title))
        .children(body)
        .child(div().flex().gap_2().child(btn(cx, light, "form-cancel".into(), "Cancel".into(), false, close, String::new()))
            .child(div().opacity(if ok { 1. } else { 0.45 }).child(btn(cx, light, "form-go".into(), go_label.into(), true, if ok { go } else { |_| Command::Page(String::new()) }, String::new()))))
        .into_any_element())
}

/// The bot's header and tab strip at the top of the main area (a bot only); a fixed height so the page below can sit under it.
pub const BAR_H: f32 = 76.;
pub fn bar(cx: &mut Context<Backplane>, light: bool, bot: &Value) -> Option<AnyElement> {
    if bot["kind"] != "bot" { return None; }
    let c = |n: &str| theme::color(light, n);
    let on = bot["navTab"].as_str().or(bot["tab"].as_str()).unwrap_or("chat").to_string();
    let mut strip = div().flex().gap_1();
    for (i, t) in tabs(bot, "tabs").into_iter().enumerate() { let is = t.id == on; strip = strip.child(btn(cx, light, format!("btab-{i}"), t.label, is, tab_cmd, t.id)); }
    let subs = tabs(bot, "subtabs");
    let mut col = div().h(px(BAR_H)).flex_shrink_0().px_3().py_1().flex().flex_col().gap_1().border_b_1().border_color(c("line")).bg(c("bg"))
        .child(div().flex().gap_2().items_center().child(div().font_weight(FontWeight::BOLD).child(bot["title"].as_str().map(str::to_string).unwrap_or_else(|| string(bot, "name"))))
            .child(div().text_size(px(11.)).text_color(c("dim")).child(string(bot, "note")))).child(strip);
    if !subs.is_empty() { let cur = string(bot, "tab"); col = col.child(div().flex().gap_1().children(subs.into_iter().enumerate().map(|(i, t)| { let is = t.id == cur; btn(cx, light, format!("bsub-{i}"), t.label, is, tab_cmd, t.id) }).collect::<Vec<_>>())); }
    Some(col.into_any_element())
}

fn memory_page(cx: &mut Context<Backplane>, light: bool, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let items: Vec<&Value> = arr(bot, "memory").collect();
    if items.is_empty() { return div().p_3().text_color(c("dim")).child("Nothing remembered yet.").into_any_element(); }
    div().flex().flex_col().children(items.into_iter().enumerate().map(|(i, m)| {
        div().flex().flex_col().gap_1().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().flex().gap_2().child(div().font_weight(FontWeight::BOLD).child(string(m, "key"))).child(div().text_size(px(11.)).text_color(c("dim")).child(string(m, "kind")))
                .child(div().flex_1()).child(div().text_size(px(11.)).text_color(c("faint")).child(string(m, "updated"))).child(btn(cx, light, format!("forget-{i}"), "Forget".into(), false, forget_cmd, string(m, "key"))))
            .child(div().child(string(m, "text"))).into_any_element()
    }).collect::<Vec<_>>()).into_any_element()
}

fn routines_page(cx: &mut Context<Backplane>, light: bool, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let items: Vec<&Value> = arr(bot, "routines").collect();
    if items.is_empty() { return div().p_3().text_color(c("dim")).child("No routines.").into_any_element(); }
    div().flex().flex_col().children(items.into_iter().enumerate().map(|(i, r)| {
        let id = string(r, "id");
        let when = [string(r, "label"), string(r, "next"), string(r, "rel")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        div().flex().flex_col().gap_1().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().flex().gap_2().items_center().child(div().font_weight(FontWeight::BOLD).child(string(r, "name"))).child(div().text_size(px(11.)).text_color(c("dim")).child(if when.is_empty() { string(r, "cron") } else { when }))
                .child(div().flex_1())
                .child(btn(cx, light, format!("rt-on-{i}"), if r["on"] == true { "On".into() } else { "Off".into() }, r["on"] == true, routine_toggle, id.clone()))
                .child(btn(cx, light, format!("rt-run-{i}"), "Run".into(), false, routine_run, id.clone()))
                .child(btn(cx, light, format!("rt-del-{i}"), "Delete".into(), false, routine_delete, id)))
            .child(div().text_size(px(12.)).text_color(c("dim")).child(string(r, "prompt"))).into_any_element()
    }).collect::<Vec<_>>()).into_any_element()
}

fn hooks_page(cx: &mut Context<Backplane>, light: bool, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let items: Vec<&Value> = arr(bot, "hooks").collect();
    if items.is_empty() { return div().p_3().text_color(c("dim")).child("No hooks.").into_any_element(); }
    div().flex().flex_col().children(items.into_iter().enumerate().map(|(i, h)| {
        div().flex().gap_2().items_center().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().font_weight(FontWeight::BOLD).child(string(h, "name"))).child(div().text_size(px(11.)).text_color(c("dim")).child(string(h, "path")))
            .child(div().flex_1()).child(div().text_size(px(11.)).text_color(c("faint")).child(format!("{} calls", h["count"])))
            .child(btn(cx, light, format!("hook-{i}"), "Revoke".into(), false, hook_revoke, string(h, "id"))).into_any_element()
    }).collect::<Vec<_>>()).into_any_element()
}

fn talk_page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value, field: &str, send: fn(&str) -> Command) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let ps = posts(bot);
    let mut list = div().id("posts").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_2().p_3();
    if ps.is_empty() { list = list.child(div().text_color(c("dim")).child("No messages yet.")); }
    for p in &ps {
        let who = if p.mine { "You".to_string() } else { p.from.clone() };
        list = list.child(div().flex().flex_col().gap_1().child(div().flex().gap_2().child(div().font_weight(FontWeight::BOLD).child(who)).child(div().text_size(px(11.)).text_color(c("faint")).child(p.ago.clone()))).child(div().child(p.text.clone())));
    }
    div().size_full().flex().flex_col().child(list)
        .child(div().flex().gap_2().items_end().p_3().border_t_1().border_color(c("line"))
            .child(div().flex_1().px_2().py_1().border_1().border_color(c("line")).bg(c("view")).children(fields.get(field).map(|s| s.input.clone())))
            .child(btn(cx, light, "talk-send".into(), "Send".into(), true, send, String::new()))).into_any_element()
}

/// The page under a bot's tab strip, or a room's or remote bot's whole page; None for a bot's chat (the thread view) and for no bot.
pub fn page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let kind = bot["kind"].as_str().unwrap_or("");
    let header = |t: String| div().h(px(36.)).flex_shrink_0().px_3().flex().items_center().border_b_1().border_color(c("line")).font_weight(FontWeight::BOLD).child(t);
    let frame = |inner: AnyElement, top: f32, header: Option<Div>| div().id("bot-page").absolute().top(px(top)).left_0().right_0().bottom_0().occlude().bg(c("bg")).flex().flex_col().children(header).child(div().id("bot-body").flex_1().min_h_0().overflow_y_scroll().child(inner)).into_any_element();
    match kind {
        "room" => Some(div().id("room-page").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().child(header(bot["title"].as_str().map(str::to_string).unwrap_or_else(|| string(bot, "name")))).child(div().flex_1().min_h_0().child(talk_page(cx, light, fields, bot, "rpost", send_room))).into_any_element()),
        "remote" => Some(div().id("remote-page").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().child(header(bot["title"].as_str().map(str::to_string).unwrap_or_else(|| string(bot, "name")))).child(div().flex_1().min_h_0().child(talk_page(cx, light, fields, bot, "rtext", send_remote))).into_any_element()),
        "bot" => {
            let tab = bot["tab"].as_str().unwrap_or("chat");
            let inner = match tab {
                "chat" => return None,
                "memory" => memory_page(cx, light, bot),
                "routines" => routines_page(cx, light, bot),
                "hooks" => hooks_page(cx, light, bot),
                other => div().p_3().text_color(c("dim")).child(format!("The {other} page is not drawn here yet.")).into_any_element(),
            };
            Some(frame(inner, BAR_H, None))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn forms_and_boxes_ask_for_the_fields_they_show() {
        let d = j(r#"{"newBot":{"name":"","hint":"Name","persona":"","provider":"claude","providers":[{"label":"Claude","provider":"claude","on":true}]},"newRoom":null,"bot":{"kind":"room","id":"r1"}}"#);
        assert_eq!(form_fields(&d).iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["name", "persona", "rpost"]);
        let d = j(r#"{"newRoom":{"name":"x","picks":[]},"bot":{"kind":"remote"}}"#);
        assert_eq!(form_fields(&d).iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["rname", "rtext"]); assert!(form_fields(&Value::Null).is_empty());
        assert!(form_fields(&d).iter().all(|f| !f.vault && !f.secret));
    }
    #[test] fn tabs_posts_picks_and_providers_parse_from_the_hubs_rows() {
        let b = j(r#"{"kind":"bot","tabs":[{"id":"chat","label":"Chat"},{"id":"memory","label":"Memory"}],"subtabs":[{"id":"routines","label":"Routines"}],"posts":[{"id":"p1","from":"Kit","text":"hi","ago":"1m","mine":false},{"id":"p2","from":"","text":"yo","ago":"now","mine":true}]}"#);
        assert_eq!(tabs(&b, "tabs").len(), 2); assert_eq!(tabs(&b, "subtabs")[0].id, "routines");
        let p = posts(&b); assert_eq!((p[0].from.as_str(), p[1].mine), ("Kit", true));
        let f = j(r#"{"picks":[{"id":"b1","name":"Kit","on":true},{"id":"b2","name":"Mochi","on":false}],"providers":[{"label":"Claude","provider":"claude","on":false}]}"#);
        assert!(picks(&f)[0].on && !picks(&f)[1].on); assert_eq!(providers(&f)[0].provider, "claude");
    }
    #[test] fn presses_are_the_phones_actions() {
        for (c, a, v) in [(tab_cmd("memory"), "bot-tab", "memory"), (pick_cmd("b1"), "room-pick", "b1"), (forget_cmd("k"), "mem-forget", "k"), (routine_toggle("r"), "routine-toggle", "r"), (hook_revoke("h"), "hook-revoke", "h"), (close_bot_form(""), "form-close", "@bnew"), (close_room_form(""), "form-close", "@rnew"), (send_room(""), "room-post", ""), (provider_cmd("claude"), "bfield", "provider\u{1f}claude")] {
            match c { Command::Action(x, y) => assert_eq!((x.as_str(), y.as_str()), (a, v)), c => panic!("{c:?}") }
        }
    }
}

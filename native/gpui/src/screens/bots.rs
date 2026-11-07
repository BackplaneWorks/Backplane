//! Bots and rooms (docs/bots.md) from the hub's screen model: the sidebar rows, the new-bot and new-room forms, a bot's tab strip and
//! its memory, routines and hooks pages, a room's or a remote bot's posts. Every press is the view action the phones send
//! (`bot`, `room`, `bot-tab`, `bot-new`, `bot-create`, `room-pick`, `room-save`, `form-close`, `room-post`, `routine-*`, `hook-*`,
//! `mem-forget`); typed fields are the form fields of screens::settings (`bfield`). Nothing here decides what a bot is or does.
//! Status (the plane as text), Mail (the conversation headers), Space (the bot's blocks) and Settings (name, persona) are drawn; Browser and Tasks say they are not available here yet.
use super::settings::{Field, FieldSlot};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
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
    // a space's inputs: the typed text stays here (a vault field sends nothing) until the input's Send, which carries it in the space action
    if b["kind"] == "bot" && b["tab"] == "settings" { out.push(Field { name: persona_field(&string(b, "id")), hint: "Persona".into(), secret: false, value: string(b, "persona"), vault: true }); }
    if b["kind"] == "bot" && b["tab"] == "routines" && b["routine"]["open"] == true { out.push(f("oname", "Name", string(&b["routine"], "name"))); out.push(f("ocron", "When", string(&b["routine"], "cron"))); out.push(f("oprompt", "What to do each time", string(&b["routine"], "prompt"))); }
    if b["kind"] == "bot" && b["tab"] == "hooks" { out.push(Field { name: "hk.name".into(), hint: "Webhook name".into(), secret: false, value: String::new(), vault: true }); }
    if b["kind"] == "bot" && b["tab"] == "tasks" { for t in arr(b, "tasks") { for p in arr(t, "params") { out.push(Field { name: task_field(&string(t, "slug"), &string(p, "name")), hint: { let e = string(p, "example"); if e.is_empty() { string(p, "label") } else { format!("{} (e.g. {e})", string(p, "label")) } }, secret: false, value: String::new(), vault: true }); } } }
    if b["kind"] == "bot" { let mut ins = Vec::new(); space_inputs(&b["space"]["blocks"], &mut ins); for (a, l) in ins { out.push(Field { name: format!("sp.{a}"), hint: l, secret: false, value: String::new(), vault: true }); } }
    out
}

/// A task's run-form field for one of its parameters.
pub fn task_field(slug: &str, param: &str) -> String { format!("tk.{slug}.{param}") }
/// The task whose Delete was pressed once (a second press deletes; any other press of this page clears it): Teach's `asking`.
static ASKING: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
fn run_task(this: &mut Backplane, slug: &str, params: &[String], cx: &mut Context<Backplane>) {
    let mut o = serde_json::Map::new();
    for p in params { let v = this.screens.fields.get(&task_field(slug, p)).map(|s| s.input.read(cx).content.to_string()).unwrap_or_default(); o.insert(p.clone(), json!(v)); }
    let _ = this.tx.try_send(act("task-run", &format!("{slug}{SEP}{}", Value::Object(o))));
    cx.notify();
}
fn delete_task(this: &mut Backplane, slug: &str, cx: &mut Context<Backplane>) {
    let again = { let mut a = ASKING.lock().unwrap(); if *a == slug { a.clear(); true } else { *a = slug.to_string(); false } };
    if again { let _ = this.tx.try_send(act("task-del", slug)); }
    cx.notify();
}
/// The Tasks tab (MBots.tasks.tab): a card per task with its run form, Run and Delete (which asks first), then the newest runs.
fn tasks_page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let items: Vec<&Value> = arr(bot, "tasks").collect();
    if items.is_empty() { return div().p_3().text_color(c("dim")).child("No tasks yet. Teach this bot a browser task and it shows here.").into_any_element(); }
    let asking = ASKING.lock().unwrap().clone();
    let mut col = div().flex().flex_col();
    for (i, t) in items.into_iter().enumerate() {
        let slug = string(t, "slug"); let names: Vec<String> = arr(t, "params").map(|p| string(p, "name")).collect();
        let mut card = div().flex().flex_col().gap_2().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().flex().gap_2().items_center().child(div().font_weight(FontWeight::BOLD).child(string(t, "name"))).child(div().text_size(px(11.)).text_color(c("dim")).child(string(t, "last"))));
        let d = string(t, "desc"); if !d.is_empty() { card = card.child(div().text_size(px(12.)).text_color(c("dim")).child(d)); }
        for p in &names { if let Some(b) = field_box(light, fields, &task_field(&slug, p)) { card = card.child(b); } }
        let (s1, s2, n2) = (slug.clone(), slug.clone(), names.clone());
        card = card.child(div().flex().gap_2()
            .child(div().id(SharedString::from(format!("task-run-{i}"))).px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).child("Run").on_click(cx.listener(move |this, _, _, cx| run_task(this, &s1, &n2, cx))))
            .child(div().id(SharedString::from(format!("task-del-{i}"))).px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))).child(if asking == slug { "Delete this task?" } else { "Delete" }).on_click(cx.listener(move |this, _, _, cx| delete_task(this, &s2, cx)))));
        col = col.child(card);
    }
    let runs: Vec<&Value> = arr(bot, "taskRuns").collect();
    if !runs.is_empty() {
        col = col.child(div().px_3().pt_3().pb_1().text_size(px(11.)).text_color(c("dim")).child("RECENT RUNS"));
        for r in runs { col = col.child(div().flex().gap_2().px_3().py_1().text_size(px(12.)).child(div().child(string(r, "run"))).child(div().text_color(c("dim")).child(string(r, "status")))); }
    }
    col.into_any_element()
}

/// The (action, label) of every input block of a space, rows included.
pub fn space_inputs(blocks: &Value, out: &mut Vec<(String, String)>) {
    for b in blocks.as_array().into_iter().flatten() {
        match b["type"].as_str() { Some("input") => out.push((string(b, "action"), string(b, "label"))), Some("row") => space_inputs(&b["blocks"], out), _ => {} }
    }
}
/// An input's Send: the typed text goes with the action as `bots.space` (the bot hears "action: text"); the box is emptied.
pub fn send_space(this: &mut Backplane, action: &str, cx: &mut Context<Backplane>) {
    let name = format!("sp.{action}");
    let Some(input) = this.screens.fields.get(&name).map(|s| s.input.clone()) else { return };
    let text = input.read(cx).content.to_string();
    if text.trim().is_empty() { return; }
    let _ = this.tx.try_send(act("space-act", &format!("{action}{SEP}{text}")));
    input.update(cx, |i, cx| i.set(String::new(), cx));
    cx.notify();
}

pub fn act(a: &str, v: &str) -> Command { Command::Action(a.into(), v.into()) }
/// A provider choice is a form field change: "provider<unit>value".
pub fn provider_choice(p: &Provider) -> Command { Command::Action("bfield".into(), format!("provider{SEP}{}", p.provider)) }

fn btn(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, cmd: fn(&str) -> Command, arg: String) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).px_2().py_1().text_size(px(12.)).cursor_pointer().border_1();
    let d = if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
    d.child(label).on_click(cx.listener(move |this, _, _, cx| { super::settings::flush_fields(this); let _ = this.tx.try_send(cmd(&arg)); cx.notify(); })).into_any_element()
}
fn tab_cmd(id: &str) -> Command { act("bot-tab", id) }
fn memory_cmd(from: &str) -> Command { act("bot-memory", from) }
fn pick_cmd(id: &str) -> Command { act("room-pick", id) }
fn forget_cmd(id: &str) -> Command { act("mem-forget", id) }
fn routine_edit(id: &str) -> Command { act("routine-edit", id) }
fn routine_save(_: &str) -> Command { act("routine-save", "") }
fn close_routine_form(_: &str) -> Command { act("form-close", "@onew") }
fn routine_toggle(id: &str) -> Command { act("routine-toggle", id) }
fn routine_run(id: &str) -> Command { act("routine-run", id) }
fn routine_delete(id: &str) -> Command { act("routine-delete", id) }
fn space_cmd(v: &str) -> Command { act("space-act", v) }
fn hook_revoke(id: &str) -> Command { act("hook-revoke", id) }
fn create_bot(_: &str) -> Command { act("bot-create", "") }
fn save_room(_: &str) -> Command { act("room-save", "") }
fn close_bot_form(_: &str) -> Command { act("form-close", "@bnew") }
fn close_room_form(_: &str) -> Command { act("form-close", "@rnew") }
fn provider_cmd(v: &str) -> Command { Command::Action("bfield".into(), format!("provider{SEP}{v}")) }

/// A disabled control: drawn dimmed, no pointer, no handler, sends nothing.
fn inert(light: bool, label: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().px_2().py_1().text_size(px(12.)).border_1().border_color(c("line")).bg(c("raise")).text_color(c("faint")).opacity(0.6).child(label.to_string()).into_any_element()
}

/// What a form or page asks the window to focus when it opens: the name of its first typed field, "" for none.
pub fn focus_target(data: &Value) -> String {
    if data["renaming"].is_object() { return super::tools::focus_target(data).into(); }
    if data["newBot"].is_object() { return "name".into(); }
    if data["newRoom"].is_object() { return "rname".into(); }
    match data["bot"]["kind"].as_str() { Some("room") => "rpost".into(), Some("remote") => "rtext".into(), _ => String::new() }
}
/// Esc on a form: the action that drops it.
pub fn close_form(data: &Value) -> Option<Command> {
    if let Some(c) = super::tools::close_question(data) { return Some(c); }
    if data["newBot"].is_object() { Some(act("form-close", "@bnew")) } else if data["newRoom"].is_object() { Some(act("form-close", "@rnew")) } else { None }
}

/// Send a post: the box's pending typing first, then the send action, then the box is emptied (nothing is sent for blank text).
pub fn send_post(this: &mut Backplane, field: &str, action: &str, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.fields.get(field).map(|s| s.input.clone()) else { return };
    if input.read(cx).content.trim().is_empty() { return; }
    super::settings::flush_fields(this);
    let _ = this.tx.try_send(act(action, ""));
    input.update(cx, |i, cx| i.set(String::new(), cx));
    cx.notify();
}

fn field_box(light: bool, fields: &HashMap<String, FieldSlot>, name: &str) -> Option<AnyElement> { field_box_h(light, fields, name, 0.) }
/// A field's box at least `min_h` px tall (the persona wraps to several lines; the box is as tall as the text it holds).
fn field_box_h(light: bool, fields: &HashMap<String, FieldSlot>, name: &str, min_h: f32) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    fields.get(name).map(|s| div().w_full().max_w(px(520.)).min_h(px(min_h)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(s.input.clone()).into_any_element())
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
        (title, body, close, go, go_label, ok) = ("New bot", els, close_bot_form, create_bot, "Create", fields.get("name").map_or(false, |s| !s.input.read(cx).content.trim().is_empty()));
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
            .child(if ok { btn(cx, light, "form-go".into(), go_label.into(), true, go, String::new()) } else { inert(light, go_label) }))
        .into_any_element())
}

/// The bot's header and tab strip at the top of the main area (a bot only); a fixed height so the page below can sit under it.
pub const BAR_H: f32 = 76.;
/// The strip is taller with a row of sub-tabs under it (Browser: Tasks, Mailroom: Status, Settings: Webhooks), or they sit under the page.
pub fn bar_h(bot: &Value) -> f32 { if bot["subtabs"].as_array().map_or(false, |a| !a.is_empty()) { BAR_H + 36. } else { BAR_H } }
pub fn bar(cx: &mut Context<Backplane>, light: bool, bot: &Value) -> Option<AnyElement> {
    if bot["kind"] != "bot" { return None; }
    let c = |n: &str| theme::color(light, n);
    let on = bot["navTab"].as_str().or(bot["tab"].as_str()).unwrap_or("chat").to_string();
    let mut strip = div().flex().gap_1();
    for (i, t) in tabs(bot, "tabs").into_iter().enumerate() { let is = t.id == on; strip = strip.child(btn(cx, light, format!("btab-{i}"), t.label, is, tab_cmd, t.id)); }
    let subs = tabs(bot, "subtabs");
    let mut col = div().h(px(bar_h(bot))).flex_shrink_0().px_3().py_1().flex().flex_col().gap_1().border_b_1().border_color(c("line")).bg(c("bg"))
        .child(div().flex().gap_2().items_center().child(div().font_weight(FontWeight::BOLD).child(bot["title"].as_str().map(str::to_string).unwrap_or_else(|| string(bot, "name"))))
            .child(div().text_size(px(11.)).text_color(c("dim")).child(string(bot, "note")))).child(strip);
    if !subs.is_empty() { let cur = string(bot, "tab"); col = col.child(div().flex().gap_1().children(subs.into_iter().enumerate().map(|(i, t)| { let is = t.id == cur; btn(cx, light, format!("bsub-{i}"), t.label, is, tab_cmd, t.id) }).collect::<Vec<_>>())); }
    Some(col.into_any_element())
}

fn memory_page(cx: &mut Context<Backplane>, light: bool, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let items: Vec<&Value> = arr(bot, "memory").collect();
    if items.is_empty() { return div().p_3().text_color(c("dim")).child("Nothing remembered yet.").into_any_element(); }
    let more = bot["memoryMore"].as_u64().unwrap_or(0); let items_len = items.len();
    div().id(SharedString::from(format!("memory-list-{}", bot["memoryFrom"].as_u64().unwrap_or(0)))).size_full().overflow_y_scroll().flex().flex_col().children(items.into_iter().enumerate().map(|(i, m)| {
        div().flex().flex_col().gap_1().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().flex().gap_2().child(div().font_weight(FontWeight::BOLD).child(string(m, "key"))).child(div().text_size(px(11.)).text_color(c("dim")).child(string(m, "kind")))
                .child(div().flex_1()).child(div().text_size(px(11.)).text_color(c("faint")).child(string(m, "updated"))).child(btn(cx, light, format!("forget-{i}"), "Forget".into(), false, forget_cmd, string(m, "key"))))
            .child(div().child(string(m, "text"))).into_any_element()
    }).collect::<Vec<_>>()).child({
        let from = bot["memoryFrom"].as_u64().unwrap_or(0); let total = bot["memoryTotal"].as_u64().unwrap_or(0);
        let mut bar = div().flex().gap_2().items_center().p_3().child(div().text_size(px(11.)).text_color(c("dim")).child(format!("{}–{} of {}", from + 1, from + items_len as u64, total)));
        if let Some(p) = bot["memoryPrev"].as_u64() { bar = bar.child(btn(cx, light, "mem-newer".into(), "Newer".into(), false, memory_cmd, p.to_string())); }
        if let Some(n) = bot["memoryNext"].as_u64() { bar = bar.child(btn(cx, light, "mem-older".into(), format!("Older ({more} more)"), false, memory_cmd, n.to_string())); }
        bar
    }).into_any_element()
}

/// The hub's reason a routine did not save (`routine.error`), shown on the form; None when there is none.
fn routine_error(form: &Value) -> Option<String> { Some(string(form, "error")).filter(|e| !e.is_empty()) }
fn routines_page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let items: Vec<&Value> = arr(bot, "routines").collect();
    let form = &bot["routine"];
    let head = if form["open"] == true {
        let hint = string(form, "hint");
        let can = fields.get("ocron").map_or(false, |s| !s.input.read(cx).content.trim().is_empty());
        div().flex().flex_col().gap_2().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().font_weight(FontWeight::BOLD).child(if string(form, "id").is_empty() { "New routine" } else { "Routine" }))
            .children(field_box(light, fields, "oname")).children(field_box(light, fields, "ocron"))
            .child(div().text_size(px(11.)).text_color(c("dim")).child(if hint.is_empty() { "When: every 15m, daily 09:30, weekdays 08:00, or a one-time time".to_string() } else { hint }))
            .children(field_box_h(light, fields, "oprompt", 60.))
            .children(routine_error(form).map(|e| div().id("rt-error").text_size(px(12.)).text_color(c("bad")).child(e)))
            .child(div().flex().gap_2().child(btn(cx, light, "rt-cancel".into(), "Cancel".into(), false, close_routine_form, String::new())).child(if can { btn(cx, light, "rt-save".into(), "Save".into(), true, routine_save, String::new()) } else { div().px_2().py_1().text_size(px(12.)).text_color(c("faint")).child("Save").into_any_element() }))
    } else {
        div().flex().px_3().py_2().border_b_1().border_color(c("line")).child(btn(cx, light, "rt-new".into(), "New routine".into(), false, routine_edit, String::new()))
    };
    if items.is_empty() { return div().flex().flex_col().child(head).child(div().p_3().text_color(c("dim")).child("No routines.")).into_any_element(); }
    div().flex().flex_col().child(head).children(items.into_iter().enumerate().map(|(i, r)| {
        let id = string(r, "id");
        let when = [string(r, "label"), string(r, "next"), string(r, "rel")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
        div().flex().flex_col().gap_1().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().flex().gap_2().items_center().child(div().font_weight(FontWeight::BOLD).child(string(r, "name"))).child(div().text_size(px(11.)).text_color(c("dim")).child(if when.is_empty() { string(r, "cron") } else { when }))
                .child(div().flex_1())
                .child(btn(cx, light, format!("rt-on-{i}"), if r["on"] == true { "On".into() } else { "Off".into() }, r["on"] == true, routine_toggle, id.clone()))
                .child(btn(cx, light, format!("rt-edit-{i}"), "Edit".into(), false, routine_edit, id.clone()))
                .child(btn(cx, light, format!("rt-run-{i}"), "Run".into(), false, routine_run, id.clone()))
                .child(btn(cx, light, format!("rt-del-{i}"), "Delete".into(), false, routine_delete, id)))
            .child(div().text_size(px(12.)).text_color(c("dim")).child(string(r, "prompt"))).into_any_element()
    }).collect::<Vec<_>>()).into_any_element()
}


fn note(light: bool, t: &str) -> AnyElement { div().p_3().text_color(theme::color(light, "dim")).child(t.to_string()).into_any_element() }

fn status_page(light: bool, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let text = string(bot, "plane");
    if text.trim().is_empty() { return note(light, "Nothing to report."); }
    div().flex().flex_col().p_3().gap_1().font_family("monospace").text_size(px(12.)).children(text.lines().map(|l| {
        let head = !l.starts_with(' ') && !l.is_empty();
        div().when(head, |d| d.font_weight(FontWeight::BOLD).pt_2()).when(!head, |d| d.text_color(c("dim"))).child(l.to_string())
    }).collect::<Vec<_>>()).into_any_element()
}

fn mail_page(light: bool, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let items: Vec<&Value> = arr(bot, "mail").collect();
    if items.is_empty() { return note(light, "No mail."); }
    let total = bot["mailTotal"].as_u64().unwrap_or(0);
    div().flex().flex_col().children(items.iter().map(|m| {
        let unread = m["unread"] == true;
        div().flex().flex_col().gap_1().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().flex().gap_2().child(div().when(unread, |d| d.font_weight(FontWeight::BOLD)).child(string(m, "subject")))
                .child(div().flex_1()).child(div().text_size(px(11.)).text_color(c("faint")).child(format!("{} · {}", m["n"], string(m, "ago")))))
            .child(div().text_size(px(12.)).text_color(c("dim")).child(format!("{}: {}", string(m, "from"), string(m, "head")))).into_any_element()
    }).collect::<Vec<_>>()).children((total as usize > items.len()).then(|| div().p_3().text_size(px(11.)).text_color(c("dim")).child(format!("{} older conversations not shown", total as usize - items.len())))).into_any_element()
}

fn space_block(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, b: &Value, key: String) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let strs = |v: &Value| -> Vec<String> { v.as_array().into_iter().flatten().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect() };
    match b["type"].as_str().unwrap_or("") {
        "heading" => div().font_weight(FontWeight::BOLD).text_size(px(16.)).child(string(b, "text")).into_any_element(),
        "text" => div().child(string(b, "text")).into_any_element(),
        "stat" => div().flex().flex_col().child(div().text_size(px(11.)).text_color(c("dim")).child(string(b, "label"))).child(div().font_weight(FontWeight::BOLD).text_size(px(20.)).child(string(b, "value"))).child(div().text_size(px(11.)).text_color(c("faint")).child(string(b, "hint"))).into_any_element(),
        "progress" => { let v = b["value"].as_f64().unwrap_or(0.).clamp(0., 1.) as f32;
            div().flex().flex_col().gap_1().child(div().text_size(px(11.)).text_color(c("dim")).child(format!("{} {}%", string(b, "label"), (v * 100.) as u32)))
                .child(div().h(px(6.)).w_full().bg(c("line")).child(div().h_full().w(relative(v)).bg(c("accent")))).into_any_element() }
        "list" => div().flex().flex_col().children(strs(&b["items"]).into_iter().map(|t| div().child(format!("• {t}"))).collect::<Vec<_>>()).into_any_element(),
        "kv" => div().flex().flex_col().children(arr(b, "rows").map(|r| { let p = strs(r); div().flex().gap_2().child(div().w(px(140.)).text_color(c("dim")).child(p.first().cloned().unwrap_or_default())).child(p.get(1).cloned().unwrap_or_default()) }).collect::<Vec<_>>()).into_any_element(),
        "table" => div().flex().flex_col().child(div().flex().gap_3().font_weight(FontWeight::BOLD).children(strs(&b["head"]).into_iter().map(|h| div().w(px(120.)).child(h)).collect::<Vec<_>>()))
            .children(arr(b, "rows").map(|r| div().flex().gap_3().children(strs(r).into_iter().map(|x| div().w(px(120.)).child(x)).collect::<Vec<_>>())).collect::<Vec<_>>()).into_any_element(),
        "button" => btn(cx, light, key, string(b, "label"), false, space_cmd, format!("{}{SEP}", string(b, "action"))),
        "input" => { let action = string(b, "action");
            div().flex().flex_col().gap_1().child(div().text_size(px(11.)).text_color(c("dim")).child(string(b, "label")))
                .child(div().flex().gap_2().items_end().child(div().flex_1().children(field_box(light, fields, &format!("sp.{action}"))))
                    .child(div().id(SharedString::from(format!("{key}-send"))).px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).child("Send")
                        .on_click(cx.listener(move |this, _, _, cx| send_space(this, &action, cx))))).into_any_element() }
        "divider" => div().h(px(1.)).w_full().bg(c("line")).into_any_element(),
        "row" => { let kids: Vec<AnyElement> = arr(b, "blocks").enumerate().map(|(i, k)| space_block(cx, light, fields, k, format!("{key}-{i}"))).collect(); div().flex().gap_3().children(kids).into_any_element() }
        _ => div().into_any_element(),
    }
}

fn space_page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value) -> AnyElement {
    let sp = &bot["space"];
    if !sp.is_object() { return note(light, if bot["spacePage"] == true { "This space is a web page; it is shown in the phone apps." } else { "No space yet. The bot writes it with space_set." }); }
    let blocks: Vec<AnyElement> = arr(sp, "blocks").enumerate().map(|(i, b)| space_block(cx, light, fields, b, format!("sp-{i}"))).collect();
    div().flex().flex_col().gap_2().p_3().children((!string(sp, "title").is_empty()).then(|| div().font_weight(FontWeight::BOLD).text_size(px(18.)).child(string(sp, "title")))).children(blocks).into_any_element()
}

fn settings_page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let row = |k: &str, v: String| div().flex().flex_col().gap_1().child(div().text_size(px(11.)).text_color(c("dim")).child(k.to_string())).child(div().child(v));
    let (id, field) = (string(bot, "id"), persona_field(&string(bot, "id")));
    div().flex().flex_col().gap_3().p_3().child(row("Name", string(bot, "name")))
        .child(div().flex().flex_col().gap_1().child(div().text_size(px(11.)).text_color(c("dim")).child("Persona")).children(field_box_h(light, fields, &field, 96.)))
        .child(div().flex().gap_2().child(div().id("bot-save").px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).child("Save")
            .on_click(cx.listener(move |this, _, _, cx| save_persona(this, &id, cx)))))
        .child(row("Created", string(bot, "created"))).into_any_element()
}
/// The persona box of one bot: its name carries the bot, so another bot's page never shows this one's text.
pub fn persona_field(bot: &str) -> String { format!("bp.{bot}") }
/// Settings' Save: the persona goes to the hub as `bots.edit` (the name is the bot's for good).
pub fn save_persona(this: &mut Backplane, bot: &str, cx: &mut Context<Backplane>) {
    let Some(input) = this.screens.fields.get(&persona_field(bot)).map(|s| s.input.clone()) else { return };
    let text = input.read(cx).content.to_string();
    let _ = this.tx.try_send(Command::Rpc("bots.edit".into(), super::settings::Secret(serde_json::json!({"bot": bot, "persona": text})), "botedit"));
}

/// What the last webhook request answered, shown on the Hooks tab until the next one: a new hook's secret (once), or a token and where to post.
static REVEAL: std::sync::Mutex<Value> = std::sync::Mutex::new(Value::Null);
/// What a webhook request answered. The hub's replies for these are flat, like the secret ones: `{t, id, ok, tokenHook, token, tokenUrl}`, no `result`. What it reveals goes to the screen only: never a log line.
pub fn hook_answer(reply: &Value) -> Value {
    if reply["result"].is_object() { return reply["result"].clone() }
    let mut o = reply.as_object().cloned().unwrap_or_default();
    for k in ["t", "id", "ok"] { o.remove(k); }
    Value::Object(o)
}
/// The answer of `bots.hook` / `bots.hook_token` (the hub hands the whole reply on for these tags).
pub fn hook_done(this: &mut Backplane, _tag: &str, ok: bool, why: &str, cx: &mut Context<Backplane>) {
    *REVEAL.lock().unwrap() = if ok { serde_json::from_str(why).unwrap_or(Value::Null) } else { json!({"error": if why.is_empty() { "The hub refused it.".to_string() } else { why.to_string() }}) };
    if ok { if let Some(input) = this.screens.fields.get("hk.name").map(|s| s.input.clone()) { input.update(cx, |i, cx| i.set(String::new(), cx)); } }
    let _ = this.tx.try_send(act("hook-refresh", "")); cx.notify();
}
fn new_hook(this: &mut Backplane, bot: &str, cx: &mut Context<Backplane>) {
    let name = this.screens.fields.get("hk.name").map(|s| s.input.read(cx).content.trim().to_string()).unwrap_or_default();
    let _ = this.tx.try_send(Command::Rpc("bots.hook".into(), super::settings::Secret(json!({"bot": bot, "name": if name.is_empty() { "webhook".to_string() } else { name }})), "hook-new")); cx.notify();
}
fn hook_token(this: &mut Backplane, hook: &str, op: &str, cx: &mut Context<Backplane>) {
    let _ = this.tx.try_send(Command::Rpc("bots.hook_token".into(), super::settings::Secret(json!({"hook": hook, "op": op})), "hook-token")); cx.notify();
}
fn chip(cx: &mut Context<Backplane>, light: bool, id: String, label: &str, accent: bool, f: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).px_2().py_1().text_size(px(12.)).cursor_pointer().border_1();
    let d = if accent { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
    d.child(label.to_string()).on_click(cx.listener(move |this, _, _, cx| f(this, cx))).into_any_element()
}
/// The registered controls of the bots page (the e2e harness's `click`), named as the elements are: the new-bot form's `prov-<i>`, `form-go`, `form-cancel`;
/// a bot's `btab-<i>` and `bsub-<i>`; the Webhooks tab's `hook-new` and each hook's `hook-tok-<i>`, `hook-new-tok-<i>`, `hook-no-tok-<i>`, `hook-<i>` (Revoke).
pub fn controls(data: &Value) -> Vec<(String, String, crate::script::Ctl)> {
    use crate::script::Ctl;
    let mut out = vec![];
    let a = |a: &str, v: &str| Ctl::Bot("act".into(), format!("{a}{SEP}{v}"));
    let nb = &data["newBot"];
    if nb.is_object() {
        for (i, p) in providers(nb).iter().enumerate() { out.push((format!("prov-{i}"), p.label.clone(), a("bfield", &format!("provider{SEP}{}", p.provider)))); }
        out.push(("form-go".into(), "Create".into(), a("bot-create", ""))); out.push(("form-cancel".into(), "Cancel".into(), a("form-close", "@bnew")));
    }
    let bot = &data["bot"];
    if bot["kind"] != "bot" { return out }
    for (k, key) in [("btab", "tabs"), ("bsub", "subtabs")] { for (i, t) in tabs(bot, key).into_iter().enumerate() { out.push((format!("{k}-{i}"), t.label, a("bot-tab", &t.id))); } }
    if bot["tab"] == "hooks" {
        out.push(("hook-new".into(), "New webhook".into(), Ctl::Bot("hook-new".into(), string(bot, "id"))));
        for (i, h) in arr(bot, "hooks").enumerate() {
            let id = string(h, "id");
            for (c, label, op) in [("hook-tok", "Token", "show"), ("hook-new-tok", "New token", "new"), ("hook-no-tok", "No token", "off")] { out.push((format!("{c}-{i}"), label.into(), Ctl::Bot("hook-token".into(), format!("{op}{SEP}{id}")))); }
            out.push((format!("hook-{i}"), "Revoke".into(), a("hook-revoke", &id)));
        }
    }
    out
}
/// A registered control's press: what its element's click does.
pub fn press(this: &mut Backplane, kind: &str, v: &str, cx: &mut Context<Backplane>) {
    let (x, y) = v.split_once(SEP).unwrap_or((v, ""));
    match kind {
        "act" => { super::settings::flush_fields(this); let _ = this.tx.try_send(act(x, y)); cx.notify(); }
        "hook-new" => new_hook(this, v, cx),
        "hook-token" => hook_token(this, y, x, cx),
        _ => {}
    }
}
/// The Webhooks tab: a name and New webhook, what the last request revealed (a secret is shown once), then each hook with its token controls and Revoke.
fn hooks_page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let bot_id = string(bot, "id");
    let mut col = div().flex().flex_col();
    col = col.child(div().flex().gap_2().items_center().px_3().py_2().border_b_1().border_color(c("line"))
        .children(field_box(light, fields, "hk.name")).child(chip(cx, light, "hook-new".into(), "New webhook", true, move |t, cx| new_hook(t, &bot_id, cx))));
    let r = REVEAL.lock().unwrap().clone();
    if r.is_object() {
        let line = |k: &str, v: String| div().flex().gap_2().child(div().w(px(80.)).text_color(c("dim")).child(k.to_string())).child(div().child(v));
        let mut box_ = div().flex().flex_col().gap_1().px_3().py_2().border_b_1().border_color(c("line")).bg(c("raise")).text_size(px(12.));
        if let Some(e) = r["error"].as_str() { box_ = box_.child(div().child(e.to_string())); }
        if let Some(s) = r["secret"].as_str() { box_ = box_.child(div().font_weight(FontWeight::BOLD).child("Webhook made. Keep its secret: it is shown only now.")).child(line("Secret", s.into())).child(line("Path", string(&r, "path"))); }
        if r["tokenHook"].is_string() { box_ = box_.child(line("Token", if string(&r, "token").is_empty() { "off".into() } else { string(&r, "token") })).child(line("Post to", string(&r, "tokenUrl"))); }
        col = col.child(box_);
    }
    let items: Vec<&Value> = arr(bot, "hooks").collect();
    if items.is_empty() { return col.child(div().p_3().text_color(c("dim")).child("No hooks.")).into_any_element(); }
    col.children(items.into_iter().enumerate().map(|(i, h)| {
        let (a, b, d) = (string(h, "id"), string(h, "id"), string(h, "id"));
        div().flex().gap_2().items_center().px_3().py_2().border_b_1().border_color(c("line"))
            .child(div().font_weight(FontWeight::BOLD).child(string(h, "name"))).child(div().text_size(px(11.)).text_color(c("dim")).child(string(h, "path")))
            .child(div().flex_1()).child(div().text_size(px(11.)).text_color(c("faint")).child(format!("{} calls", h["count"])))
            .child(chip(cx, light, format!("hook-tok-{i}"), "Token", false, move |t, cx| hook_token(t, &a, "show", cx)))
            .child(chip(cx, light, format!("hook-new-tok-{i}"), "New token", false, move |t, cx| hook_token(t, &b, "new", cx)))
            .child(chip(cx, light, format!("hook-no-tok-{i}"), "No token", false, move |t, cx| hook_token(t, &d, "off", cx)))
            .child(btn(cx, light, format!("hook-{i}"), "Revoke".into(), false, hook_revoke, string(h, "id"))).into_any_element()
    }).collect::<Vec<_>>()).into_any_element()
}

fn talk_page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value, field: &'static str, action: &'static str) -> AnyElement {
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
            .child(div().id("talk-send").px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")).child("Send").on_click(cx.listener(move |this, _, _, cx| send_post(this, field, action, cx))))).into_any_element()
}

/// The page under a bot's tab strip, or a room's or remote bot's whole page; None for a bot's chat (the thread view) and for no bot.
pub fn page(cx: &mut Context<Backplane>, light: bool, fields: &HashMap<String, FieldSlot>, bot: &Value, browser: Option<&Entity<crate::viewers::browser::BrowserPage>>) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let kind = bot["kind"].as_str().unwrap_or("");
    let header = |t: String| div().h(px(36.)).flex_shrink_0().px_3().flex().items_center().border_b_1().border_color(c("line")).font_weight(FontWeight::BOLD).child(t);
    let frame = |inner: AnyElement, top: f32, header: Option<Div>| div().id("bot-page").absolute().top(px(top)).left_0().right_0().bottom_0().occlude().bg(c("bg")).flex().flex_col().children(header).child(div().id("bot-body").flex_1().min_h_0().overflow_y_scroll().child(inner)).into_any_element();
    match kind {
        "room" => Some(div().id("room-page").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().child(header(bot["title"].as_str().map(str::to_string).unwrap_or_else(|| string(bot, "name")))).child(div().flex_1().min_h_0().child(talk_page(cx, light, fields, bot, "rpost", "room-post"))).into_any_element()),
        "remote" => Some(div().id("remote-page").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().child(header(bot["title"].as_str().map(str::to_string).unwrap_or_else(|| string(bot, "name")))).child(div().flex_1().min_h_0().child(talk_page(cx, light, fields, bot, "rtext", "remote-send"))).into_any_element()),
        "bot" => {
            let tab = bot["tab"].as_str().unwrap_or("chat");
            let inner = match tab {
                "chat" => return None,
                "memory" => memory_page(cx, light, bot),
                "routines" => routines_page(cx, light, fields, bot),
                "hooks" => hooks_page(cx, light, fields, bot),
                "status" => status_page(light, bot),
                "mail" => mail_page(light, bot),
                "space" => space_page(cx, light, fields, bot),
                "settings" => settings_page(cx, light, fields, bot),
                // the bot's page as the hub last drew it (a picture, polled while shown); the person's clicks and keys into it, teach mode and the task runs are not here yet
                "browser" => match browser { Some(b) => div().size_full().child(b.clone()).into_any_element(), None => note(light, "Connecting to the bot's browser page.") },
                "tasks" => tasks_page(cx, light, fields, bot),
                other => div().p_3().text_color(c("dim")).child(format!("The {other} page is not drawn here yet.")).into_any_element(),
            };
            Some(frame(inner, bar_h(bot), None))
        }
        _ => None,
    }
}

#[cfg(test)]
mod hook_tests {
    use super::hook_answer; use core::prelude::v1::test;
    use serde_json::json;
    #[test] fn a_token_reply_is_read_flat_as_the_hub_sends_it() {
        let a = hook_answer(&json!({"t":"reply","id":7,"ok":true,"tokenHook":"h1","token":"tok","tokenUrl":"http://hub/hook/h1"}));
        assert_eq!(a, json!({"tokenHook":"h1","token":"tok","tokenUrl":"http://hub/hook/h1"}));
        assert_eq!(hook_answer(&json!({"t":"reply","id":7,"ok":true,"secret":"s1","secretFor":"h1"}))["secret"], "s1");
        assert_eq!(hook_answer(&json!({"t":"reply","id":7,"ok":true,"result":{"token":"x"}}))["token"], "x");
    }
    #[test] fn the_webhooks_tab_and_the_bot_form_are_registered_controls() {
        use crate::script::Ctl;
        let d = json!({"bot":{"kind":"bot","id":"b1","tab":"hooks","tabs":[{"id":"chat","label":"Chat"},{"id":"settings","label":"Settings"}],"subtabs":[{"id":"hooks","label":"Webhooks"}],"hooks":[{"id":"h1","name":"ci","path":"/hook/h1"}]}});
        let c = super::controls(&d);
        let find = |id: &str| c.iter().find(|(i, _, _)| i == id).map(|(_, l, k)| (l.clone(), k.clone()));
        assert!(matches!(find("bsub-0"), Some((l, Ctl::Bot(k, v))) if l == "Webhooks" && k == "act" && v == "bot-tab\u{1f}hooks"));
        assert!(matches!(find("hook-new"), Some((_, Ctl::Bot(k, v))) if k == "hook-new" && v == "b1"));
        assert!(matches!(find("hook-new-tok-0"), Some((l, Ctl::Bot(k, v))) if l == "New token" && k == "hook-token" && v == "new\u{1f}h1"));
        assert!(matches!(find("hook-0"), Some((l, _)) if l == "Revoke"));
        assert!(find("form-go").is_none(), "no form open");
        let f = super::controls(&json!({"newBot":{"providers":[{"label":"Claude","provider":"claude","on":true}]}}));
        assert_eq!(f.iter().map(|(i, _, _)| i.as_str()).collect::<Vec<_>>(), ["prov-0", "form-go", "form-cancel"]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn the_routine_form_shows_the_hubs_reason_when_there_is_one() {
        assert_eq!(routine_error(&j(r#"{"open":true,"error":"I can't read that time."}"#)).as_deref(), Some("I can't read that time."));
        assert_eq!(routine_error(&j(r#"{"open":true,"error":""}"#)), None); assert_eq!(routine_error(&j(r#"{"open":true}"#)), None);
    }
    #[test] fn the_hooks_tab_asks_for_a_name_field() {
        let d = j(r#"{"bot":{"kind":"bot","id":"b","tab":"hooks","hooks":[]}}"#); assert_eq!(form_fields(&d).iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["hk.name"]);
    }
    #[test] fn a_tasks_run_form_asks_for_a_field_per_parameter() {
        let d = j(r#"{"bot":{"kind":"bot","id":"b","tab":"tasks","tasks":[{"slug":"login","name":"Log in","params":[{"name":"user","label":"User","example":"ada"},{"name":"pw","label":"Password","example":""}]}]}}"#);
        let f = form_fields(&d); assert_eq!(f.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["tk.login.user", "tk.login.pw"]); assert_eq!(f[0].hint, "User (e.g. ada)"); assert_eq!(f[1].hint, "Password"); assert!(f.iter().all(|f| f.vault));
    }
    #[test] fn forms_and_boxes_ask_for_the_fields_they_show() {
        let d = j(r#"{"newBot":{"name":"","hint":"Name","persona":"","provider":"claude","providers":[{"label":"Claude","provider":"claude","on":true}]},"newRoom":null,"bot":{"kind":"room","id":"r1"}}"#);
        assert_eq!(form_fields(&d).iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["name", "persona", "rpost"]);
        let d = j(r#"{"newRoom":{"name":"x","picks":[]},"bot":{"kind":"remote"}}"#);
        assert_eq!(form_fields(&d).iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["rname", "rtext"]); assert!(form_fields(&Value::Null).is_empty());
        assert!(form_fields(&d).iter().all(|f| !f.vault && !f.secret));
    }
    #[test] fn tabs_posts_picks_and_providers_parse_from_the_hubs_rows() {
        let b = j(r#"{"kind":"bot","tabs":[{"id":"chat","label":"Chat"},{"id":"memory","label":"Memory"}],"subtabs":[{"id":"routines","label":"Routines"}],"posts":[{"id":"p1","from":"Kit","text":"hi","ago":"1m","mine":false},{"id":"p2","from":"","text":"yo","ago":"now","mine":true}]}"#);
        assert_eq!(tabs(&b, "tabs").len(), 2); assert_eq!(bar_h(&b), BAR_H + 36.); assert_eq!(bar_h(&j(r#"{"subtabs":[]}"#)), BAR_H); assert_eq!(tabs(&b, "subtabs")[0].id, "routines");
        let p = posts(&b); assert_eq!((p[0].from.as_str(), p[1].mine), ("Kit", true));
        let f = j(r#"{"picks":[{"id":"b1","name":"Kit","on":true},{"id":"b2","name":"Mochi","on":false}],"providers":[{"label":"Claude","provider":"claude","on":false}]}"#);
        assert!(picks(&f)[0].on && !picks(&f)[1].on); assert_eq!(providers(&f)[0].provider, "claude");
    }
    #[test] fn focus_and_escape_follow_the_open_form_or_page() {
        assert_eq!(focus_target(&j(r#"{"newBot":{}}"#)), "name"); assert_eq!(focus_target(&j(r#"{"newRoom":{}}"#)), "rname");
        assert_eq!(focus_target(&j(r#"{"bot":{"kind":"room"}}"#)), "rpost"); assert_eq!(focus_target(&j(r#"{"bot":{"kind":"remote"}}"#)), "rtext"); assert_eq!(focus_target(&j(r#"{"bot":{"kind":"bot"}}"#)), "");
        assert!(matches!(close_form(&j(r#"{"newBot":{}}"#)), Some(Command::Action(a, v)) if a == "form-close" && v == "@bnew"));
        assert!(matches!(close_form(&j(r#"{"newRoom":{}}"#)), Some(Command::Action(a, v)) if v == "@rnew")); assert!(close_form(&j(r#"{"bot":{"kind":"room"}}"#)).is_none());
    }
    #[test] fn a_spaces_inputs_are_found_even_in_rows() {
        let mut o = Vec::new(); space_inputs(&j(r#"[{"type":"input","label":"Name","action":"n"},{"type":"row","blocks":[{"type":"input","label":"Q","action":"q"},{"type":"text"}]}]"#), &mut o);
        assert_eq!(o, vec![("n".to_string(), "Name".to_string()), ("q".to_string(), "Q".to_string())]);
        let f = form_fields(&j(r#"{"bot":{"kind":"bot","space":{"blocks":[{"type":"input","label":"Name","action":"n"}]}}}"#)); assert_eq!((f[0].name.as_str(), f[0].vault), ("sp.n", true));
    }
    #[test] fn presses_are_the_phones_actions() {
        for (c, a, v) in [(tab_cmd("memory"), "bot-tab", "memory"), (pick_cmd("b1"), "room-pick", "b1"), (forget_cmd("k"), "mem-forget", "k"), (routine_toggle("r"), "routine-toggle", "r"), (hook_revoke("h"), "hook-revoke", "h"), (close_bot_form(""), "form-close", "@bnew"), (close_room_form(""), "form-close", "@rnew"), (provider_cmd("claude"), "bfield", "provider\u{1f}claude")] {
            match c { Command::Action(x, y) => assert_eq!((x.as_str(), y.as_str()), (a, v)), c => panic!("{c:?}") }
        }
    }
}

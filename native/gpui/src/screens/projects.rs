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
    /// The project page's own reads: `project.home` (the folder's goal and notes, board and files) and `fab.look {project}` (the parts its fab files name).
    pub home: Home,
    /// The page's Edit: (the thread it opened, the text for its composer), put there once that thread is on show.
    pub intent_edit: Option<(String, String)>,
    /// Send on a project's page (0.14's home composer): (the thread on show before, the message, when), sent once the new thread the page
    /// asked for is the one on show; back in the box if none came.
    pub first_send: Option<(String, String, std::time::Instant)>,
}

/// What Edit puts in the composer: `/intent` and the goal and notes (the hub's `intent_edit`; its Send saves them), the template for none.
/* i18n-ok: the intent file's headings, sent to the hub */ pub fn intent_draft(intent: &str) -> String { format!("/intent\n{}", if intent.trim().is_empty() { "# Goal\n\n# Parts\n\n# Notes\n" } else { intent }) }

/// What the hub said of the project on show (asked when its page opens; an answer for another project is dropped).
#[derive(Default)]
pub struct Home { pub pid: String, pub snap: Value, pub card: Value, seq: u64, asks: Vec<(u64, &'static str, String)> }
/// Ask tags of the project page (far above the Improvements page's own).
const TAG: u64 = 1 << 48;
/// The page opened (or opened again): its two reads go out; what the last answers said stays until the new ones come.
pub fn ask_home(this: &mut Backplane, pid: &str) {
    let h = &mut this.screens.projects.home;
    if h.pid != pid { h.snap = Value::Null; h.card = Value::Null; h.pid = pid.to_string(); }
    for (m, p) in [("project.home", serde_json::json!({"project": pid})), ("fab.look", serde_json::json!({"project": pid}))] {
        let h = &mut this.screens.projects.home; h.seq += 1; let tag = TAG + h.seq;
        h.asks.push((tag, m, pid.to_string())); if h.asks.len() > 16 { h.asks.remove(0); }
        let _ = this.tx.try_send(Command::Ask(m.to_string(), p, tag));
    }
}
/// An answer to one of the page's reads (false: not the page's).
pub fn answered(this: &mut Backplane, tag: u64, reply: &Value, cx: &mut Context<Backplane>) -> bool {
    if tag >= super::palette::TAG { return super::palette::answered(this, tag, reply, cx) } // the palette's searches
    if tag < TAG { return false }
    let h = &mut this.screens.projects.home;
    let Some(i) = h.asks.iter().position(|a| a.0 == tag) else { return true };
    let (_, m, pid) = h.asks.remove(i);
    if pid != h.pid || reply["ok"] != true { return true }
    let r = reply["result"].clone();
    if m == "project.home" { if r.get("artifacts").is_some() { h.snap = r; } } else { h.card = r; }
    cx.notify(); true
}
/// The goal and notes as the person wrote them (`.backplane/intent.md`), a line each: (kind, text): "h" a heading, "li" a list item, "p" text.
pub fn intent_lines(text: &str) -> Vec<(&'static str, String)> {
    text.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).take(60).map(|l| {
        let t = l.trim_start();
        if let Some(h) = t.strip_prefix('#') { ("h", h.trim_start_matches('#').trim().to_string()) }
        else if let Some(i) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) { ("li", i.trim().to_string()) }
        else { ("p", t.to_string()) }
    }).collect()
}
/// The boards the project has: the board and schematic the hub found (`project.home`'s artifacts), else the card's board files.
pub fn boards(snap: &Value, card: &Value, root: &str) -> Vec<String> {
    let rel = |p: &str| p.strip_prefix(root).map(|r| r.trim_start_matches('/').to_string()).unwrap_or_else(|| p.to_string());
    let mut out: Vec<String> = snap["artifacts"].as_array().into_iter().flatten().filter(|a| a["kind"] == "board").map(|a| rel(a["path"].as_str().unwrap_or(""))).filter(|p| !p.is_empty()).collect();
    for b in card["board_files"].as_array().into_iter().flatten().filter_map(Value::as_str) { if !out.iter().any(|o| o == b) { out.push(b.to_string()); } }
    out.truncate(12); out
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
/* i18n-ok: the hint, drawn through trs by the composer */ pub fn hint(data: &Value) -> String { let h = string(&data["search"], "hint"); if h.is_empty() { "Search projects".into() } else { h } }

pub fn search_cmd(text: &str) -> Command { Command::Action("proj-find-q".into(), text.to_string()) }
pub fn go_cmd() -> Command { Command::Action("proj-go".into(), String::new()) }
pub fn open_cmd() -> Command { Command::Action("picker-open".into(), String::new()) }
pub fn type_cmd(text: &str) -> Command { Command::Action("picker-type".into(), text.to_string()) }
/// Enter in the path field: the Add or Create row of the hub's rows, when they were made from the text the field holds (else none: a row for an
/// older text would add the wrong folder).
pub fn submit_cmd(data: &Value, typed: &str) -> Option<Command> {
    let f = folders(data)?;
    if f.text != typed { return None }
    f.items.into_iter().find(|i| i.kind == "add" && !i.action.is_empty()).map(|i| Command::Action(i.action, i.value))
}
pub fn close_cmd() -> Command { Command::Action("proj-close".into(), String::new()) }
/// Esc leaves the folder picker while it is open.
pub fn close_on_escape(data: &Value) -> Option<Command> { data["folders"].is_object().then(close_cmd) }
/// Whether the hub's text replaces what is in the box: only a real change that is not one of the last texts this box sent.
pub fn adopt(hub: &str, typed: &str, sent: &[String]) -> bool { hub != typed && !sent.iter().any(|s| s == hub) }
fn remember(v: &mut Vec<String>, t: &str) { v.push(t.to_string()); if v.len() > 16 { v.remove(0); } }

pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    // Send on the page asked for a new thread in the project: once it is on show, the message goes to it (or back in the box after 10 s)
    if let Some((before, text, at)) = this.screens.projects.first_send.clone() {
        let shown = string(&this.data["thread"], "id"); let shown = shown.rsplit('|').next().unwrap_or("").to_string();
        if this.arm_new.is_none() && !shown.is_empty() && shown != before && this.selected.rsplit('|').next() == Some(shown.as_str()) && this.data["thread"]["loading"] != true {
            this.screens.projects.first_send = None;
            let _ = this.tx.try_send(Command::Send(text));
        } else if at.elapsed() > std::time::Duration::from_secs(10) {
            this.screens.projects.first_send = None;
            this.input.update(cx, |i, cx| i.set(text, cx));
        }
    }
    // the page's Edit opened its thread: once that thread is on show, the goal and notes go in its composer
    if let Some((th, text)) = this.screens.projects.intent_edit.clone() {
        let shown = string(&this.data["thread"], "id"); let shown = shown.rsplit('|').next().unwrap_or("").to_string();
        if shown == th && this.selected.rsplit('|').next() == Some(th.as_str()) {
            this.screens.projects.intent_edit = None;
            this.input.update(cx, |i, cx| i.set(text.clone(), cx));
            let _ = this.tx.try_send(Command::Draft(text));
        }
    }
    if this.screens.projects.search.is_none() {
        let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        cx.observe(&input, |this, input, cx| {
            let text = input.read(cx).content.to_string(); let p = &mut this.screens.projects;
            if p.echo_q.as_deref() == Some(text.as_str()) { p.echo_q = None; return; }
            if p.sent_q.last() == Some(&text) { return; }
            remember(&mut p.sent_q, &text); let _ = this.tx.try_send(search_cmd(&text));
        }).detach();
        // Enter: the hub opens what was found and clears its query; the box forgets what it sent, so that clear is taken (an older "" sent is no echo)
        cx.subscribe(&input, |this, _, ev: &ComposerEvent, _| if matches!(ev, ComposerEvent::Submit) { this.screens.projects.sent_q.clear(); let _ = this.tx.try_send(go_cmd()); }).detach();
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
                // Enter: the picker's Add or Create row, as a click on it would (only once the hub's rows are for what the field holds)
                cx.subscribe(&input, |this, input, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) {
                    if let Some(c) = submit_cmd(&this.data, &input.read(cx).content.to_string()) { this.arm_new(); let _ = this.tx.try_send(c); cx.notify(); }
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
        list = list.child(crate::ui::with_rect(div().id(SharedString::from(format!("folder-{i}"))), move || format!("folder-{i}")).flex().gap_2().px_2().py_1().text_size(px(12.)).text_color(c(if live { "text" } else { "faint" })).when(live, |d| d.cursor_pointer().hover(|s| s.bg(c("raise"))))
            .child(div().w(px(14.)).text_color(c("dim")).child(icon)).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(crate::i18n::trs(&it.label)))
            .on_click(cx.listener(move |this, _, _, cx| { if !a.is_empty() { if a == "proj-add" || a == "proj-new" { this.arm_new(); } let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); } cx.notify(); })));
    }
    Some(div().id("folders-q").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().items_start().p_4().gap_3()
        .child(div().font_weight(FontWeight::BOLD).child(crate::i18n::trs("Add project")))
        .children(p.folder.clone().map(|i| div().w_full().max_w(px(640.)).px_2().py_1().border_1().border_color(c("line")).bg(c("view")).child(i)))
        .when(!f.error.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(c("bad")).child(crate::i18n::trs(&f.error))))
        .child(list)
        .child(crate::ui::with_rect(div().id("folders-cancel"), || "folders-cancel".to_string()).px_3().py_1().text_size(px(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).child(crate::i18n::trs("Cancel")).on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(close_cmd()); cx.notify(); })))
        .into_any_element())
}

/// The top bar's place while a project's page is on show: the project's name (its page stands for the selection, not the last thread).
pub fn page_title(data: &Value, page: Option<&str>) -> Option<String> { page.and_then(|p| find(data, p)).map(|p| string(p, "title")) }
/// Whether an action sent to the hub goes somewhere (a bot's chat, a room, a link, a thread): the project's page on show gives way to it.
/// The page is this window's own; the hub's selection may not move at all (the uberbot's chat was already selected under the page), so
/// a click on the uberbot after a project page left the page up.
pub fn leaves_page(action: &str) -> bool { matches!(action, "bot" | "room" | "link-open" | "select") }
/// The sidebar's project with this (raw) id.
pub fn find<'a>(data: &'a Value, pid: &str) -> Option<&'a Value> {
    data["projects"].as_array()?.iter().find(|p| string(p, "id").rsplit('|').next() == Some(pid))
}
/// A project's page, from the sidebar's own rows (no round trip): its name and folder, New thread, and each thread that is in it.
pub fn page(cx: &mut Context<Backplane>, light: bool, data: &Value, home: &Home, pid: &str, ctl: &mut Vec<crate::script::CtlEntry>) -> Vec<AnyElement> {
    use crate::script::CtlEntry;
    let Some(p) = find(data, pid) else { return vec![] };
    let c = |n: &str| theme::color(light, n);
    let threads: Vec<&Value> = p["threads"].as_array().into_iter().flatten().chain(p["settled"].as_array().into_iter().flatten()).collect();
    let mut out: Vec<AnyElement> = vec![
        div().text_size(px(22.)).font_weight(FontWeight::BOLD).child(string(p, "title")).into_any_element(),
        div().text_size(px(12.)).text_color(c("dim")).child(string(p, "root")).into_any_element(),
    ];
    // what the folder holds (the hub's reads): the goal and notes, the parts to order and the boards (no Ready to order bar and no Buy it here: Buy it is a thread's tool)
    let (hp, snap, card) = (home.pid.clone(), home.snap.clone(), home.card.clone());
    if hp == pid {
        let root = string(&snap, "root"); let root = if root.is_empty() { string(p, "root") } else { root };
        let intent = { let i = string(&card, "intent"); if i.is_empty() && snap["artifacts"].as_array().map_or(false, |a| a.iter().any(|x| x["kind"] == "intent")) { string(&snap, "intent") } else { i } };
        out.push(div().pt_3().text_size(px(11.)).text_color(c("faint")).child(crate::i18n::trs("GOAL & NOTES")).into_any_element());
        if intent.trim().is_empty() { out.push(div().text_color(c("dim")).child(crate::i18n::trs("No goal yet. Ask an agent to write what this project is for, or edit .backplane/intent.md.")).into_any_element()); }
        let mut col = div().id("home-intent").flex().flex_col().gap_1().text_size(px(13.));
        for (k, t) in intent_lines(&intent) {
            col = col.child(match k { "h" => div().pt_1().font_weight(FontWeight::BOLD).child(t), "li" => div().flex().gap_2().child(div().text_color(c("dim")).child("•")).child(div().flex_1().min_w_0().child(t)), _ => div().child(t) });
        }
        out.push(col.into_any_element());
        // Edit: the project's top thread opens with the goal and notes in its composer as `/intent ...`; Send saves them (0.14 Intent's Edit)
        if let Some(top) = threads.first().map(|t| string(t, "id")) {
            let text = intent_draft(&intent);
            /* i18n-ok: ctl labels and ui: ids are what drivers press */ ctl.push(crate::script::CtlEntry { id: "home-intent-edit".into(), label: "Edit goal & notes".into(), ctl: Ctl::Select(top.clone()) });
            /* i18n-ok: ctl labels and ui: ids are what drivers press */ out.push(crate::ui::with_rect(div().id("home-intent-edit"), || "ui:Edit goal & notes".to_string()).px_3().py_1().w(px(160.)).border_1().border_color(c("line")).bg(c("raise")).text_size(px(12.)).cursor_pointer().hover(|s| s.bg(c("panel"))).child(crate::i18n::trs("Edit goal & notes"))
                .on_click(cx.listener(move |this, _, window, cx| { let raw = top.rsplit('|').next().unwrap_or("").to_string(); this.screens.projects.intent_edit = Some((raw, text.clone())); this.select(top.clone(), window, cx); })).into_any_element());
        }
        let parts: Vec<&Value> = card["parts"].as_array().into_iter().flatten().collect();
        if !parts.is_empty() {
            out.push(div().pt_3().text_size(px(11.)).text_color(c("faint")).child(crate::i18n::tf("gpui.projects.to.order", &[("n", parts.len().into())])).into_any_element());
            for (i, x) in parts.iter().enumerate() {
                out.push(div().id(SharedString::from(format!("home-part-{i}"))).flex().gap_2().px_2().h(px(26.)).items_center().border_b_1().border_color(c("line")).text_size(px(12.))
                    .child(div().flex_1().child(string(x, "name"))).child(div().text_color(c("dim")).child(string(x, "kind"))).child(div().text_color(c("faint")).child(string(x, "path"))).into_any_element());
            }
        }
        let bs = boards(&snap, &card, &root);
        if !bs.is_empty() {
            out.push(div().pt_3().text_size(px(11.)).text_color(c("faint")).child(crate::i18n::tf("gpui.projects.boards", &[("n", bs.len().into())])).into_any_element());
            for (i, b) in bs.iter().enumerate() { out.push(div().id(SharedString::from(format!("home-board-{i}"))).px_2().h(px(26.)).flex().items_center().border_b_1().border_color(c("line")).text_size(px(12.)).child(b.clone()).into_any_element()); }
        }
    }
    let pid2 = pid.to_string();
    /* i18n-ok: ctl labels and ui: ids are what drivers press */ ctl.push(CtlEntry { id: "page-new-thread".into(), label: "New thread".into(), ctl: Ctl::Node("new-thread".into(), pid.to_string()) });
    out.push(div().id("page-new-thread").px_3().py_1().w(px(140.)).border_1().border_color(c("line")).bg(c("raise")).text_size(px(12.)).cursor_pointer().hover(|s| s.bg(c("panel"))).child(crate::i18n::trs("New thread"))
        .on_click(cx.listener(move |this, _, _, cx| { this.arm_new(); let _ = this.tx.try_send(Command::Action("new-thread".into(), pid2.clone())); cx.notify(); })).into_any_element());
    out.push(div().pt_3().text_size(px(11.)).text_color(c("faint")).child(crate::i18n::tf("gpui.projects.threads", &[("n", threads.len().into())])).into_any_element());
    if threads.is_empty() { out.push(div().text_color(c("dim")).child(crate::i18n::trs("No threads yet.")).into_any_element()); }
    for t in threads {
        let id = string(t, "id"); let raw = id.rsplit('|').next().unwrap_or(&id).to_string();
        ctl.push(CtlEntry { id: format!("page-thread:{raw}"), label: string(t, "title"), ctl: Ctl::Select(id.clone()) });
        out.push(div().id(SharedString::from(format!("page-thread-{raw}"))).flex().items_center().gap_2().px_2().h(px(30.)).border_b_1().border_color(c("line")).cursor_pointer().hover(|s| s.bg(c("raise")))
            .child(super::siderow::dot(light, t)).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(string(t, "title")))
            .on_click(cx.listener(move |this, _, window, cx| this.select(id.clone(), window, cx))).into_any_element());
    }
    out
}

/// The controls these draw, for the harness.
pub fn controls(data: &Value) -> Vec<(String, String, Ctl)> {
    /* i18n-ok: ctl labels and ui: ids are what drivers press */ let mut out = vec![("ui:Add project".to_string(), "Add project".to_string(), Ctl::Node("picker-open".into(), String::new()))];
    if let Some(f) = folders(data) {
        for (i, it) in f.items.iter().enumerate() { if !it.action.is_empty() { out.push((format!("folder-{i}"), it.label.clone(), Ctl::Node(it.action.clone(), it.value.clone()))); } }
        /* i18n-ok: ctl labels and ui: ids are what drivers press */ out.push(("folders-cancel".into(), "Cancel".into(), Ctl::Node("proj-close".into(), String::new())));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test] fn going_to_a_bot_a_room_a_link_or_a_thread_leaves_the_project_page() {
        for a in ["bot", "room", "link-open", "select"] { assert!(super::leaves_page(a), "{a}"); }
        for a in ["flag", "new-thread", "proj-find-q", ""] { assert!(!super::leaves_page(a), "{a}"); }
    }
    use super::*; use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn the_hubs_folder_picker_parses_and_closes_on_escape() {
        let d = j(r#"{"folders":{"text":"~/Doc","hint":"Path","error":"","items":[{"label":"Documents","action":"proj-cd","value":"~/Documents/","kind":"dir"},{"label":"Add ~/Doc","action":"proj-add","value":"~/Doc","kind":"add"}]}}"#);
        let f = folders(&d).unwrap(); assert_eq!(f.items.len(), 2); assert_eq!(f.items[1].action, "proj-add");
        assert!(matches!(submit_cmd(&d, "~/Doc"), Some(Command::Action(a, v)) if a == "proj-add" && v == "~/Doc"), "Enter adds what the rows were made for");
        assert!(submit_cmd(&d, "~/Docs").is_none(), "rows for an older text: Enter waits"); assert!(matches!(close_on_escape(&d), Some(Command::Action(a, _)) if a == "proj-close"));
        assert!(folders(&Value::Null).is_none() && close_on_escape(&Value::Null).is_none());
        assert_eq!(controls(&d).iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["ui:Add project", "folder-0", "folder-1", "folders-cancel"]);
    }
    #[test] fn a_project_is_found_by_its_raw_id_with_or_without_the_hub_prefix() {
        let d = j(r#"{"projects":[{"id":"http://h|p1","title":"A","threads":[{"id":"http://h|t1","title":"T"}]},{"id":"p2","title":"B"}]}"#);
        assert_eq!(string(find(&d, "p1").unwrap(), "title"), "A"); assert_eq!(string(find(&d, "p2").unwrap(), "title"), "B"); assert!(find(&d, "p3").is_none() && find(&Value::Null, "p1").is_none());
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

#[cfg(test)]
mod home_tests {
    use super::*; use serde_json::json; use core::prelude::v1::test;
    #[test] fn edit_puts_the_goal_and_notes_in_the_composer_as_intent() {
        assert_eq!(intent_draft("# Goal\nlamp"), "/intent\n# Goal\nlamp"); assert!(intent_draft("  ").starts_with("/intent\n# Goal\n"));
    }
    #[test] fn the_goal_and_notes_are_drawn_a_line_each() {
        assert_eq!(intent_lines("# Goal\n\nA lamp.\n\n# Parts\n\n- LED board\n* Shade\n## Notes"), vec![("h", "Goal".into()), ("p", "A lamp.".into()), ("h", "Parts".into()), ("li", "LED board".into()), ("li", "Shade".into()), ("h", "Notes".into())]);
    }
    #[test] fn the_boards_are_the_folders_board_then_the_cards_board_files() {
        let snap = json!({"artifacts": [{"kind": "board", "path": "/p/board.kicad_pcb"}, {"kind": "intent", "path": "/p/.backplane/intent.md"}]});
        assert_eq!(boards(&snap, &json!({"board_files": ["board.kicad_pcb", "boards/b/b.kicad_pcb"]}), "/p"), vec!["board.kicad_pcb".to_string(), "boards/b/b.kicad_pcb".into()]);
        assert!(boards(&Value::Null, &Value::Null, "/p").is_empty());
    }
}

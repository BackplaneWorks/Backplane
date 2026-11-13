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
    pub focus: Option<&'static str>, folder_open: bool, /// What the folder field held before its last change, whoever made it (`back`).
    prev_f: String,
    /// The picker's row under the arrow keys (an index into the hub's rows; none: the field, where Enter is Add or Create), and its list's scroll.
    pub folder_ix: Option<usize>, pub folder_scroll: ScrollHandle,
    /// The project page's own reads: `project.home` (the folder's goal and notes, board and files) and `fab.look {project}` (the parts its fab files name).
    pub home: Home,
    /// The page's Edit: (the thread it opened, the text for its composer), put there once that thread is on show.
    pub intent_edit: Option<(String, String)>,
    /// The page's viewer buttons: (the thread it opened, the view kind), sent as `view-kind` once that thread is on show.
    pub view_edit: Option<(String, &'static str)>,
    /// Send on a project's page (0.14's home composer): (the thread on show before, the message, when), sent once the new thread the page
    /// asked for is the one on show; back in the box if none came.
    pub first_send: Option<(String, String, std::time::Instant)>,
    /// Text a page's Send could not deliver, while the box holds something else: shown over the box with Restore (never merged into it)
    pub restore: Option<String>,
    /// The thread a New thread's reply named (raw id, `<link>~<id>` for a linked machine's), opened once the lists hold it
    pub made: Option<(String, std::time::Instant)>,
    /// A first message on its way to the thread just made (kept until its answer, so a refusal gives it back)
    pub sending: Option<String>,
    /// What the hub said when it would not make a page's New thread (its machine away, not linked...): drawn by the box until the next Send
    /// (kept apart from the window's `error`, which a view that says Connected clears)
    pub refused: String,
    /// What the person picked on each project's runs-on line here (`run-on`): project id -> the target project id (`<link>~<id>` for a linked
    /// machine's). A page's Send never goes to another machine than this (`page_send`)
    pub picked: std::collections::HashMap<String, String>,
}

/// What Edit puts in the composer: `/intent` and the goal and notes (the hub's `intent_edit`; its Send saves them), the template for none.
/* i18n-ok: the intent file's headings, sent to the hub */ pub fn intent_draft(intent: &str) -> String { format!("/intent\n{}", if intent.trim().is_empty() { "# Goal\n\n# Parts\n\n# Notes\n" } else { intent }) }

/// What the hub said of the project on show (asked when its page opens; an answer for another project is dropped).
#[derive(Default)]
pub struct Home { pub pid: String, pub snap: Value, pub card: Value, /// the README's blocks, parsed once when the answer came
    pub readme: Vec<super::md::Block>, seq: u64, asks: Vec<(u64, &'static str, String)> }
/// Ask tags of the project page (far above the Improvements page's own).
const TAG: u64 = 1 << 48;
/// The page opened (or opened again): its two reads go out; what the last answers said stays until the new ones come.
pub fn ask_home(this: &mut Backplane, pid: &str) {
    let h = &mut this.screens.projects.home;
    if h.pid != pid { h.snap = Value::Null; h.card = Value::Null; h.readme.clear(); h.pid = pid.to_string(); }
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
    if m == "project.home" { if r.get("artifacts").is_some() { h.readme = r["readme"].as_array().map(|n| super::md::parse(n)).unwrap_or_default(); h.snap = r; } } else { h.card = r; }
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
pub struct Item { pub label: String, pub action: String, pub value: String, pub kind: String, /// "project", "KiCad", "git" or none: what the folder is or looks like.
    pub tag: String, /// What the field shows on going into the row ("~/x/"); an older hub sends none and the value is shown.
    pub field: String, /// The project this folder is already (its id): a click opens its page.
    pub project: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Folders { pub text: String, pub hint: String, pub error: String, pub items: Vec<Item>, /// Folders that match past the rows sent.
    pub more: u64 }

pub fn folders(data: &Value) -> Option<Folders> {
    let f = data["folders"].as_object()?;
    let s = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let items = f.get("items").and_then(Value::as_array).into_iter().flatten().take(200).map(|i| Item { label: string(i, "label"), action: string(i, "action"), value: string(i, "value"), kind: string(i, "kind"), tag: string(i, "tag"), field: string(i, "field"), project: string(i, "project") }).collect();
    Some(Folders { text: s("text"), hint: s("hint"), error: s("error"), items, more: f.get("more").and_then(Value::as_u64).unwrap_or(0) })
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
/// Send on a project's page: the new thread goes where its "runs on" line says (the picker's choice, the hub's first `newOn` entry: this
/// machine's project id or a linked machine's `<link>~<id>`), and the typed text waits to be its first message once it is on show.
/// `picked`: what the person picked here for this project. The target must be on that machine, or nothing is sent (Err: the words to show,
/// the text stays in the box); the command carries the pick too, and the hub refuses a target on another machine as well.
pub fn page_send(data: &Value, pid: &str, text: &str, picked: Option<&str>) -> Option<Result<(Command, String), String>> {
    if text.trim().is_empty() { return None }
    let p = find(data, pid)?;
    let to = p["newOn"].as_array().and_then(|a| a.iter().find(|o| o["on"] == true && matches!(o["kind"].as_str(), Some("machine") | Some("wait") | None))).and_then(|o| o["value"].as_str()).filter(|v| !v.is_empty()).unwrap_or(pid);
    let machine = |id: &str| id.split_once('~').map(|x| x.0.to_string());
    if let Some(pk) = picked.filter(|p| !p.is_empty()) {
        if machine(to) != machine(pk) { return Some(Err(crate::i18n::trs("Not sent: the machine you picked is not where this would run. Pick it again, or another."))) }
        return Some(Ok((Command::Action("new-thread".into(), format!("{to}\u{1f}{pk}")), text.to_string())))
    }
    Some(Ok((Command::Action("new-thread".into(), to.to_string()), text.to_string())))
}
/// The hub refused the new thread a page's Send asked for: the text goes back in the box (never lost), the window says why.
pub fn take_back(first_send: &mut Option<(String, String, std::time::Instant)>) -> Option<String> { first_send.take().map(|(_, text, _)| text) }
/// A page's Send, once the hub has made its thread (`id` from the reply, `<link>~<id>` on a linked machine): the typed text as that thread's first
/// message, sent by id (never to whatever thread is open).
pub fn first_message(id: &str, text: &str) -> Command {
    let msg = format!("m{}-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()), id.len());
    Command::Rpc("turn.start".into(), crate::screens::settings::Secret(serde_json::json!({"thread": id, "text": text, "msg": msg})), "first-message")
}
/// Where text that came back goes: into the box when it is empty, else held to restore (`restore`), so it is never merged with what is there.
pub fn give_back(text: String, box_now: &str, restore: &mut Option<String>) -> Option<String> {
    if box_now == text { return None } // (still in the box: the send keeps it there until the hub answers)
    if box_now.trim().is_empty() { Some(text) } else { *restore = Some(text); None }
}
/// The rows the arrow keys walk: the folders and "..", never the Add row (that is Enter in the field) or a note.
fn walkable(i: &Item) -> bool { !i.action.is_empty() && i.kind != "add" }
/// Up and Down in the picker: from the field (none) Down takes the first folder row; Up from the first goes back to the field; the ends hold.
/// A row that is gone (the hub's rows changed) counts as the field.
pub fn step(items: &[Item], cur: Option<usize>, d: i32) -> Option<usize> {
    let rows: Vec<usize> = items.iter().enumerate().filter(|(_, i)| walkable(i)).map(|(n, _)| n).collect();
    let at = cur.and_then(|c| rows.iter().position(|r| *r == c));
    match (at, d) {
        (None, d) if d > 0 => rows.first().copied(),
        (None, _) => None,
        (Some(0), d) if d < 0 => None,
        (Some(p), d) => rows.get((p as i32 + d.signum()).clamp(0, rows.len() as i32 - 1) as usize).copied(),
    }
}
/// The row under the arrow keys, when it is still one of the hub's rows.
pub fn marked(f: &Folders, ix: Option<usize>) -> Option<&Item> { ix.and_then(|i| f.items.get(i)).filter(|i| walkable(i)) }
/// What a key in the field does.
#[derive(Debug, PartialEq)]
pub enum Key { /// Go into this folder: the field takes the path (and tells the hub as typing does).
    Cd(String), /// The Add, Create or Clone row.
    Run(String, String), /// The folder is a project already: its page is opened and the picker shut.
    Open(String) }
/// What the field takes on going into a row: its `field` ("~/x/"), else its value.
fn into(i: &Item) -> String { if i.field.is_empty() { i.value.clone() } else { i.field.clone() } }
/// How the hub's rows stand to what the field holds: made from it (`Now`); made in the same folder for another name (`Same`: the name typed now,
/// lower case, by which the rows are narrowed here until the hub answers); or made for another folder (`Away`: the rows are drawn faint).
#[derive(Debug, PartialEq)]
pub enum Fresh { Now, Same(String), Away }
fn folder_of(t: &str) -> &str { &t[..t.rfind('/').map_or(0, |i| i + 1)] }
pub fn fresh(hub: &str, typed: &str) -> Fresh {
    if hub == typed { Fresh::Now } else if folder_of(hub) == folder_of(typed) { Fresh::Same(typed[folder_of(typed).len()..].to_lowercase()) } else { Fresh::Away }
}
/// Whether a row is drawn, and drawn faint, for how the rows stand: narrowed by the name typed now (a hidden one only once a dot is typed), the
/// Add or Create row faint while it is for an older text (Enter does not take it then: `submit_cmd`).
pub fn shows(i: &Item, f: &Fresh) -> (bool, bool) {
    match f {
        Fresh::Now => (true, false),
        Fresh::Away => (true, true),
        Fresh::Same(p) => match i.kind.as_str() {
            "dir" => { let n = i.label.to_lowercase(); (n.starts_with(p.as_str()) && (p.starts_with('.') || !n.starts_with('.')), false) }
            "up" => (true, false),
            _ => (true, true),
        },
    }
}
/// Backspace that took the "/" off the end of a folder goes up to its parent ("~/a/b/" to "~/a/"), as a shell's file prompt does; "~/" and "/" stay.
pub fn back(before: &str, now: &str) -> Option<String> {
    if !before.ends_with('/') || before.len() < 2 || before == "~/" || now != &before[..before.len() - 1] { return None }
    Some(folder_of(now).to_string()).filter(|p| !p.is_empty())
}
/// The footer's keys, each (key, what it does): Enter is named only while a row is marked (else it is on the Add row), Tab only while it has
/// something to complete, the arrows only while there are rows.
pub fn hints(f: &Folders, typed: &str, ix: Option<usize>) -> Vec<(&'static str, &'static str)> {
    let mut out = vec![];
    if f.items.iter().any(walkable) { out.push(("↑↓", "select")); }
    if let Some(i) = marked(f, ix) { out.push(("Enter", if matches!(take(i), Key::Open(_)) { "open project" } else { "go into" })); } // i18n-ok: a key and its words, drawn through trs
    if marked(f, ix).is_some() || (!typed.ends_with('/') && f.items.iter().any(|i| i.kind == "dir")) { out.push(("Tab", "complete")); }  // i18n-ok: a key and its words, drawn through trs
    out
}
/// A label cut to `max` characters from the middle: its first word ("Add") and the end of the path, which is the part that tells folders apart.
pub fn fit(s: &str, max: usize) -> String {
    let n = s.chars().count(); if n <= max || max < 8 { return s.to_string() }
    let head: String = s.split_inclusive(' ').next().filter(|h| h.ends_with(' ') && h.chars().count() < max / 3).unwrap_or("").to_string();
    let keep = max - head.chars().count() - 1;
    format!("{head}…{}", s.chars().skip(n - keep).collect::<String>())
}
/// The marked row once the hub's rows are for what the field holds (until then the mark is on a row of an older list and no key takes it).
fn marked_now<'a>(f: &'a Folders, typed: &str, ix: Option<usize>) -> Option<&'a Item> { if f.text == typed { marked(f, ix) } else { None } }
/// What taking a folder row does: a project's page is opened, any other folder is gone into.
fn take(i: &Item) -> Key { if i.kind == "dir" && !i.project.is_empty() { Key::Open(i.project.clone()) } else { Key::Cd(into(i)) } }
/// Enter: the marked folder is taken; with none marked it is the Add or Create row (`submit_cmd`).
pub fn enter(data: &Value, typed: &str, ix: Option<usize>) -> Option<Key> {
    let f = folders(data)?;
    if let Some(i) = marked_now(&f, typed, ix) { return Some(take(i)) }
    match submit_cmd(data, typed) { Some(Command::Action(a, v)) => Some(Key::Run(a, v)), _ => None }
}
/// Tab: the marked folder (gone into, a project's too), else the first folder that matches the name being typed (none while no name is typed, or while the rows are for an older text).
pub fn complete(data: &Value, typed: &str, ix: Option<usize>) -> Option<String> {
    let f = folders(data)?;
    if let Some(i) = marked_now(&f, typed, ix) { return Some(into(i)) }
    if f.text != typed || typed.ends_with('/') || typed == "~" { return None }
    f.items.iter().find(|i| i.kind == "dir" && !i.action.is_empty()).map(into)
}
/// A click on row `n`, judged when it lands against what the field holds then: a row drawn faint or not drawn (made for an older text or
/// another folder) does nothing, so "Add ~" can never be taken once the field says "~/board/".
pub fn click(data: &Value, typed: &str, n: usize) -> Option<Key> {
    let f = folders(data)?; let i = f.items.get(n)?;
    if i.action.is_empty() || shows(i, &fresh(&f.text, typed)) != (true, false) { return None }
    Some(match i.kind.as_str() { "dir" | "up" if i.action == "proj-cd" => take(i), _ => Key::Run(i.action.clone(), i.value.clone()) })
}
/// Up and Down: only once the rows are for what the field holds.
pub fn walk(data: &Value, typed: &str, ix: Option<usize>, d: i32) -> Option<(Option<usize>, usize)> {
    let f = folders(data)?; if f.text != typed { return None }
    let to = step(&f.items, ix, d);
    Some((to, to.map_or(0, |t| f.items[..t].iter().filter(|i| i.kind != "add" && i.kind != "note").count())))
}
/// Going into a folder from here: the field shows the path at once and its change goes to the hub as typing does (no wait for the hub's echo).
fn cd(this: &mut Backplane, path: String, cx: &mut Context<Backplane>) {
    this.screens.projects.folder_ix = None; this.screens.projects.folder_scroll.scroll_to_item(0);
    if let Some(i) = this.screens.projects.folder.clone() { i.update(cx, |i, cx| i.set(path, cx)); }
    cx.notify();
}
fn key(this: &mut Backplane, k: Option<Key>, cx: &mut Context<Backplane>) {
    match k { Some(Key::Cd(p)) => cd(this, p, cx), Some(Key::Open(pid)) => { let _ = this.tx.try_send(close_cmd()); this.open_project(pid, cx); } Some(Key::Run(a, v)) => { if a == "proj-add" || a == "proj-new" { this.arm_new(); } let _ = this.tx.try_send(Command::Action(a, v)); cx.notify(); } None => {} }
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
            let now = this.input.read(cx).content.to_string();
            if let Some(t) = give_back(text, &now, &mut this.screens.projects.restore) { this.input.update(cx, |i, cx| i.set(t, cx)); }
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
    if let Some((th, kind)) = this.screens.projects.view_edit.clone() {
        let shown = string(&this.data["thread"], "id"); let shown = shown.rsplit('|').next().unwrap_or("").to_string();
        if shown == th && this.selected.rsplit('|').next() == Some(th.as_str()) && this.data["thread"]["loading"] != true {
            this.screens.projects.view_edit = None; let _ = this.tx.try_send(Command::Action("view-kind".into(), kind.into()));
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
        None => { this.screens.projects.folder_open = false; this.screens.projects.sent_f.clear(); this.screens.projects.folder_ix = None; }
        Some(f) => {
            if this.screens.projects.folder.is_none() {
                let input = cx.new(|cx| Composer::with_context(cx, "AskInput"));
                // the arrows and Tab come here as the popup's keys do (one line: they have nothing to move in the field)
                input.update(cx, |i, _| i.popup_open = true);
                cx.observe(&input, |this, input, cx| {
                    let text = input.read(cx).content.to_string(); let p = &mut this.screens.projects;
                    // `prev`: what the field held before this change, typed here or put by the hub (a notify with the same text is no change)
                    let prev = std::mem::replace(&mut p.prev_f, text.clone()); if prev == text { return; }
                    if p.echo_f.as_deref() == Some(text.as_str()) { p.echo_f = None; return; }
                    if p.sent_f.last() == Some(&text) { return; }
                    p.folder_ix = None;
                    if let Some(up) = back(&prev, &text) { input.update(cx, |i, cx| i.set(up, cx)); return; }
                    remember(&mut p.sent_f, &text); let _ = this.tx.try_send(type_cmd(&text));
                }).detach();
                // Enter: the marked folder, else the picker's Add or Create row, as a click on it would (only once the hub's rows are for what the field holds)
                cx.subscribe(&input, |this, input, ev: &ComposerEvent, cx| {
                    let typed = input.read(cx).content.to_string(); let ix = this.screens.projects.folder_ix;
                    match ev {
                        ComposerEvent::Submit => { let k = enter(&this.data, &typed, ix); key(this, k, cx); }
                        ComposerEvent::PopupPick => { let k = complete(&this.data, &typed, ix).map(Key::Cd); key(this, k, cx); }
                        ComposerEvent::PopupMove(d) => if let Some((to, row)) = walk(&this.data, &typed, ix, *d) {
                            let p = &mut this.screens.projects;
                            if to != p.folder_ix { p.folder_ix = to; p.folder_scroll.scroll_to_item(row); cx.notify(); }
                        }
                        _ => {}
                    }
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

/// The folder picker over the main area, while the hub shows `folders`: a column with the title (Esc and × on its right), the path field, the
/// Add or Create row under it, then "..", and the folders with what each is on the right: "added" (a project here already, drawn quieter), "KiCad"
/// or "git". Rows the hub made for an older text are narrowed or drawn faint until it answers (`fresh`); the footer names only the keys that do something now.
pub fn view(cx: &mut Context<Backplane>, light: bool, p: &Projects, data: &Value) -> Option<AnyElement> {
    let f = folders(data)?;
    let c = |n: &str| theme::color(light, n);
    let typed = p.folder.as_ref().map(|i| i.read(cx).content.to_string()).unwrap_or_else(|| f.text.clone());
    let now = fresh(&f.text, &typed);
    let mark = marked(&f, p.folder_ix).map(|_| p.folder_ix.unwrap_or(0));
    let mut top: Vec<AnyElement> = vec![];
    let mut list = div().id("folders-list").track_scroll(&p.folder_scroll).flex().flex_col().w_full().flex_initial().min_h_0().overflow_y_scroll();
    let mut dirs = 0;
    for (i, it) in f.items.iter().enumerate() {
        let (show, faint) = shows(it, &now); if !show { continue }
        let a = it.action.as_str(); let live = !a.is_empty();
        // a click is judged when it lands (`click`): the rows may be older than the field by then
        let hit = cx.listener(move |this, _: &gpui::ClickEvent, _, cx| { let typed = this.screens.projects.folder.as_ref().map(|i| i.read(cx).content.to_string()); let k = typed.and_then(|t| click(&this.data, &t, i)); key(this, k, cx); });
        let row = crate::ui::with_rect(div().id(SharedString::from(format!("folder-{i}"))), move || format!("folder-{i}")).flex().flex_none().items_center().gap_2().w_full().when(faint, |d| d.opacity(0.45));
        match it.kind.as_str() {
            "add" => top.push(row.h(px(34.)).px(px(11.)).bg(c("accent")).text_size(px(13.)).text_color(c("on-accent")).when(!faint, |d| d.cursor_pointer().hover(|s| s.opacity(0.9)))
                .child(crate::ui::ic(light, if a == "proj-add" { "plus" } else if a == "proj-new" { "folder-plus" } else { "folder-down" }, 15., "on-accent"))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(fit(&crate::i18n::trs(&it.label), 72)))
                .when(mark.is_none() && !faint, |d| d.child(div().flex_none().text_size(px(11.)).opacity(0.7).child("Enter" /* i18n-ok: a key's name */)))
                .on_click(hit).into_any_element()),
            "note" => top.push(row.h(px(34.)).px(px(10.)).border_1().border_color(c("line")).text_size(px(13.)).text_color(c("dim"))
                .child(crate::ui::ic(light, "circle-check", 14., "ok"))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(fit(&crate::i18n::trs(&it.label), 76))).into_any_element()),
            kind => {
                let up = kind == "up"; if !up { dirs += 1; }
                let on = mark == Some(i); let added = it.tag == "project";
                let name = if up { it.label.clone() } else { it.label.trim_end_matches('/').to_string() };
                // the marked row: a bar of the accent on its left over the raised colour; the pointer's row only the panel's colour
                list = list.child(row.h(px(28.)).px(px(10.)).border_l_2().border_color(if on { c("accent") } else { gpui::Rgba { r: 0., g: 0., b: 0., a: 0. } })
                    .text_size(px(13.)).text_color(c(if !live { "faint" } else if added { "dim" } else { "text" })).when(on, |d| d.bg(c("raise"))).when(live && !on && !faint, |d| d.cursor_pointer().hover(|s| s.bg(c("panel"))))
                    .child(crate::ui::ic(light, if up { "corner-left-up" } else if added { "circle-check" } else { "folder" }, 14., if added { "ok" } else if it.tag.is_empty() { "dim" } else { "accent" }))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(name))
                    .when(!it.tag.is_empty(), |d| d.child(div().flex_none().text_size(px(11.)).text_color(c("faint")).child(crate::i18n::trs(if added { "added" } else { it.tag.as_str() }))))
                    .on_click(hit));
            }
        }
    }
    let naming = matches!(&now, Fresh::Same(n) if !n.is_empty()) || (now == Fresh::Now && !typed.ends_with('/'));
    if dirs == 0 && f.error.is_empty() && !naming { list = list.child(div().px(px(12.)).h(px(28.)).flex().items_center().text_size(px(12.)).text_color(c("faint")).child(crate::i18n::trs("No folders"))); }
    if f.more > 0 && now == Fresh::Now { list = list.child(div().px(px(12.)).h(px(28.)).flex().flex_none().items_center().text_size(px(12.)).text_color(c("faint")).child(crate::i18n::trs(&format!("{} more: type to filter", f.more)))); }
    let cancel = crate::ui::with_rect(div().id("folders-cancel"), || "folders-cancel".to_string()).flex().items_center().gap_2().h(px(26.)).px_2().cursor_pointer().hover(|s| s.bg(c("raise")))
        .child(div().text_size(px(11.)).text_color(c("faint")).child("Esc" /* i18n-ok: a key's name */)).child(crate::ui::ic(light, "x", 14., "dim"))
        .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(close_cmd()); cx.notify(); }));
    let keys: Vec<_> = hints(&f, &typed, p.folder_ix).into_iter().map(|(k, w)| div().flex().gap_1().child(div().text_color(c("dim")).child(k)).child(crate::i18n::trs(w))).collect();
    Some(div().id("folders-q").absolute().inset_0().occlude().bg(c("bg")).flex().flex_col().items_center().px_6().pt(px(56.)).pb_4()
        .child(div().flex().flex_col().w_full().max_w(px(680.)).flex_1().min_h_0().gap_2()
            .child(div().flex().flex_none().items_center().justify_between().pb_1().child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(crate::i18n::trs("Add project"))).child(cancel))
            .children(p.folder.clone().map(|i| div().flex().flex_none().items_center().gap_2().w_full().h(px(36.)).px(px(10.)).border_1().border_color(c(if f.error.is_empty() { "line" } else { "bad" })).bg(c("view")).text_size(px(14.))
                .child(crate::ui::ic(light, "folder", 15., "dim")).child(div().flex_1().min_w_0().child(i))))
            .when(!f.error.is_empty(), |d| d.child(div().flex_none().px(px(10.)).text_size(px(12.)).text_color(c("bad")).child(fit(&crate::i18n::trs(&f.error), 90))))
            .children(top)
            .child(list)
            // the keys sit right under the last row (the list takes only the height of its rows), not at the foot of the window
            .when(!keys.is_empty(), |d| d.child(div().flex().flex_none().gap_4().h(px(22.)).px(px(12.)).items_end().text_size(px(11.)).text_color(c("faint")).children(keys))))
        .into_any_element())
}

/// The top bar's place while a project's page is on show: the project's name (its page stands for the selection, not the last thread).
pub fn page_title(data: &Value, page: Option<&str>) -> Option<String> { page.and_then(|p| find(data, p)).map(|p| string(p, "title")) }
/// Whether an action sent to the hub goes somewhere (a bot's chat, a room, a link, a thread): the project's page on show gives way to it.
/// The page is this window's own; the hub's selection may not move at all (the uberbot's chat was already selected under the page), so
/// a click on the uberbot after a project page left the page up.
pub fn leaves_page(action: &str) -> bool { matches!(action, "bot" | "room" | "link-open" | "link-file" | "select") }
/// The sidebar's project with this (raw) id.
pub fn find<'a>(data: &'a Value, pid: &str) -> Option<&'a Value> {
    data["projects"].as_array().and_then(|ps| ps.iter().find(|p| string(p, "id").rsplit('|').next() == Some(pid))).or_else(|| super::sections::find(data, pid))
}
/// The folder's mechanical parts, assembly first: (path, name, bytes) of the `mech` artifacts, the largest file (the assembly, the one with the most parts in it) at the top.
pub fn mech_parts(snap: &Value) -> Vec<(String, String, u64)> {
    let mut v: Vec<(String, String, u64)> = snap["artifacts"].as_array().into_iter().flatten().filter(|a| a["kind"] == "mech").map(|a| {
        let p = a["path"].as_str().unwrap_or("").to_string(); let n = p.rsplit('/').next().unwrap_or("").to_string(); (p, n, a["size"].as_u64().unwrap_or(0))
    }).collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1))); v
}
/// The viewers a project's page offers, from what the folder holds: (label, view kind, icon).
pub fn viewer_buttons(snap: &Value) -> Vec<(&'static str, &'static str, &'static str)> {
    let has = |k: &str| snap["artifacts"].as_array().map_or(false, |a| a.iter().any(|x| x["kind"] == k));
    let mut v = vec![];
    /* i18n-ok: viewer kinds, drawn through trs */ { if has("board") { v.push(("Board", "board", "circuit-board")); } if has("schematic") { v.push(("Schematic", "schematic", "cpu")); } if has("board") { v.push(("3D", "3d", "box")); } if has("mech") { v.push(("Mech", "mech", "layers")); } v.push(("Files", "files", "folder")); }
    v
}
/// A size in words: 1.2 MB, 340 KB.
pub fn size_text(b: u64) -> String { if b >= 1 << 20 { format!("{:.1} MB", b as f64 / (1u64 << 20) as f64) } else { format!("{} KB", (b + 1023) >> 10) } }
fn section(light: bool, label: String, right: Option<AnyElement>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().flex().items_center().justify_between().pt_5().pb_1().border_b_1().border_color(c("line")).child(div().text_size(px(11.)).text_color(c("faint")).child(label)).children(right).into_any_element()
}
fn action_button(cx: &mut Context<Backplane>, light: bool, id: &'static str, icon: &'static str, label: String, primary: bool) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let _ = cx;
    div().id(id).flex().items_center().gap_2().px_3().h(px(30.)).border_1().border_color(c(if primary { "accent" } else { "line" })).bg(c(if primary { "accent" } else { "raise" })).text_size(px(12.)).cursor_pointer()
        .hover(|s| s.bg(c(if primary { "accent" } else { "panel" })).opacity(0.9))
        .child(crate::ui::ic(light, icon, 14., if primary { "bg" } else { "text" })).child(div().text_color(c(if primary { "bg" } else { "text" })).child(label))
}
/// A project's page, from the sidebar's own rows (no round trip): its name and folder with the actions (New thread, Open folder, the viewers), its threads, the README,
/// the goal and notes, the mechanical parts (assembly first), the parts to order and the boards.
pub fn page(cx: &mut Context<Backplane>, reg: &super::timeline::Reg, light: bool, data: &Value, home: &Home, pid: &str, ctl: &mut Vec<crate::script::CtlEntry>, doc: &super::timeline::Doc) -> Vec<AnyElement> {
    use crate::script::CtlEntry;
    let Some(p) = find(data, pid) else { return vec![] };
    let c = |n: &str| theme::color(light, n);
    let threads: Vec<&Value> = p["threads"].as_array().into_iter().flatten().chain(p["settled"].as_array().into_iter().flatten()).collect();
    let (hp, snap, card) = (home.pid.clone(), home.snap.clone(), home.card.clone());
    let loaded = hp == pid;
    let root = if loaded { let r = string(&snap, "root"); if r.is_empty() { string(p, "root") } else { r } } else { string(p, "root") };
    let top = threads.first().map(|t| string(t, "id"));
    let mut out: Vec<AnyElement> = vec![
        div().text_size(px(22.)).font_weight(FontWeight::BOLD).child(string(p, "title")).into_any_element(),
        div().text_size(px(12.)).text_color(c("dim")).child(root.clone()).into_any_element(),
    ];
    // the actions: New thread first (the way in), Open folder, then the viewers the folder has something for (they open on the project's top thread)
    let mut bar = div().flex().flex_wrap().items_center().gap_2().pt_3().pb_1();
    { let pid2 = pid.to_string();
      /* i18n-ok: ctl labels and ui: ids are what drivers press */ ctl.push(CtlEntry { id: "page-new-thread".into(), label: "New thread".into(), ctl: Ctl::Node("new-thread".into(), pid.to_string()) });
      bar = bar.child(action_button(cx, light, "page-new-thread", "plus", crate::i18n::trs("New thread"), true)
        .on_click(cx.listener(move |this, _, _, cx| { this.arm_new(); let _ = this.tx.try_send(Command::Action("new-thread".into(), pid2.clone())); cx.notify(); }))); }
    { let r = root.clone();
      bar = bar.child(action_button(cx, light, "page-open-folder", "folder-open", crate::i18n::trs("Open folder"), false).on_click(cx.listener(move |this, _, _, _| this.file_open(&r, false)))); }
    if loaded { if let Some(top) = top.clone() {
        bar = bar.child(div().w(px(1.)).h(px(18.)).mx_1().bg(c("line")));
        for (label, kind, icon) in viewer_buttons(&snap) {
            let (t, id) = (top.clone(), format!("page-open-{kind}"));
            /* i18n-ok: ctl labels and ids */ ctl.push(CtlEntry { id: id.clone(), label: format!("Open {label}"), ctl: Ctl::Select(top.clone()) });
            let sid: &'static str = Box::leak(id.into_boxed_str());
            bar = bar.child(action_button(cx, light, sid, icon, crate::i18n::trs(label), false).on_click(cx.listener(move |this, _, window, cx| {
                let raw = t.rsplit('|').next().unwrap_or("").to_string(); this.screens.projects.view_edit = Some((raw, kind)); this.select(t.clone(), window, cx);
            })));
        }
    } }
    out.push(bar.into_any_element());
    // threads: the hub's lists for this project (running, needs you, recent, Done (n)), every machine's threads
    let lists = super::sections::page_lists(cx, light, data, pid, ctl);
    let listed = lists.is_some();
    if let Some(l) = lists { out.extend(l); }
    if !listed {
        out.push(section(light, crate::i18n::tf("gpui.projects.threads", &[("n", threads.len().into())]), None));
        if threads.is_empty() { out.push(div().py_2().text_color(c("dim")).child(crate::i18n::trs("No threads yet.")).into_any_element()); }
        for t in &threads {
            let id = string(t, "id"); let raw = id.rsplit('|').next().unwrap_or(&id).to_string();
            ctl.push(CtlEntry { id: format!("page-thread:{raw}"), label: string(t, "title"), ctl: Ctl::Select(id.clone()) });
            out.push(crate::ui::with_rect(div().id(SharedString::from(format!("page-thread-{raw}"))), { let r = raw.clone(); move || format!("page-thread:{r}") }).flex().items_center().gap_2().px_2().h(px(30.)).border_b_1().border_color(c("line")).cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(super::siderow::dot(light, t)).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(string(t, "title")))
                .on_click(cx.listener(move |this, _, window, cx| this.select(id.clone(), window, cx))).into_any_element());
        }
    }
    if !loaded { return out }
    // the README, drawn from its Markdown as GitHub draws it (pictures, alignment, alerts, details: the hub's md/doc.rs)
    if !home.readme.is_empty() {
        out.push(section(light, crate::i18n::trs("README"), None));
        reg.borrow_mut().enter(0, "home-readme");
        out.push(div().id("home-readme").flex().flex_col().gap_3().py_2().text_size(px(14.)).children(super::timeline::doc_blocks(cx, reg, light, &home.readme, "home-readme", doc)).into_any_element());
    }
    // the goal and notes
    let intent = { let i = string(&card, "intent"); if i.is_empty() && snap["artifacts"].as_array().map_or(false, |a| a.iter().any(|x| x["kind"] == "intent")) { string(&snap, "intent") } else { i } };
    let edit = top.clone().map(|top| {
        let text = intent_draft(&intent);
        /* i18n-ok: ctl labels and ui: ids are what drivers press */ ctl.push(CtlEntry { id: "home-intent-edit".into(), label: "Edit goal & notes".into(), ctl: Ctl::Select(top.clone()) });
        /* i18n-ok: ctl labels and ui: ids are what drivers press */ crate::ui::with_rect(div().id("home-intent-edit"), || "ui:Edit goal & notes".to_string()).px_2().py_px().border_1().border_color(c("line")).bg(c("raise")).text_size(px(11.)).cursor_pointer().hover(|s| s.bg(c("panel"))).child(crate::i18n::trs("Edit goal & notes"))
            .on_click(cx.listener(move |this, _, window, cx| { let raw = top.rsplit('|').next().unwrap_or("").to_string(); this.screens.projects.intent_edit = Some((raw, text.clone())); this.select(top.clone(), window, cx); })).into_any_element()
    });
    out.push(section(light, crate::i18n::trs("GOAL & NOTES"), edit));
    if intent.trim().is_empty() { out.push(div().py_2().text_color(c("dim")).child(crate::i18n::trs("No goal yet. Ask an agent to write what this project is for, or edit .backplane/intent.md.")).into_any_element()); }
    let mut col = div().id("home-intent").flex().flex_col().gap_1().py_2().text_size(px(13.));
    for (k, t) in intent_lines(&intent) {
        col = col.child(match k { "h" => div().pt_1().font_weight(FontWeight::BOLD).child(t), "li" => div().flex().gap_2().child(div().text_color(c("dim")).child("•")).child(div().flex_1().min_w_0().child(t)), _ => div().child(t) });
    }
    out.push(col.into_any_element());
    // the mechanical parts: the assembly (the file with the most in it) first
    let mech = mech_parts(&snap);
    if !mech.is_empty() {
        let open = top.clone().map(|top| {
            let t = top.clone();
            /* i18n-ok: a ctl id */ crate::ui::with_rect(div().id("home-mech-open"), || "ui:Open Mech".to_string()).px_2().py_px().border_1().border_color(c("line")).bg(c("raise")).text_size(px(11.)).cursor_pointer().hover(|s| s.bg(c("panel"))).child(crate::i18n::trs("Mech"))
                .on_click(cx.listener(move |this, _, window, cx| { let raw = t.rsplit('|').next().unwrap_or("").to_string(); this.screens.projects.view_edit = Some((raw, "mech")); this.select(t.clone(), window, cx); })).into_any_element()
        });
        out.push(section(light, crate::i18n::tf("gpui.projects.mech", &[("n", mech.len().into())]), open));
        for (i, (path, name, size)) in mech.iter().enumerate() {
            let rel = path.strip_prefix(&root).map(|r| r.trim_start_matches('/').to_string()).unwrap_or_else(|| path.clone());
            let (pth, first) = (path.clone(), i == 0 && mech.len() > 1);
            out.push(div().id(SharedString::from(format!("home-mech-{i}"))).flex().gap_2().px_2().h(px(28.)).items_center().border_b_1().border_color(c("line")).text_size(px(12.)).cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(crate::ui::ic(light, if first { "layers" } else { "box" }, 14., if first { "accent" } else { "dim" }))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().child(name.clone()))
                .children(first.then(|| div().text_color(c("accent")).child(crate::i18n::trs("Assembly"))))
                .child(div().text_color(c("faint")).child(size_text(*size)))
                .on_click(cx.listener(move |this, _, _, _| this.file_open(&pth, true)))
                .tooltip(crate::keys::tip_raw(&rel, "", light))
                .into_any_element());
        }
    }
    let parts: Vec<&Value> = card["parts"].as_array().into_iter().flatten().collect();
    if !parts.is_empty() {
        out.push(section(light, crate::i18n::tf("gpui.projects.to.order", &[("n", parts.len().into())]), None));
        for (i, x) in parts.iter().enumerate() {
            out.push(div().id(SharedString::from(format!("home-part-{i}"))).flex().gap_2().px_2().h(px(26.)).items_center().border_b_1().border_color(c("line")).text_size(px(12.))
                .child(div().flex_1().child(string(x, "name"))).child(div().text_color(c("dim")).child(string(x, "kind"))).child(div().text_color(c("faint")).child(string(x, "path"))).into_any_element());
        }
    }
    let bs = boards(&snap, &card, &root);
    if !bs.is_empty() {
        out.push(section(light, crate::i18n::tf("gpui.projects.boards", &[("n", bs.len().into())]), None));
        for (i, b) in bs.iter().enumerate() { out.push(div().id(SharedString::from(format!("home-board-{i}"))).px_2().h(px(26.)).flex().items_center().border_b_1().border_color(c("line")).text_size(px(12.)).child(b.clone()).into_any_element()); }
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
        for a in ["bot", "room", "link-open", "link-file", "select"] { assert!(super::leaves_page(a), "{a}"); }
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
        assert_eq!(f.items[0].tag, "", "an older hub sends no tag");
    }
    #[test] fn the_arrows_walk_the_folder_rows_and_enter_and_tab_go_into_the_marked_one() {
        let d = j(r#"{"folders":{"text":"~/a","hint":"","error":"","items":[{"label":"Create ~/a","action":"proj-new","value":"/h/a","kind":"add"},{"label":"..","action":"proj-cd","value":"/","kind":"up"},
            {"label":"alpha/","action":"proj-cd","value":"/h/alpha/","kind":"dir","tag":"KiCad"},{"label":"avr/","action":"proj-cd","value":"/h/avr/","kind":"dir"}]}}"#);
        let f = folders(&d).unwrap(); assert_eq!(f.items[2].tag, "KiCad");
        assert_eq!(step(&f.items, None, 1), Some(1), "Down from the field: the first row under the Add row"); assert_eq!(step(&f.items, None, -1), None);
        assert_eq!(step(&f.items, Some(1), 1), Some(2)); assert_eq!(step(&f.items, Some(3), 1), Some(3), "the last row holds");
        assert_eq!(step(&f.items, Some(2), -1), Some(1)); assert_eq!(step(&f.items, Some(1), -1), None, "Up from the first: the field again");
        assert_eq!(step(&f.items, Some(9), 1), Some(1), "a row that is gone counts as the field"); assert_eq!(step(&[], None, 1), None);
        assert_eq!(enter(&d, "~/a", Some(2)), Some(Key::Cd("/h/alpha/".into())), "Enter on a marked folder goes into it");
        assert_eq!(enter(&d, "~/a", None), Some(Key::Run("proj-new".into(), "/h/a".into())), "Enter with none marked is the Add or Create row");
        assert_eq!(enter(&d, "~/a", Some(0)), enter(&d, "~/a", None), "the Add row is never the marked one"); assert_eq!(enter(&d, "~/ab", None), None);
        assert_eq!(complete(&d, "~/a", None).as_deref(), Some("/h/alpha/"), "Tab: the first folder that matches"); assert_eq!(complete(&d, "~/a", Some(3)).as_deref(), Some("/h/avr/"));
        assert_eq!(complete(&d, "~/al", None), None, "rows for an older text: Tab waits");
        let bare = j(r#"{"folders":{"text":"~/","items":[{"label":"alpha/","action":"proj-cd","value":"/h/alpha/","kind":"dir"}]}}"#);
        assert_eq!(complete(&bare, "~/", None), None, "no name typed: nothing to complete"); assert_eq!(complete(&bare, "~/", Some(0)).as_deref(), Some("/h/alpha/"));
    }
    #[test] fn going_into_a_row_writes_its_tilde_form_and_rows_for_an_older_text_are_narrowed_or_faint() {
        let d = j(r#"{"folders":{"text":"~/a","more":7,"items":[{"label":"Create ~/a","action":"proj-new","value":"/h/a","kind":"add"},{"label":"..","action":"proj-cd","value":"/","field":"/","kind":"up"},
            {"label":"alpha/","action":"proj-cd","value":"/h/alpha/","field":"~/alpha/","kind":"dir"},{"label":"avr/","action":"proj-cd","value":"/h/avr/","field":"~/avr/","kind":"dir"},{"label":".al/","action":"proj-cd","value":"/h/.al/","kind":"dir"}]}}"#);
        let f = folders(&d).unwrap(); assert_eq!(f.more, 7);
        assert_eq!(enter(&d, "~/a", Some(2)), Some(Key::Cd("~/alpha/".into())), "the field takes \"~/alpha/\", not the absolute path");
        assert_eq!(complete(&d, "~/a", None).as_deref(), Some("~/alpha/"));
        assert_eq!(fresh("~/a", "~/a"), Fresh::Now); assert_eq!(fresh("~/a", "~/AL"), Fresh::Same("al".into())); assert_eq!(fresh("~/", "~/alpha/"), Fresh::Away); assert_eq!(fresh("~/alpha/", "~/"), Fresh::Away);
        let drawn = |t: &str| f.items.iter().map(|i| shows(i, &fresh(&f.text, t))).collect::<Vec<_>>();
        assert_eq!(drawn("~/a"), [(true, false); 5]);
        assert_eq!(drawn("~/al"), [(true, true), (true, false), (true, false), (false, false), (false, false)], "narrowed at once by the name typed; the Create row is for \"~/a\": faint");
        assert_eq!(drawn("~/.a")[4], (true, false), "a hidden folder once a dot is typed");
        assert_eq!(drawn("~/alpha/"), [(true, true); 5], "another folder: every row faint until the hub answers");
    }
    #[test] fn backspace_over_a_folders_slash_goes_up_and_the_footer_names_only_live_keys() {
        assert_eq!(back("~/a/b/", "~/a/b").as_deref(), Some("~/a/")); assert_eq!(back("~/a/", "~/a").as_deref(), Some("~/")); assert_eq!(back("/a/", "/a").as_deref(), Some("/"));
        assert_eq!(back("~/", "~"), None, "the home stays"); assert_eq!(back("/", ""), None); assert_eq!(back("~/a/b", "~/a/"), None, "an ordinary backspace"); assert_eq!(back("~/a/b/", "~/a/"), None, "not one key's worth");
        let d = j(r#"{"folders":{"text":"~/","items":[{"label":"Add ~","action":"proj-add","value":"/h","kind":"add"},{"label":"alpha/","action":"proj-cd","value":"/h/alpha/","kind":"dir"}]}}"#);
        let f = folders(&d).unwrap();
        assert_eq!(hints(&f, "~/", None), [("↑↓", "select")], "Enter is on the Add row; no name: nothing to complete");
        assert_eq!(hints(&f, "~/", Some(1)), [("↑↓", "select"), ("Enter", "go into"), ("Tab", "complete")]);
        assert_eq!(hints(&f, "~/al", None), [("↑↓", "select"), ("Tab", "complete")]);
        let empty = folders(&j(r#"{"folders":{"text":"~/x/","items":[]}}"#)).unwrap(); assert!(hints(&empty, "~/x/", None).is_empty());
    }
    #[test] fn a_click_is_judged_against_the_field_when_it_lands_and_a_project_row_opens_its_page() {
        let d = j(r#"{"folders":{"text":"~/","items":[{"label":"Add ~","action":"proj-add","value":"/h","kind":"add"},{"label":"..","action":"proj-cd","value":"/","field":"/","kind":"up"},
            {"label":"alpha/","action":"proj-cd","value":"/h/alpha/","field":"~/alpha/","kind":"dir"},{"label":"mine/","action":"proj-cd","value":"/h/mine/","field":"~/mine/","kind":"dir","tag":"project","project":"p9"},
            {"label":"x is there","action":"","value":"","kind":"note"}]}}"#);
        assert_eq!(click(&d, "~/", 0), Some(Key::Run("proj-add".into(), "/h".into()))); assert_eq!(click(&d, "~/", 1), Some(Key::Cd("/".into()))); assert_eq!(click(&d, "~/", 2), Some(Key::Cd("~/alpha/".into())));
        assert_eq!(click(&d, "~/", 3), Some(Key::Open("p9".into())), "a folder that is a project: its page"); assert_eq!(click(&d, "~/", 4), None, "a note"); assert_eq!(click(&d, "~/", 9), None);
        // the field has moved on and the hub has not answered: "Add ~" must not add the home, and a row of the older folder is not gone into
        assert_eq!(click(&d, "~/alpha/", 0), None); assert_eq!(click(&d, "~/alpha/", 2), None); assert_eq!(click(&d, "~/alpha/", 3), None);
        assert_eq!(click(&d, "~/al", 0), None, "the Add row is for \"~/\""); assert_eq!(click(&d, "~/al", 2), Some(Key::Cd("~/alpha/".into())), "the same folder, and the row matches"); assert_eq!(click(&d, "~/al", 3), None, "narrowed away");
        assert_eq!(click(&d, "~/al", 1), Some(Key::Cd("/".into())));
        // the keys: Enter opens the project, Tab still goes in; neither takes a mark while the rows are for an older text
        assert_eq!(enter(&d, "~/", Some(3)), Some(Key::Open("p9".into()))); assert_eq!(complete(&d, "~/", Some(3)).as_deref(), Some("~/mine/"));
        assert_eq!(enter(&d, "~/alpha/", Some(3)), None); assert_eq!(complete(&d, "~/alpha/", Some(3)), None);
        assert_eq!(walk(&d, "~/", None, 1), Some((Some(1), 0))); assert_eq!(walk(&d, "~/", Some(2), 1), Some((Some(3), 2)), "the mark and its place in the list (the Add row is above it)"); assert_eq!(walk(&d, "~/alpha/", None, 1), None, "the arrows wait for the hub's rows");
        let f = folders(&d).unwrap(); assert_eq!(hints(&f, "~/", Some(3))[1], ("Enter", "open project")); assert_eq!(hints(&f, "~/", Some(2))[1], ("Enter", "go into"));
    }
    #[test] fn names_that_are_not_ascii_are_cut_narrowed_and_gone_up_from_without_a_panic() {
        assert_eq!(fit("Add ~/基板/日本語のとても長いフォルダ名/プロジェクト", 15), "Add …ルダ名/プロジェクト"); assert_eq!(fit("基板基板基板基板基板基板", 8), "…板基板基板基板"); assert_eq!(fit("éééé", 3), "éééé", "too narrow to cut: as it is");
        assert_eq!(fit(&"é ".repeat(20), 9).chars().count(), 9);
        assert_eq!(fresh("~/基板/É", "~/基板/éc"), Fresh::Same("éc".into())); assert_eq!(fresh("~/基板/", "~/基"), Fresh::Away);
        let it = |l: &str| Item { label: l.into(), action: "proj-cd".into(), value: String::new(), kind: "dir".into(), tag: String::new(), field: String::new(), project: String::new() };
        assert_eq!(shows(&it("Écran/"), &Fresh::Same("éc".into())), (true, false)); assert_eq!(shows(&it("基板/"), &Fresh::Same("é".into())), (false, false));
        assert_eq!(back("~/基板/", "~/基板").as_deref(), Some("~/")); assert_eq!(back("~/a/é/", "~/a/é").as_deref(), Some("~/a/")); assert_eq!(back("é/", "é"), None, "no folder above");
    }
    #[test] fn the_pickers_words_are_in_the_japanese_catalog() {
        let c = crate::i18n::catalog();
        for w in ["No folders", "select", "go into", "open project", "complete", "added", "7 more: type to filter", "Add project"] { assert!(c.japanese(w).is_some(), "{w}"); }
        assert_eq!(c.japanese("open project").as_deref(), Some("プロジェクトを開く")); assert_eq!(c.japanese("7 more: type to filter").as_deref(), Some("ほか7件: 入力して絞り込む"));
    }
    #[test] fn a_long_label_keeps_its_first_word_and_the_end_of_its_path() {
        assert_eq!(fit("Add ~/x", 72), "Add ~/x");
        let long = format!("Add /very/{}board", "deep/".repeat(30)); let got = fit(&long, 40);
        assert_eq!(got.chars().count(), 40); assert!(got.starts_with("Add …") && got.ends_with("deep/board"), "{got}");
        assert_eq!(fit(&"x".repeat(50), 20), format!("…{}", "x".repeat(19)), "no first word: the end");
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
    /// A page's Send with a linked machine picked: the command names that machine's project, the text waits; refused, the text comes back.
    #[test] fn a_page_send_goes_to_the_machine_picked_and_a_refusal_gives_the_text_back() {
        let d: serde_json::Value = serde_json::from_str(r#"{"projects":[{"id":"h|pA1","runsOn":"runs on boat-hub","newOn":[{"value":"L~pB1","label":"New thread on boat-hub","kind":"machine","on":true},{"value":"pA1","label":"New thread on desktop","kind":"machine"},{"value":"github.com/o/demo","label":"A Boat (fresh clone)","kind":"boat","disabled":true}]}]}"#).unwrap();
        match page_send(&d, "pA1", "a thread sent to boat-hub", None) { Some(Ok((Command::Action(a, v), t))) => { assert_eq!((a.as_str(), v.as_str(), t.as_str()), ("new-thread", "L~pB1", "a thread sent to boat-hub")) } other => panic!("{other:?}") }
        match page_send(&d, "pA1", "x", Some("L~pB1")) { Some(Ok((Command::Action(_, v), _))) => assert_eq!(v, "L~pB1\u{1f}L~pB1", "the pick goes with it"), other => panic!("{other:?}") }
        let mut d2 = d.clone(); d2["projects"][0]["newOn"][0]["on"] = serde_json::json!(false);
        match page_send(&d2, "pA1", "x", None) { Some(Ok((Command::Action(_, v), _))) => assert_eq!(v, "pA1", "no choice: this project"), other => panic!("{other:?}") }
        // boat-hub picked, but the line's choice is desktop (the pick was lost, the rows moved): never sent to desktop, the text stays
        assert!(matches!(page_send(&d2, "pA1", "x", Some("L~pB1")), Some(Err(_))), "a pick of B never becomes a create on A");
        assert!(page_send(&d, "pA1", "  ", None).is_none(), "nothing typed: nothing sent");
        // boat-hub picked, then it goes away and drops out of the live rows entirely (only desktop is listed): still refused, never desktop
        let gone: serde_json::Value = serde_json::from_str(r#"{"projects":[{"id":"h|pA1","runsOn":"runs on desktop","newOn":[{"value":"pA1","label":"New thread","kind":"machine","on":true}]}]}"#).unwrap();
        assert!(matches!(page_send(&gone, "pA1", "x", Some("L~pB1")), Some(Err(_))), "the person's last pick stands when its machine has left the rows");
        // the window's line still offers it as away: sent there (the hub then refuses it while that machine is away)
        let away: serde_json::Value = serde_json::from_str(r#"{"projects":[{"id":"h|pA1","runsOn":"boat-hub is away","newOn":[{"value":"L~pB1","label":"Wait for boat-hub","kind":"wait","on":true},{"value":"pA1","label":"New thread on desktop","kind":"machine"}]}]}"#).unwrap();
        match page_send(&away, "pA1", "x", Some("L~pB1")) { Some(Ok((Command::Action(_, v), _))) => assert_eq!(v, "L~pB1\u{1f}L~pB1"), other => panic!("{other:?}") }
        let mut fs = Some(("t0".to_string(), "a thread sent to boat-hub".to_string(), std::time::Instant::now()));
        assert_eq!(take_back(&mut fs).as_deref(), Some("a thread sent to boat-hub")); assert!(fs.is_none());
        // given back: into an empty box exactly; a box with new text keeps it, and the old text waits to be restored (never merged)
        match first_message("L~t9", "a thread sent to boat-hub") { Command::Rpc(m, p, _) => { assert_eq!(m, "turn.start"); assert_eq!((p.0["thread"].as_str(), p.0["text"].as_str()), (Some("L~t9"), Some("a thread sent to boat-hub"))) } other => panic!("{other:?}") }
        let mut r = None;
        assert_eq!(give_back("old".into(), "", &mut r).as_deref(), Some("old")); assert!(r.is_none());
        assert_eq!(give_back("old".into(), "new words", &mut r), None); assert_eq!(r.as_deref(), Some("old"));
        let mut r2 = None; assert_eq!(give_back("same".into(), "same", &mut r2), None); assert!(r2.is_none(), "still in the box: nothing to put back");
    }

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
    #[test] fn the_mech_list_puts_the_assembly_first_and_the_viewers_follow_what_the_folder_has() {
        let snap = json!({"artifacts": [{"kind": "board", "path": "/p/a.kicad_pcb"}, {"kind": "mech", "path": "/p/mech/lid.step", "size": 100}, {"kind": "mech", "path": "/p/mech/asm.step", "size": 5000}]});
        assert_eq!(mech_parts(&snap).iter().map(|m| m.1.as_str()).collect::<Vec<_>>(), ["asm.step", "lid.step"]);
        assert_eq!(viewer_buttons(&snap).iter().map(|v| v.1).collect::<Vec<_>>(), ["board", "3d", "mech", "files"]);
        assert_eq!(viewer_buttons(&Value::Null).iter().map(|v| v.1).collect::<Vec<_>>(), ["files"]);
        assert_eq!((size_text(2048), size_text(3 << 20)), ("2 KB".to_string(), "3.0 MB".to_string()));
    }
}

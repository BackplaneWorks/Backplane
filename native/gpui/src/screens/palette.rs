//! "Jump or run a command": one box over the window that finds a thread in any project, or an action, as you type, and does it on Enter
//! or a click. Threads come from the sidebar's rows (`projects`, or `active`), the actions are the window's and the open thread's own (its header tools, the viewer
//! kinds, Settings...). Opened from the bar at the top, the Commands item on the rail or Ctrl+Shift+P; Esc shuts it. The ranking is `rank`: what starts with the text,
//! then a word that does, then the letters in order.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, i18n::{tf, trs}, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Do { Thread(String), /** a message found by what it says: its thread and the message */ Message(String, String), Action(String, String), Settings, Sidebar, Models, Prev, Next, Uber, Shortcuts, /** streamer mode on or off (streamer.rs) */ Streamer, Hub(String) }
#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub label: String, pub place: String, pub kind: &'static str, pub what: Do, /** a thread's row (its status, state and want): its dot and what it wants */ pub row: Value }
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
pub struct Palette { pub focus: bool, pub open: bool, pub input: Option<Entity<Composer>>, pub q: String, pub ix: usize,
    /// "Search in thread": the thread (its raw id) the box searches, "" for the whole palette
    pub scope: String,
    /// Find across threads (Ctrl+Shift+F): the box lists only what was said, and the threads named by the words
    pub find_all: bool,
    /// what the hub found said (`thread.search`) for `hits_q`; the ask in flight is `asked` with its tag
    pub hits: Vec<Item>, pub hits_q: String, asked: (u64, String), seq: u64,
    /// the threads whose title has every word, from the hub (the sidebar's rows are only its first page), for `hits_q`
    pub titles: Vec<Item> }

/// Ask tags of the palette's searches (above the project page's).
pub const TAG: u64 = 1 << 49;
/// The search a text asks the hub for: the words of two characters or more (the hub's own rule), else none.
pub fn search_of(q: &str) -> Option<String> { let t = q.trim(); (t.chars().count() >= 2).then(|| t.to_string()) }
/// Where the text's first word is in a found line (bytes, case ignored), to mark it; none when lower-casing would move the bytes.
pub fn mark(label: &str, q: &str) -> Option<std::ops::Range<usize>> {
    let w = q.split_whitespace().next()?.to_lowercase(); let low = label.to_lowercase();
    if low.len() != label.len() || w.is_empty() { return None }
    low.find(&w).map(|a| a..a + w.len()).filter(|r| label.is_char_boundary(r.start) && label.is_char_boundary(r.end))
}
/// The hub's answer to `thread.search` as items: each message found, its line as the label, its thread's title and project as the place.
pub fn hit_items(result: &Value, hub_of: &dyn Fn(&str) -> String) -> Vec<Item> {
    result["hits"].as_array().into_iter().flatten().map(|h| {
        let (th, title, project) = (string(h, "thread"), string(h, "title"), string(h, "project"));
        /* i18n-ok: a title and project, not words */ let place = if project.is_empty() { title } else { format!("{title} · {project}") };
        /* i18n-ok: kinds, compared; drawn through trs */ Item { label: string(h, "text"), place, kind: if h["role"] == "user" { "you said" } else { "agent said" }, what: Do::Message(hub_of(&th), string(h, "msg")), row: Value::Null }
    }).collect()
}
/// The hub's title hits (`thread.search`'s `titles`) as thread items, with their project as the place.
pub fn title_items(result: &Value, hub_of: &dyn Fn(&str) -> String) -> Vec<Item> {
    result["titles"].as_array().into_iter().flatten().map(|t| Item { label: string(t, "title"), place: string(t, "project"), kind: "thread", what: Do::Thread(hub_of(&string(t, "thread"))), row: Value::Null }).collect()
}
/// The palette's answer (false: not the palette's tag).
pub fn answered(this: &mut Backplane, tag: u64, reply: &Value, cx: &mut Context<Backplane>) -> bool {
    if tag >= super::find::TAG { return super::find::answered(this, tag, reply, cx) } // the find bar's
    if tag < TAG { return false }
    let p = &mut this.screens.palette;
    if tag != p.asked.0 || reply["ok"] != true { return true }
    let data = this.data.clone();
    let hub_of = move |raw: &str| super::tools::qualify(&data, raw);
    p.hits = hit_items(&reply["result"], &hub_of); p.titles = title_items(&reply["result"], &hub_of); p.hits_q = p.asked.1.clone();
    cx.notify(); true
}

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
            v.push(Item { label: trs(&label), place: String::new(), kind: "action", what: Do::Action(action, value), row: Value::Null });
        }
        /* i18n-ok: viewer kinds, drawn through trs in the label */ for (l, k) in [("Board", "board"), ("Schematic", "schematic"), ("3D", "3d"), ("Gerbers", "gerbers"), ("Split", "split"), ("Mech", "mech"), ("Files", "files"), ("Review", "review"), ("Browser", "browser")] {
            v.push(Item { label: tf("gpui.palette.open", &[("kind", trs(l).into())]), place: trs("viewer"), kind: "action", what: Do::Action("view-kind".into(), k.into()), row: Value::Null });
        }
    }
    if thread.is_object() {
        v.push(Item { label: trs("Agent, model and effort"), place: String::new(), kind: "action", what: Do::Models, row: Value::Null });
        v.push(Item { label: trs("Previous thread"), place: String::new(), kind: "action", what: Do::Prev, row: Value::Null });
        v.push(Item { label: trs("Next thread"), place: String::new(), kind: "action", what: Do::Next, row: Value::Null });
    }
    if data["bots"].as_array().map_or(false, |b| b.iter().any(|b| b["uber"] == true)) { v.push(Item { label: trs("Uberbot chat"), place: String::new(), kind: "action", what: Do::Uber, row: Value::Null }); }
    v.push(Item { label: trs("Keyboard shortcuts"), place: String::new(), kind: "action", what: Do::Shortcuts, row: Value::Null });
    v.push(Item { label: trs("Settings"), place: String::new(), kind: "action", what: Do::Settings, row: Value::Null });
    v.push(Item { label: if crate::streamer::is_on() { trs("Streamer mode: turn off") } else { trs("Streamer mode: hide emails, paths and machine names") }, place: String::new(), kind: "action", what: Do::Streamer, row: Value::Null });
    v.push(Item { label: trs("Fold or unfold the sidebar"), place: String::new(), kind: "action", what: Do::Sidebar, row: Value::Null });
    // always here, whatever the hub sends or fails to send: these do not go through it (hubbar.rs)
    for a in ["restart", "safe", "rollback"] { v.push(Item { label: trs(super::hubbar::label_of(a)), place: String::new(), kind: "action", what: Do::Hub(a.into()), row: Value::Null }); }
    for p in data["projects"].as_array().into_iter().flatten() {
        let (pid, ptitle) = (string(p, "id"), string(p, "title"));
        let raw = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
        v.push(Item { label: tf("gpui.palette.new.thread.in", &[("project", ptitle.as_str().into())]), place: String::new(), kind: "action", what: Do::Action("new-thread".into(), raw), row: Value::Null });
        for t in p["threads"].as_array().into_iter().flatten().chain(p["settled"].as_array().into_iter().flatten()) {
            v.push(Item { label: string(t, "title"), place: on_machine(&ptitle, t), kind: "thread", what: Do::Thread(string(t, "id")), row: row_of(t) });
        }
    }
    // bots and rooms are places to jump to as threads are: the sidebar's own rows, by name. A bot is its chat thread (so the hub's title hit for that thread is the same item).
    for b in data["bots"].as_array().into_iter().flatten() {
        let (name, nav) = (string(b, "name"), string(b, "nav"));
        if name.is_empty() { continue }
        /* i18n-ok: a kind, drawn through trs */ v.push(Item { label: name, place: String::new(), kind: "bot", what: if nav.is_empty() || nav.starts_with('@') { Do::Action("bot".into(), string(b, "id")) } else { Do::Thread(nav) }, row: Value::Null });
    }
    for r in data["rooms"].as_array().into_iter().flatten() {
        let (id, name) = (string(r, "id"), string(r, "name"));
        if id.is_empty() || name.is_empty() { continue }
        /* i18n-ok: a kind, drawn through trs */ v.push(Item { label: name, place: String::new(), kind: "room", what: Do::Action("room".into(), id), row: Value::Null });
    }
    for t in data["conversationHistory"]["jump"].as_array().into_iter().flatten().chain(data["legacy"].as_array().into_iter().flatten()) {
        v.push(Item { label: string(t, "title"), place: trs("Conversation history"), kind: "thread", what: Do::Thread(string(t, "id")), row: Value::Null });
    }
    // every project and thread on every machine (the hub's `jump`, present/sections.rs): a project opens its page, a thread opens; one on a machine that is off is listed and opens nothing
    for j in data["jump"].as_array().into_iter().flatten() {
        let (action, value) = (string(j, "action"), string(j, "value"));
        if action.is_empty() { continue }
        let what = if action == "select" { Do::Thread(value) } else { Do::Action(action, value) };
        let kind = if j["kind"] == "project" { "project" } else { "thread" };
        v.push(Item { label: string(j, "label"), place: string(j, "place"), kind, what, row: row_of(j) });
    }
    for t in data["active"].as_array().into_iter().flatten().chain(data["activeSettled"]["rows"].as_array().into_iter().flatten()) {
        v.push(Item { label: string(t, "title"), place: on_machine(&string(t, "project"), t), kind: "thread", what: Do::Thread(string(t, "id")), row: row_of(t) });
    }
    v
}

/// A thread row's dot and want, as the palette keeps it.
fn row_of(t: &Value) -> Value { serde_json::json!({"status": t["status"], "state": t["state"], "want": t["want"]}) }
/// How much a thread waits on the person (0.14's Urg.rank, its top: an approval, then a question, then a failed turn); 0 for anything else.
pub fn urgency(it: &Item) -> u8 { match it.row["want"].as_str().unwrap_or("") { "approval" => 3, "question" => 2, "failed" => 1, _ => 0 } }

/// The items the text keeps, best first (threads before actions among equals), at most `n`. Opened with nothing typed, the threads that
/// wait on the person come first (r04: Ctrl+K, Enter goes to the one waiting).
pub fn found(all: Vec<Item>, q: &str, n: usize) -> Vec<Item> {
    let empty = q.trim().is_empty();
    let mut r: Vec<(u8, usize, Item)> = all.into_iter().enumerate().filter_map(|(i, it)| { let s = rank(&it.label, q).max(if it.place.is_empty() { 0 } else { rank(&it.place, q).min(1) }); (s > 0).then_some((s, i, it)) }).collect();
    r.sort_by(|a, b| (if empty { urgency(&b.2).cmp(&urgency(&a.2)) } else { std::cmp::Ordering::Equal }).then(b.0.cmp(&a.0)).then((a.2.kind != "thread").cmp(&(b.2.kind != "thread"))).then(a.1.cmp(&b.1)));
    let mut seen = std::collections::HashSet::new();
    r.into_iter().map(|x| x.2).filter(|it| seen.insert((it.label.clone(), it.place.clone(), it.kind))).take(n).collect()
}

/// What the box lists for this text: in a thread's search only what was said there; else the jumps and commands, then what was said.
pub fn listed(data: &Value, p: &Palette) -> Vec<Item> {
    let answered = search_of(&p.q).as_deref() == Some(p.hits_q.as_str());
    let hits: Vec<Item> = if answered { p.hits.clone() } else { vec![] };
    if !p.scope.is_empty() { return hits }
    // Find across threads: what was said, then the threads whose title has the words
    if p.find_all { let mut v = hits; if answered { v.extend(p.titles.iter().take(6).cloned()); } return v }
    // the sidebar's rows, then the hub's title hits for threads it does not hold (settled, older pages)
    let mut all = items(data);
    if answered { let held: std::collections::HashSet<Do> = all.iter().map(|it| it.what.clone()).collect(); all.extend(p.titles.iter().filter(|t| !held.contains(&t.what)).cloned()); }
    let mut v = found(all, &p.q, 10);
    let room = 14usize.saturating_sub(v.len());
    v.extend(hits.into_iter().take(room)); v
}

impl Backplane {
    /// The text changed: after a pause, the hub is asked what was said with it (the last answer that matches the text is shown).
    fn palette_search(&mut self, cx: &mut Context<Self>) {
        let Some(q) = search_of(&self.screens.palette.q) else { return };
        if q == self.screens.palette.hits_q || q == self.screens.palette.asked.1 { return }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(220)).await;
            let _ = this.update(cx, |this, _| {
                let p = &mut this.screens.palette;
                if search_of(&p.q).as_deref() != Some(q.as_str()) || !p.open { return }
                p.seq += 1; let tag = TAG + p.seq; p.asked = (tag, q.clone());
                let mut params = serde_json::json!({"q": q, "limit": if p.find_all { 40 } else if p.scope.is_empty() { 20 } else { 60 }});
                if !p.scope.is_empty() { params["thread"] = serde_json::json!(p.scope); }
                let _ = this.tx.try_send(Command::Ask("thread.search".into(), params, tag));
            });
        }).detach();
    }
    /// Find across threads (Ctrl+Shift+F, 0.14's Find): the box finds what was said in any thread; a pick opens its thread at the message.
    pub(crate) fn palette_open_find(&mut self, cx: &mut Context<Self>) {
        self.palette_open(cx);
        self.screens.palette.find_all = true;
        /* i18n-ok: placeholders drawn through trs by the input, and the text through trs */ if let Some(i) = self.screens.palette.input.clone() { i.update(cx, |i, _| i.set_placeholder("Type a word said in any thread")); }
    }
    pub(crate) fn palette_open(&mut self, cx: &mut Context<Self>) {
        if self.screens.palette.input.is_none() {
            let input = cx.new(|cx| Composer::with_context(cx, "PickInput"));
            /* i18n-ok: hints, drawn through trs by the composer */ input.update(cx, |i, _| { i.set_placeholder("Jump to a thread or run a command"); });
            cx.observe(&input, |this, input, cx| { let q = input.read(cx).content.to_string(); if q != this.screens.palette.q { this.screens.palette.q = q; this.screens.palette.ix = 0; this.palette_search(cx); } cx.notify(); }).detach();
            cx.subscribe(&input, |this, _, ev: &ComposerEvent, cx| match ev {
                ComposerEvent::Submit | ComposerEvent::PopupPick => this.palette_run(None, cx),
                ComposerEvent::PopupMove(d) => { let n = listed(&this.data, &this.screens.palette).len().max(1); this.screens.palette.ix = (this.screens.palette.ix as i32 + d).rem_euclid(n as i32) as usize; cx.notify(); }
                _ => {}
            }).detach();
            self.screens.palette.input = Some(input);
        }
        let input = self.screens.palette.input.clone().unwrap();
        /* i18n-ok: placeholders drawn through trs by the input, and the text through trs */ input.update(cx, |i, cx| { i.set(String::new(), cx); i.popup_open = true; i.set_placeholder("Jump to a thread, run a command or find what was said"); });
        self.screens.palette.q.clear(); self.screens.palette.ix = 0; self.screens.palette.open = true; self.screens.palette.scope.clear(); self.screens.palette.find_all = false;
        // what an earlier opening found is not this one's (a thread's search must never show another thread's hits)
        self.screens.palette.hits.clear(); self.screens.palette.titles.clear(); self.screens.palette.hits_q.clear(); self.screens.palette.asked = (0, String::new());
        self.screens.palette.focus = true; cx.notify();
    }
    pub(crate) fn palette_close(&mut self, cx: &mut Context<Self>) -> bool {
        let was = self.screens.palette.open; self.screens.palette.open = false;
        if was { self.want_focus = true; cx.notify(); } was
    }
    /// Do the highlighted item (or the one at `i`) and shut the box.
    fn palette_run(&mut self, i: Option<usize>, cx: &mut Context<Self>) {
        let list = listed(&self.data, &self.screens.palette);
        let Some(it) = list.get(i.unwrap_or(self.screens.palette.ix)).cloned() else { return };
        self.screens.palette.open = false; self.want_focus = true;
        match it.what {
            // a thread of this hub: as its sidebar row does (the thread drawn at once from the cache, its viewer and editor as they were left);
            // the hub's `select` action left a thread with its viewer open closed (the embedded KiCad gone with it)
            Do::Thread(id) if it.kind == "thread" && !id.contains('~') => self.select_id(id, cx),
            Do::Thread(id) => self.go("select", id, cx),
            // a message found: its thread at the message, marked, with the find bar holding the words for the next and previous ones there
            Do::Message(id, msg) => { let q = self.screens.palette.q.trim().to_string(); self.find_open_at(&id, &msg, &q, cx); }
            Do::Action(a, v) => {
                if super::tools::makes_thread(&a) { self.arm_new(); }
                // a project from the jump list: its page opens here too (the hub fills its lists)
                let page = (a == "project-page").then(|| self.data["jump"].as_array().into_iter().flatten().find(|j| j["kind"] == "project" && j["value"] == v.as_str()).map(|j| string(j, "pid"))).flatten().filter(|p| !p.is_empty());
                self.go(&a, v, cx);
                if let Some(pid) = page { self.open_project(pid, cx); }
            }
            Do::Settings => self.open_settings(cx),
            Do::Sidebar => self.sidebar_toggle(cx),
            Do::Models => self.models_pending = true,
            Do::Prev | Do::Next => { let n = crate::keys::neighbour(&self.data, &self.selected, it.what == Do::Next); if let Some(id) = n { self.select_id(id, cx); } }
            Do::Uber => { let id = self.data["bots"].as_array().into_iter().flatten().find(|b| b["uber"] == true).map(|b| crate::string(b, "id")); if let Some(id) = id { self.go("bot", id, cx); } }
            Do::Shortcuts => self.keysheet_toggle(cx),
            Do::Streamer => self.streamer_toggle(cx),
            Do::Hub(a) => self.hub_click(&a, cx),
        }
        cx.notify();
    }
}

pub fn view(cx: &mut Context<Backplane>, light: bool, data: &Value, p: &Palette, l: &crate::local::Local, sel: &str) -> Option<AnyElement> {
    if !p.open { return None }
    let c = |n: &str| theme::color(light, n);
    let list = listed(data, p);
    let ix = p.ix.min(list.len().saturating_sub(1));
    let current = crate::keys::project_of(data, sel);
    let mut col = div().flex().flex_col().py_1();
    /* i18n-ok: placeholders drawn through trs by the input, and the text through trs */ if list.is_empty() { col = col.child(div().px_3().py_1().text_color(c("faint")).child(crate::i18n::trs(if (!p.scope.is_empty() || p.find_all) && search_of(&p.q).is_none() { if p.find_all { "Type a word said in any thread" } else { "Type a word said in this thread" } } else { "Nothing matches" }))); }
    for (i, it) in list.iter().enumerate() {
        let want = it.row["want"].as_str().unwrap_or("").to_string();
        col = col.child(div().id(SharedString::from(format!("pal-{i}"))).flex().items_center().gap_3().px_3().py_1().cursor_pointer().when(i == ix, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
            .children(it.row.is_object().then(|| super::siderow::dot(light, &it.row)))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().when(matches!(it.what, Do::Message(..)), |d| d.text_color(c("dim")))
                .child(match (&it.what, mark(&it.label, &p.q)) { // a message found: the word it was found by stands out (0.14 marked it)
                    (Do::Message(..), Some(r)) => StyledText::new(it.label.clone()).with_highlights([(r, HighlightStyle { color: Some(c("text").into()), font_weight: Some(FontWeight::BOLD), ..Default::default() })]).into_any_element(),
                    _ => it.label.clone().into_any_element(),
                }))
            .when(!want.is_empty(), |d| d.child(div().text_size(px(11.)).text_color(c(if want == "failed" { "dim" } else { "bad" })).child(trs(&want))))
            .child(div().text_size(px(11.)).text_color(c("dim")).child(shortcut(it, &current, l)))
            .child(div().flex_shrink_0().max_w(px(220.)).overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(px(11.)).text_color(c("faint")).child(if it.place.is_empty() { trs(it.kind) } else { it.place.clone() }))
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
    #[test] fn a_thread_the_sidebar_does_not_hold_is_found_by_its_title_from_the_hub() {
        let res = json!({"hits": [], "titles": [{"thread": "t1", "title": "Board layout review", "project": "Demo"}, {"thread": "t9", "title": "Old board notes", "project": "Amp"}]});
        let titles = title_items(&res, &|raw| format!("h|{raw}"));
        let mut p = Palette { q: "board".into(), titles, hits_q: "board".into(), ..Default::default() };
        let l = listed(&data(), &p);
        assert_eq!(l.iter().filter(|i| i.what == Do::Thread("h|t1".into())).count(), 1, "a held row is listed once");
        assert!(l.iter().any(|i| i.what == Do::Thread("h|t9".into()) && i.place == "Amp" && i.kind == "thread"), "{l:?}");
        p.q = "boar".into(); assert!(!listed(&data(), &p).iter().any(|i| i.what == Do::Thread("h|t9".into())), "an answer for another text is not shown");
    }
    #[test] fn a_bot_and_a_room_are_found_by_name() {
        let d = json!({"bots": [{"id": "h|b1", "name": "foreman", "nav": "h|t7"}, {"id": "h|b2", "name": "far cat", "nav": "@remote:b2"}], "rooms": [{"id": "h|r1", "name": "shop floor"}]});
        let p = Palette { q: "shop".into(), ..Default::default() };
        let l = listed(&d, &p); assert_eq!((l[0].label.as_str(), l[0].kind, l[0].what.clone()), ("shop floor", "room", Do::Action("room".into(), "h|r1".into())));
        // a bot is its chat thread: the hub's title hit for that thread is the same item, listed once, as a bot
        let res = json!({"hits": [], "titles": [{"thread": "t7", "title": "foreman", "project": ""}]});
        let p = Palette { q: "fore".into(), titles: title_items(&res, &|raw| format!("h|{raw}")), hits_q: "fore".into(), ..Default::default() };
        let l = listed(&d, &p); assert_eq!(l.iter().filter(|i| i.kind != "action").map(|i| (i.label.as_str(), i.kind)).collect::<Vec<_>>(), [("foreman", "bot")]);
        assert_eq!(l[0].what, Do::Thread("h|t7".into()));
        let far = items(&d).into_iter().find(|i| i.label == "far cat").unwrap(); assert_eq!(far.what, Do::Action("bot".into(), "h|b2".into()));
    }
    #[test] fn an_old_scratch_chat_is_found_by_its_title() {
        let d = serde_json::json!({"conversationHistory": {"jump": [{"id": "h|s1", "title": "zebra scratch chat"}]}, "legacy": [{"id": "h|s2", "title": "live chat"}]});
        let found: Vec<_> = items(&d).into_iter().filter(|i| i.kind == "thread").collect();
        assert_eq!(found.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["zebra scratch chat", "live chat"]);
        assert_eq!(found[0].what, Do::Thread("h|s1".into())); assert_eq!(found[0].place, "Conversation history");
    }
    #[test] fn threads_and_actions_are_found_by_what_is_typed_and_a_delete_is_never_offered() {
        let all = items(&data());
        assert!(all.iter().any(|i| i.what == Do::Thread("h|t3".into())) && all.iter().any(|i| i.what == Do::Action("pin".into(), String::new())) && all.iter().any(|i| i.what == Do::Settings));
        assert!(!all.iter().any(|i| i.what == Do::Action("delete".into(), String::new())), "Delete asks first and is not a palette action");
        let r = found(all.clone(), "pow", 10); assert_eq!(r[0].label, "Power budget");
        assert!(found(all.clone(), "zzzz", 10).is_empty());
        let r = found(all, "new thr", 10); assert_eq!(r[0].what, Do::Action("new-thread".into(), "p1".into()), "the project's raw id");
    }
    #[test] fn opened_empty_the_threads_waiting_on_the_person_come_first() {
        let d = json!({"projects": [{"id": "h|p1", "title": "Amp", "threads": [{"id": "h|t1", "title": "Quiet one", "status": "inactive", "want": ""},
            {"id": "h|t2", "title": "Firmware pin map", "status": "failed", "want": "failed"}, {"id": "h|t3", "title": "Battery life", "status": "blocked", "want": "approval"},
            {"id": "h|t4", "title": "Grounding plan", "status": "blocked", "want": "question"}]}]});
        let r = found(items(&d), "", 10);
        assert_eq!(r[..3].iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["Battery life", "Grounding plan", "Firmware pin map"], "approval, question, failed (Urg.rank)");
        assert_eq!(r[0].row["want"], "approval");
        assert_eq!(found(items(&d), "quiet", 10)[0].label, "Quiet one", "a typed query ranks as before");
    }
    #[test] fn what_was_said_is_listed_after_the_jumps_and_alone_in_a_threads_search() {
        let res = json!({"hits": [{"thread": "t2", "title": "Firmware pin map", "project": "Sensor", "msg": "m1", "role": "user", "text": "where does the pinecone go"}]});
        let hits = hit_items(&res, &|raw| format!("h|{raw}"));
        assert_eq!(hits[0].what, Do::Message("h|t2".into(), "m1".into())); assert_eq!(hits[0].place, "Firmware pin map · Sensor"); assert_eq!(hits[0].kind, "you said");
        let mut p = Palette { q: "pinecone".into(), hits: hits.clone(), hits_q: "pinecone".into(), ..Default::default() };
        let l = listed(&data(), &p); assert_eq!(l.last().unwrap().what, Do::Message("h|t2".into(), "m1".into()));
        p.q = "pine".into(); assert!(listed(&data(), &p).iter().all(|i| !matches!(i.what, Do::Message(..))), "an answer for another text is not shown");
        p.q = "pinecone".into(); p.scope = "t2".into(); assert_eq!(listed(&data(), &p).len(), 1, "a thread's search lists only what was said there");
        assert!(search_of("p").is_none() && search_of(" pi ").as_deref() == Some("pi"));
        assert_eq!(mark("…the Pinecone goes", "pinecone sits"), Some(7..15), "bytes: … is three"); assert_eq!(mark("nothing", "pine"), None);
    }
    #[test] fn the_list_is_bounded_and_each_thing_is_listed_once() {
        let mut d = data(); d["active"] = json!([{"id": "h|t1", "title": "Board layout review", "project": "Demo"}]);
        let r = found(items(&d), "", 100); assert_eq!(r.iter().filter(|i| i.label == "Board layout review").count(), 1, "the same thread from two lists shows once");
        assert!(found(items(&d), "", 3).len() == 3);
    }
}

/// A thread's place in the palette: its project, and the machine it runs on when that is another one (its marker's short name).
fn on_machine(place: &str, t: &serde_json::Value) -> String {
    match super::badge::machine_text(&t["mark"], false) { Some((m, _, _)) => format!("{place} · {m}"), None => place.to_string() }
}
#[cfg(test)]
mod machine_tests {
    #[test] fn another_machines_thread_names_it_in_the_palette() {
        assert_eq!(super::on_machine("Backplane", &serde_json::json!({"mark":{"name":"framework-1","short":"framework-1","away":false}})), "Backplane · framework-1");
        assert_eq!(super::on_machine("Backplane", &serde_json::json!({})), "Backplane");
    }
}

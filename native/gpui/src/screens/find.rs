//! Find in a thread (Ctrl+F, the ▾ menu's "Find in thread", or a message picked in Find across threads): a bar over the thread with the
//! words, "n of total", previous and next (Shift+Enter, Enter, the arrows) and close (Esc, which clears the mark). The matches are the hub's
//! (`thread.find`: every message of the thread with every word, oldest first); the bar steps through them and the timeline goes to each,
//! reading older pages until the message is held, and marks it. What the bar decides is in the pure functions below.
use super::composer::{Composer, ComposerEvent};
use crate::{hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

/// Ask tags of the bar's finds (above the palette's).
pub const TAG: u64 = 1 << 50;
/// Older pages read at most while going to one message (a page holds 32 entries).
const PAGES: u32 = 400;

#[derive(Default)]
pub struct Find {
    pub open: bool, pub focus: bool, pub input: Option<Entity<Composer>>, pub q: String,
    /// the thread (its raw id) the bar finds in
    pub thread: String,
    /// the hub's matches for `hits_q` (message ids, oldest first), how many in all, and the one on show
    pub matches: Vec<String>, pub total: usize, pub more: bool, pub ix: usize, pub hits_q: String,
    asked: (u64, String), seq: u64,
    /// the match to show first when the answer comes (a message picked in Find across threads)
    pub want: String,
    /// the message marked in the timeline, and the one still being looked for (older pages on their way) with the pages read for it
    pub mark: String, pub going: Option<String>, pub pages: u32,
}

/// The query the hub is asked for: two characters or more (its own rule).
pub fn query_of(q: &str) -> Option<String> { let t = q.trim(); (t.chars().count() >= 2).then(|| t.to_string()) }
/// The next (or previous) match, wrapping round.
pub fn step(ix: usize, n: usize, forward: bool) -> usize { if n == 0 { 0 } else if forward { (ix + 1) % n } else { (ix + n - 1) % n } }
/// The match to show when an answer comes: `want` when it is there, else the newest (the last).
pub fn first(matches: &[String], want: &str) -> usize { matches.iter().position(|m| !want.is_empty() && m == want).unwrap_or(matches.len().saturating_sub(1)) }
/// What the bar says: "3 of 12" ("3 of 1000+" past the hub's cap), "No matches", or nothing before an answer.
pub fn count(ix: usize, n: usize, more: bool, answered: bool) -> String {
    if !answered { return String::new() }
    if n == 0 { return "No matches".into() }
    format!("{} of {}{}", ix + 1, n, if more { "+" } else { "" })
}
/// The matches of a `thread.find` answer.
pub fn matches_of(result: &Value) -> (Vec<String>, usize, bool) {
    let m: Vec<String> = result["matches"].as_array().into_iter().flatten().filter_map(|h| h["msg"].as_str().map(str::to_string)).collect();
    let total = result["total"].as_u64().map_or(m.len(), |t| t as usize);
    (m, total, result["more"] == true)
}
/// Where a message is in the rows held: its own row (a message is a row of its id).
pub fn row_of(rows: &[Value], msg: &str) -> Option<usize> { rows.iter().position(|e| e["id"].as_str() == Some(msg)) }

/// The bar's answer (false: not the bar's tag).
pub fn answered(this: &mut Backplane, tag: u64, reply: &Value, cx: &mut Context<Backplane>) -> bool {
    if tag < TAG { return false }
    let f = &mut this.screens.find;
    if tag != f.asked.0 || reply["ok"] != true { return true }
    let (m, total, more) = matches_of(&reply["result"]);
    f.ix = first(&m, &f.want); f.want.clear(); f.matches = m; f.total = total; f.more = more; f.hits_q = f.asked.1.clone();
    match f.matches.get(f.ix).cloned() { Some(msg) => this.find_go(msg), None => { this.screens.find.mark.clear(); this.screens.find.going = None; } }
    cx.notify(); true
}

impl Backplane {
    fn find_input(&mut self, cx: &mut Context<Self>) -> Entity<Composer> {
        if let Some(i) = self.screens.find.input.clone() { return i }
        let input = cx.new(|cx| Composer::with_context(cx, "FindInput"));
        input.update(cx, |i, _| i.set_placeholder("Find in this thread"));
        cx.observe(&input, |this, input, cx| { let q = input.read(cx).content.to_string(); if q != this.screens.find.q { this.screens.find.q = q; this.find_ask(cx); } cx.notify(); }).detach();
        cx.subscribe(&input, |this, _, ev: &ComposerEvent, cx| match ev {
            ComposerEvent::Submit => this.find_step(true, cx),
            ComposerEvent::SubmitBack => this.find_step(false, cx),
            _ => {}
        }).detach();
        self.screens.find.input = Some(input.clone()); input
    }
    /// Ctrl+F: the bar over the thread on show, the keys in it (its words kept from the last time in this thread).
    pub(crate) fn find_open(&mut self, cx: &mut Context<Self>) { if self.data["thread"].is_object() { let id = self.selected.clone(); self.find_open_for(&id, cx) } }
    /// The bar for `thread` (a qualified or raw id: the ▾ menu's row), which is opened when it is not the one on show.
    pub(crate) fn find_open_for(&mut self, thread: &str, cx: &mut Context<Self>) {
        let raw = thread.rsplit('|').next().unwrap_or("").to_string();
        if raw.is_empty() { return }
        self.find_input(cx);
        if self.screens.find.thread != raw { self.find_reset(cx); self.screens.find.thread = raw.clone(); }
        if self.selected.rsplit('|').next() != Some(raw.as_str()) { let id = if thread.contains('|') { thread.to_string() } else { super::tools::qualify(&self.data, &raw) }; let _ = self.tx.try_send(Command::Action("select".into(), id)); }
        self.screens.find.open = true; self.screens.find.focus = true;
        if !self.screens.find.mark.is_empty() || self.screens.find.hits_q.is_empty() { self.find_ask(cx); }
        cx.notify();
    }
    /// A message found elsewhere (Find across threads): its thread, the bar holding the words, and that message shown first and marked.
    pub(crate) fn find_open_at(&mut self, thread: &str, msg: &str, q: &str, cx: &mut Context<Self>) {
        let raw = thread.rsplit('|').next().unwrap_or("").to_string();
        let input = self.find_input(cx);
        self.find_reset(cx);
        self.screens.find.thread = raw.clone(); self.screens.find.open = true; self.screens.find.focus = true; self.screens.find.want = msg.to_string(); self.screens.find.q = q.to_string();
        input.update(cx, |i, cx| i.set(q.to_string(), cx));
        self.find_go(msg.to_string());
        if self.selected.rsplit('|').next() != Some(raw.as_str()) { let _ = self.tx.try_send(Command::Action("select".into(), thread.to_string())); }
        self.find_ask(cx);
        cx.notify();
    }
    /// Shut the bar: the mark goes, the keys go back to the message box.
    pub(crate) fn find_close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.screens.find.open && self.screens.find.mark.is_empty() { return false }
        self.screens.find.open = false; self.screens.find.mark.clear(); self.screens.find.going = None; self.want_focus = true; cx.notify(); true
    }
    fn find_reset(&mut self, cx: &mut Context<Self>) {
        let f = &mut self.screens.find;
        f.matches.clear(); f.total = 0; f.more = false; f.ix = 0; f.hits_q.clear(); f.asked = (0, String::new()); f.mark.clear(); f.going = None; f.want.clear(); f.q.clear();
        if let Some(i) = f.input.clone() { i.update(cx, |i, cx| i.set(String::new(), cx)); }
    }
    /// The words changed: after a pause the hub is asked for the thread's matches (the last answer for the words on show is the one kept).
    fn find_ask(&mut self, cx: &mut Context<Self>) {
        let Some(q) = query_of(&self.screens.find.q) else { let f = &mut self.screens.find; f.matches.clear(); f.total = 0; f.hits_q.clear(); f.mark.clear(); f.going = None; return };
        let thread = self.screens.find.thread.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(220)).await;
            let _ = this.update(cx, |this, _| {
                let f = &mut this.screens.find;
                if query_of(&f.q).as_deref() != Some(q.as_str()) || f.thread != thread || !f.open { return }
                f.seq += 1; let tag = TAG + f.seq; f.asked = (tag, q.clone());
                let _ = this.tx.try_send(Command::Ask("thread.find".into(), serde_json::json!({"thread": thread, "q": q}), tag));
            });
        }).detach();
    }
    /// Enter / the down arrow: the next match; Shift+Enter / the up arrow: the previous.
    pub(crate) fn find_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let f = &mut self.screens.find;
        if f.matches.is_empty() { return }
        f.ix = step(f.ix, f.matches.len(), forward);
        let msg = f.matches[f.ix].clone(); self.find_go(msg); cx.notify();
    }
    /// Show a message: marked, and the timeline at it once its row is held.
    pub(crate) fn find_go(&mut self, msg: String) {
        self.screens.find.mark = msg.clone(); self.screens.find.going = Some(msg); self.screens.find.pages = 0;
        self.find_landed();
    }
    /// The rows held changed (a picture, an older page): go to the message looked for when it is there, else read the next older page.
    pub(crate) fn find_landed(&mut self) {
        let Some(msg) = self.screens.find.going.clone() else { return };
        let raw = crate::string(&self.data["thread"], "id"); let raw = raw.rsplit('|').next().unwrap_or("");
        if raw != self.screens.find.thread || self.data["thread"]["loading"] == true { return } // its thread is still on its way
        self.screens.seeking = None; // a place kept from the last visit gives way to the message asked for
        if let Some(ix) = self.data["thread"]["entries"].as_array().and_then(|r| row_of(r, &msg)) {
            self.timeline.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(0.) });
            self.screens.find.going = None; return;
        }
        let older = crate::string(&self.data["thread"], "historyOlder");
        if older.is_empty() || self.screens.find.pages >= PAGES { self.screens.find.going = None; return; } // the start of the thread: it is not there
        if self.screens.paging.is_some() { return } // a page is on its way; this runs again when it lands
        if self.tx.try_send(Command::History(older)).is_ok() { self.screens.paging = Some(std::time::Instant::now()); self.screens.find.pages += 1; }
    }
}

/// The bar, at the top right of the thread.
pub fn bar(cx: &mut Context<Backplane>, light: bool, f: &Find) -> Option<AnyElement> {
    if !f.open { return None }
    let c = |n: &str| theme::color(light, n);
    let answered = query_of(&f.q).as_deref() == Some(f.hits_q.as_str());
    let label = count(f.ix, f.matches.len(), f.more, answered);
    // previous and next as arrows in the text (the icon set has no up chevron and is generated, assets/GENERATED.md)
    let btn = |id: &'static str, glyph: &'static str, tip: &'static str| crate::ui::with_rect(div().id(id), move || format!("ui:{tip}")).cursor_pointer().w(px(22.)).h(px(22.)).flex().items_center().justify_center().text_color(c("dim")).hover(|s| s.bg(c("raise")).text_color(c("text"))).child(glyph);
    // a strip over the thread's rows (never over them: the message it marks would hide under it), the bar at its right
    Some(div().w_full().flex_shrink_0().flex().justify_end().px_4().py_1().border_b_1().border_color(c("line")).child(div().id("find-bar").w(px(400.)).max_w_full().flex().items_center().gap_2().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(13.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(crate::art::icon("search", 13., c("dim")))
        .child(div().flex_1().min_w_0().children(f.input.clone()))
        .child(div().flex_shrink_0().text_size(px(11.)).text_color(c(if answered && f.matches.is_empty() { "bad" } else { "faint" })).child(label))
        .child(btn("find-prev", "↑", "Previous match").on_click(cx.listener(|this, _, _, cx| this.find_step(false, cx))))
        .child(btn("find-next", "↓", "Next match").on_click(cx.listener(|this, _, _, cx| this.find_step(true, cx))))
        .child(crate::ui::with_rect(div().id("find-close"), || "ui:Close find".to_string()).cursor_pointer().p_1().hover(|s| s.bg(c("raise"))).child(crate::art::icon("x", 14., c("dim"))).on_click(cx.listener(|this, _, _, cx| { this.find_close(cx); }))))
        .into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use serde_json::json;
    #[test] fn next_and_previous_wrap_round() {
        assert_eq!((step(0, 3, true), step(2, 3, true), step(0, 3, false), step(1, 3, false)), (1, 0, 2, 0));
        assert_eq!(step(0, 0, true), 0, "no matches: nowhere to go");
    }
    #[test] fn an_answer_shows_the_message_asked_for_else_the_newest() {
        let m: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!((first(&m, "b"), first(&m, ""), first(&m, "gone"), first(&[], "")), (1, 2, 2, 0));
    }
    #[test] fn the_count_says_where_the_reader_is() {
        assert_eq!(count(2, 12, false, true), "3 of 12"); assert_eq!(count(0, 1000, true, true), "1 of 1000+");
        assert_eq!(count(0, 0, false, true), "No matches"); assert_eq!(count(0, 0, false, false), "", "nothing before the hub answers");
    }
    #[test] fn the_hubs_answer_is_its_matches_in_order() {
        let r = json!({"q": "pine", "matches": [{"msg": "m1"}, {"msg": "m4"}], "total": 2, "more": false});
        assert_eq!(matches_of(&r), (vec!["m1".to_string(), "m4".to_string()], 2, false));
        assert_eq!(query_of(" p "), None); assert_eq!(query_of(" pi "), Some("pi".into()));
        let rows = vec![json!({"id": "a1"}), json!({"id": "m4"})]; assert_eq!(row_of(&rows, "m4"), Some(1)); assert_eq!(row_of(&rows, "m9"), None);
    }
}

//! Find in a thread (Ctrl+F, the ▾ menu's "Find in thread", or a message picked in Find across threads): a bar over the thread with the
//! words, "n of total", previous and next (Shift+Enter, Enter, the arrows) and close (Esc, which clears the mark). The matches are the hub's
//! (`thread.find`: every message of the thread with every word, oldest first); the bar steps through them and the timeline goes to each,
//! reading older pages until the message is held, and marks it. The rows this window holds are searched too, whatever their kind (tool rows,
//! fold heads, the worktree line, stamps, cards: `local_matches`), at once on each change of the words; the hub's answer is merged with them
//! when it comes (`merge`). What the bar decides is in the pure functions below.
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
    /// the stops for `hits_q` (row ids, oldest first: the hub's and the rows held here merged), how many the hub counts, and the one on show
    pub matches: Vec<String>, pub total: usize, pub more: bool, pub ix: usize, pub hits_q: String,
    /// the hub's matches (message ids, oldest first) for `hub_q`, and the rows held here that hold the words of `hits_q`
    pub hub: Vec<String>, pub hub_q: String, pub local: Vec<String>,
    asked: (u64, String), seq: u64,
    /// the match to show first when the answer comes (a message picked in Find across threads)
    pub want: String,
    /// the message marked in the timeline, and the one still being looked for (older pages on their way) with the pages read for it
    pub mark: String, pub going: Option<String>, pub pages: u32,
    /// the message the timeline went to, its words not yet in view: the frames waited for its row to be drawn (`find_seek`)
    pub seek: Option<(String, u8)>,
    /// which of the message's places to show when it is drawn, the one on show and how many its drawn text holds (0: not known yet)
    pub at: At, pub occ: usize, pub occs: usize,
}

/// Which place in a message a step goes to: its first (going forward), its last (coming back from the next message), or the k-th.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum At { #[default] First, Last, Nth(usize) }
/// Where Enter (`forward`) or Shift+Enter goes from place `occ` of the message `ix` (of `n` messages), whose drawn text holds `occs`
/// places (0: none found there): the next place in the same message, else the next message's first (the previous one's last going back),
/// wrapping round at both ends. `(message, place)`.
pub fn step_at(ix: usize, occ: usize, occs: usize, n: usize, forward: bool) -> (usize, At) {
    if n == 0 { return (0, At::First) }
    if forward { if occ + 1 < occs { (ix, At::Nth(occ + 1)) } else { (step(ix, n, true), At::First) } }
    else if occ > 0 && occs > 0 { (ix, At::Nth((occ - 1).min(occs - 1))) } else { (step(ix, n, false), At::Last) }
}
/// The place `at` names among `occs` places.
pub fn place_of(at: At, occs: usize) -> usize { match at { At::First => 0, At::Last => occs.saturating_sub(1), At::Nth(k) => k.min(occs.saturating_sub(1)) } }

/// The query the hub is asked for: two characters or more (its own rule).
pub fn query_of(q: &str) -> Option<String> { let t = q.trim(); (t.chars().count() >= 2).then(|| t.to_string()) }
/// The next (or previous) match, wrapping round.
pub fn step(ix: usize, n: usize, forward: bool) -> usize { if n == 0 { 0 } else if forward { (ix + 1) % n } else { (ix + n - 1) % n } }
/// What the bar says: "3 of 12" ("3 of 1000+" past the hub's cap) for the messages, then the place within the message on show when its
/// drawn text holds more than one ("3 of 12 · 2/5"); "No matches", or nothing before an answer.
pub fn count(ix: usize, n: usize, more: bool, answered: bool, occ: usize, occs: usize) -> String {
    if !answered { return String::new() }
    /* i18n-ok: a control id */ if n == 0 { return crate::i18n::trs("No matches") }
    let within = if occs > 1 { format!(" \u{b7} {}/{}", occ + 1, occs) } else { String::new() };
    /* i18n-ok: numbers round "of", as the counter was before the place within it was added (a catalog entry is a follow-up) */ format!("{} of {}{}{within}", ix + 1, n, if more { "+" } else { "" })
}
/// The matches of a `thread.find` answer.
pub fn matches_of(result: &Value) -> (Vec<String>, usize, bool) {
    let m: Vec<String> = result["matches"].as_array().into_iter().flatten().filter_map(|h| h["msg"].as_str().map(str::to_string)).collect();
    let total = result["total"].as_u64().map_or(m.len(), |t| t as usize);
    (m, total, result["more"] == true)
}
/// What a row held shows that the words are looked for in: its kind's label, its text, its stamp and a card's own lines.
pub fn row_text(e: &Value) -> String {
    ["label", "text", "stamp", "title", "head", "detail", "cause", "note", "from"].iter().filter_map(|k| e[*k].as_str()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ")
}
/// The rows (id, text) that hold every word of `q`, case aside (the hub's rule), in their order. A row still being drawn (`@live:`, a
/// stream) has no lasting id and is left out.
pub fn local_matches<'a>(rows: impl IntoIterator<Item = (&'a str, String)>, q: &str) -> Vec<String> {
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() { return vec![] }
    rows.into_iter().filter(|(id, t)| !id.is_empty() && !id.starts_with('@') && { let t = t.to_lowercase(); words.iter().all(|w| t.contains(w.as_str())) }).map(|(id, _)| id.to_string()).collect()
}
/// The stops: the hub's matches that are not among the rows held (older ones, where the hub put them) first, then the rows held that the
/// hub or this window found, in the thread's order (a message found by both is one stop).
pub fn merge(hub: &[String], rows: &[&str], local: &[String]) -> Vec<String> {
    let held: std::collections::HashSet<&str> = rows.iter().copied().collect();
    let found: std::collections::HashSet<&str> = hub.iter().chain(local).map(String::as_str).collect();
    let mut out: Vec<String> = hub.iter().filter(|m| !held.contains(m.as_str())).cloned().collect();
    let mut seen = std::collections::HashSet::new();
    out.extend(rows.iter().filter(|r| found.contains(*r) && seen.insert(**r)).map(|r| r.to_string()));
    out
}
/// The stop to show after the stops changed: `want` (a message picked in Find across threads) when it is there, else the one on show
/// when it still is (the list moves under it, not the reader), else the newest.
pub fn pick(stops: &[String], current: &str, want: &str) -> usize {
    let at = |m: &str| (!m.is_empty()).then(|| stops.iter().position(|s| s == m)).flatten();
    at(want).or_else(|| at(current)).unwrap_or(stops.len().saturating_sub(1))
}
/// Where a message is in the rows held: its own row (a message is a row of its id).
pub fn row_of(rows: &[Value], msg: &str) -> Option<usize> { rows.iter().position(|e| e["id"].as_str() == Some(msg)) }
/// The words found in a message's drawn blocks (in reading order): the first block holding the whole query (case aside), else the first
/// holding its first word (the hub finds every word, anywhere in the message), and the byte range there.
pub fn match_in(blocks: &[&str], q: &str) -> Option<(usize, std::ops::Range<usize>)> { matches_in(blocks, q).into_iter().next() }
/// Every place of the words in a message's drawn blocks, in reading order (block, byte range): the whole query's places (case aside), else,
/// when it is nowhere, its first word's (the hub finds every word, anywhere in the message). Places never overlap.
pub fn matches_in(blocks: &[&str], q: &str) -> Vec<(usize, std::ops::Range<usize>)> {
    let q = q.trim();
    let all = |w: &str| blocks.iter().enumerate().flat_map(|(i, t)| find_all_ci(t, w).into_iter().map(move |at| (i, at..at + w.len()))).collect::<Vec<_>>();
    if q.is_empty() { return vec![] }
    let whole = all(q);
    if !whole.is_empty() { return whole }
    q.split_whitespace().next().filter(|w| *w != q).map_or(vec![], all)
}
/// Every place of `w` in `t`, ASCII case aside (byte offsets of `t`), one after another.
fn find_all_ci(t: &str, w: &str) -> Vec<usize> {
    let mut out = vec![];
    if w.is_empty() || w.len() > t.len() { return out }
    let mut i = 0;
    while i + w.len() <= t.len() {
        if t.is_char_boundary(i) && t.as_bytes()[i..i + w.len()].eq_ignore_ascii_case(w.as_bytes()) { out.push(i); i += w.len(); } else { i += 1; }
    }
    out
}
/// How far into its message the list starts so the found line stands a third of the way down the list (`y`: the line's top within the message).
pub fn scroll_into(y: Pixels, list_h: Pixels) -> Pixels { (y - list_h / 3.).max(px(0.)) }
/// Frames waited at most for the message's row to be drawn after the list went to it.
const SEEK_FRAMES: u8 = 20;

/// The bar's answer (false: not the bar's tag).
pub fn answered(this: &mut Backplane, tag: u64, reply: &Value, cx: &mut Context<Backplane>) -> bool {
    if tag < TAG { return false }
    let f = &mut this.screens.find;
    if tag != f.asked.0 || reply["ok"] != true { return true }
    // an answer for words since changed: the next ask brings theirs
    if query_of(&f.q).as_deref() != Some(f.asked.1.as_str()) { return true }
    let (m, total, more) = matches_of(&reply["result"]);
    f.hub = m; f.total = total; f.more = more; f.hub_q = f.asked.1.clone();
    // the rows held looked at again: the thread (or the page) asked for may have come since the words changed
    let q = f.hub_q.clone(); this.find_local(&q);
    this.find_merge(true);
    cx.notify(); true
}

impl Backplane {
    fn find_input(&mut self, cx: &mut Context<Self>) -> Entity<Composer> {
        if let Some(i) = self.screens.find.input.clone() { return i }
        let input = cx.new(|cx| { let mut c = Composer::with_context(cx, "FindInput"); c.max_lines = 1.0; c });
        /* i18n-ok: a control id */ input.update(cx, |i, _| i.set_placeholder("Find in this thread"));
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
        self.solidcraft_lend(cx);
        self.find_input(cx);
        if self.screens.find.thread != raw { self.find_reset(cx); self.screens.find.thread = raw.clone(); }
        if self.selected.rsplit('|').next() != Some(raw.as_str()) { let id = if thread.contains('|') { thread.to_string() } else { super::tools::qualify(&self.data, &raw) }; let _ = self.tx.try_send(Command::Action("select".into(), id)); }
        self.screens.find.open = true; self.screens.find.focus = true;
        // words kept from the last time: all selected, as every find box reopens, so typing replaces them and Enter repeats the search
        if let Some(i) = self.screens.find.input.clone() { i.update(cx, |i, cx| if !i.editor().is_empty() { i.select_everything(cx) }); }
        if !self.screens.find.mark.is_empty() || self.screens.find.hits_q.is_empty() { self.find_ask(cx); }
        cx.notify();
    }
    /// A message found elsewhere (Find across threads): its thread, the bar holding the words, and that message shown first and marked.
    pub(crate) fn find_open_at(&mut self, thread: &str, msg: &str, q: &str, cx: &mut Context<Self>) {
        let raw = thread.rsplit('|').next().unwrap_or("").to_string();
        self.solidcraft_lend(cx);
        let input = self.find_input(cx);
        self.find_reset(cx);
        self.screens.find.thread = raw.clone(); self.screens.find.open = true; self.screens.find.focus = true; self.screens.find.want = msg.to_string(); self.screens.find.q = q.to_string();
        input.update(cx, |i, cx| { i.set(q.to_string(), cx); i.select_everything(cx) });
        self.find_go(msg.to_string());
        if self.selected.rsplit('|').next() != Some(raw.as_str()) { let _ = self.tx.try_send(Command::Action("select".into(), thread.to_string())); }
        self.find_ask(cx);
        cx.notify();
    }
    /// Shut the bar: the mark goes, the keys go back to the message box.
    pub(crate) fn find_close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.screens.find.open && self.screens.find.mark.is_empty() { return false }
        self.screens.find.open = false; self.screens.find.mark.clear(); self.screens.find.going = None; self.screens.find.seek = None; self.want_focus = true; cx.notify(); true
    }
    fn find_reset(&mut self, cx: &mut Context<Self>) {
        let f = &mut self.screens.find;
        f.matches.clear(); f.total = 0; f.more = false; f.ix = 0; f.at = At::First; f.occ = 0; f.occs = 0; f.hits_q.clear(); f.hub.clear(); f.hub_q.clear(); f.local.clear(); f.asked = (0, String::new()); f.mark.clear(); f.going = None; f.seek = None; f.want.clear(); f.q.clear();
        if let Some(i) = f.input.clone() { i.update(cx, |i, cx| i.set(String::new(), cx)); }
    }
    /// The words changed: after a pause the hub is asked for the thread's matches (the last answer for the words on show is the one kept).
    fn find_ask(&mut self, cx: &mut Context<Self>) {
        let Some(q) = query_of(&self.screens.find.q) else { let f = &mut self.screens.find; f.matches.clear(); f.total = 0; f.hits_q.clear(); f.hub.clear(); f.hub_q.clear(); f.local.clear(); f.mark.clear(); f.going = None; return };
        let thread = self.screens.find.thread.clone();
        // the rows held here at once (once per change of the words), the hub's answer merged in when it comes
        if self.screens.find.hits_q != q {
            let f = &mut self.screens.find;
            if f.hub_q != q { f.hub.clear(); f.hub_q.clear(); f.total = 0; f.more = false; }
            self.find_local(&q);
            self.find_merge(false);
        }
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
    /// The rows held here that hold the words (none while the bar's thread is not the one held).
    fn find_local(&mut self, q: &str) {
        let raw = crate::string(&self.data["thread"], "id");
        let held = raw.rsplit('|').next() == Some(self.screens.find.thread.as_str());
        let rows = self.data["thread"]["entries"].as_array().filter(|_| held);
        self.screens.find.local = local_matches(rows.into_iter().flatten().map(|e| (e["id"].as_str().unwrap_or(""), row_text(e))), q);
        self.screens.find.hits_q = q.to_string();
    }
    /// The stops again from the hub's matches and the rows held here; the one on show stays when it still is, else the list goes to the new one.
    /// `answer`: the hub's answer came (the message asked for from Find across threads is then shown or given up).
    fn find_merge(&mut self, answer: bool) {
        let ids: Vec<String> = self.data["thread"]["entries"].as_array().into_iter().flatten().map(|e| crate::string(e, "id")).collect();
        let rows: Vec<&str> = ids.iter().map(String::as_str).collect();
        let f = &mut self.screens.find;
        let stops = merge(&f.hub, &rows, &f.local);
        let current = if f.mark.is_empty() { String::new() } else { f.matches.get(f.ix).cloned().unwrap_or_default() };
        let ix = pick(&stops, &current, &f.want);
        // a message picked in Find across threads is on its way (its thread or its page may not be held yet): the hub's answer settles it
        if !answer && !f.want.is_empty() && stops.get(ix) != Some(&f.want) { f.matches = stops; f.ix = 0; return }
        if answer { f.want.clear(); }
        f.matches = stops; f.ix = ix;
        match f.matches.get(ix).cloned() {
            // the hub's answer under the stop on show: it stays as it is
            Some(msg) if msg == current && !current.is_empty() && answer => {}
            // new words, the same stop: their first place there is marked again (the list stays when the row is drawn)
            Some(msg) if msg == current && !current.is_empty() => {
                f.at = At::First; f.occ = 0; f.occs = 0;
                let drawn = self.data["thread"]["entries"].as_array().and_then(|r| row_of(r, &msg)).and_then(|i| self.timeline.bounds_for_item(i)).is_some();
                if drawn && self.screens.find.going.is_none() { self.screens.find.seek = Some((msg, 1)); } else { self.find_go(msg) }
            }
            Some(msg) => { f.at = At::First; f.occ = 0; f.occs = 0; self.find_go(msg) }
            None => { f.mark.clear(); f.going = None; }
        }
    }
    /// Enter / the down arrow: the next place of the words (the next one in the same message first, then the next message's first);
    /// Shift+Enter / the up arrow: the previous (the previous message's last when leaving a message backwards).
    pub(crate) fn find_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let f = &mut self.screens.find;
        if f.matches.is_empty() { return }
        let (ix, at) = step_at(f.ix, f.occ, f.occs, f.matches.len(), forward);
        let msg = f.matches[ix].clone();
        let same = ix == f.ix && matches!(at, At::Nth(_)) && f.going.is_none();
        f.ix = ix; f.at = at;
        // a place in the message on show whose row is drawn: its words are found again on the next frame (no trip to the message's top)
        let drawn = same && self.data["thread"]["entries"].as_array().and_then(|r| row_of(r, &msg)).and_then(|i| self.timeline.bounds_for_item(i)).is_some();
        if drawn { self.screens.find.seek = Some((msg, 1)); } else { if !same { self.screens.find.occs = 0; self.screens.find.occ = 0; } self.find_go(msg); }
        cx.notify();
    }
    /// After the list went to a found message (at its top): once its row is drawn, the words themselves, a third of the way down the list and
    /// selected (the selection's colour marks them; a long message's match is often far below its top). Waits a few frames at most, then stops.
    pub(crate) fn find_seek(&mut self, window: &mut Window) {
        let Some((msg, n)) = self.screens.find.seek.clone() else { return };
        let Some(ix) = self.data["thread"]["entries"].as_array().and_then(|r| row_of(r, &msg)) else { self.screens.find.seek = None; return };
        // the first frame after the list moved lays the row out; its blocks are known from the next
        let found = if n == 0 { None } else {
            let reg = self.screens.reg.borrow();
            let blocks: Vec<(usize, &super::timeline::Info)> = reg.blocks.iter().enumerate().filter(|(_, b)| b.id == msg).collect();
            let texts: Vec<&str> = blocks.iter().map(|(_, b)| b.text.as_str()).collect();
            let all = matches_in(&texts, &self.screens.find.q);
            let k = place_of(self.screens.find.at, all.len());
            all.get(k).cloned().and_then(|(i, r)| {
                let (ord, b) = blocks[i];
                let y = b.layout.as_ref()?.position_for_index(r.start)?.y;
                let item = self.timeline.bounds_for_item(ix)?;
                Some((ord, r, y - item.top(), k, all.len()))
            })
        };
        match found {
            Some((ord, r, y, k, n)) => {
                self.screens.find.occ = k; self.screens.find.occs = n;
                self.timeline.scroll_to(ListOffset { item_ix: ix, offset_in_item: scroll_into(y, self.timeline.viewport_bounds().size.height) });
                self.screens.reg.borrow_mut().select(super::select::Selection { anchor: super::select::Pos { block: ord, off: r.start }, head: super::select::Pos { block: ord, off: r.end } });
                self.screens.find.seek = None; window.refresh();
            }
            None if n < SEEK_FRAMES => { self.screens.find.seek = Some((msg, n + 1)); window.request_animation_frame(); }
            None => { self.screens.find.seek = None; self.screens.find.occ = 0; self.screens.find.occs = 0; }
        }
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
            self.screens.find.going = None; self.screens.find.seek = Some((msg, 0)); return;
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
    // the rows held here answer at once when they hold the words; "No matches" waits for the hub's word too
    let q = query_of(&f.q);
    let answered = q.as_deref() == Some(f.hits_q.as_str()) && (!f.matches.is_empty() || q.as_deref() == Some(f.hub_q.as_str()));
    let label = count(f.ix, f.matches.len(), f.more, answered, f.occ, f.occs);
    // previous and next as arrows in the text (the icon set has no up chevron and is generated, assets/GENERATED.md)
    let btn = |id: &'static str, glyph: &'static str, tip: &'static str| crate::ui::with_rect(div().id(id), move || format!("ui:{tip}")).cursor_pointer().w(px(22.)).h(px(22.)).flex().items_center().justify_center().text_color(c("dim")).hover(|s| s.bg(c("raise")).text_color(c("text"))).child(glyph);
    // a strip over the thread's rows (never over them: the message it marks would hide under it), the bar at its right
    Some(div().w_full().flex_shrink_0().flex().justify_end().px_4().py_1().border_b_1().border_color(c("line")).child(div().id("find-bar").w(px(400.)).max_w_full().flex().items_center().gap_2().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(13.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(crate::art::icon("search", 13., c("dim")))
        .child(div().flex_1().min_w_0().children(f.input.clone()))
        .child(div().flex_shrink_0().text_size(crate::scale::tpx(11.)).text_color(c(if answered && f.matches.is_empty() { "bad" } else { "faint" })).child(label))
        /* i18n-ok: a control id */ .child(btn("find-prev", "↑", "Previous match").on_click(cx.listener(|this, _, _, cx| this.find_step(false, cx))))
        /* i18n-ok: a control id */ .child(btn("find-next", "↓", "Next match").on_click(cx.listener(|this, _, _, cx| this.find_step(true, cx))))
        /* i18n-ok: a control id */ .child(crate::ui::with_rect(div().id("find-close"), || "ui:Close find".to_string()).cursor_pointer().p_1().hover(|s| s.bg(c("raise"))).child(crate::art::icon("x", 14., c("dim"))).on_click(cx.listener(|this, _, _, cx| { this.find_close(cx); }))))
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
    fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
    #[test] fn an_answer_shows_the_message_asked_for_else_the_one_on_show_else_the_newest() {
        let m = ids(&["a", "b", "c"]);
        assert_eq!((pick(&m, "", "b"), pick(&m, "", ""), pick(&m, "", "gone"), pick(&[], "", "")), (1, 2, 2, 0));
        assert_eq!(pick(&m, "a", ""), 0, "the stop on show stays when it still is");
        assert_eq!(pick(&m, "gone", ""), 2, "else the newest");
        assert_eq!(pick(&m, "a", "b"), 1, "the message asked for first");
    }
    #[test] fn the_rows_held_are_found_whatever_their_kind() {
        let rows = vec![
            json!({"id": "w", "kind": "act", "label": "git", "text": "code worktree /work/amp"}),
            json!({"id": "u1", "kind": "user", "text": "mode:stream approve: go", "stamp": "05:31"}),
            json!({"id": "fold:t1", "kind": "fold", "text": "4 tool calls \u{b7} Bash", "value": "0t1"}),
            json!({"id": "a1", "kind": "act", "label": "Bash", "text": "grep -n go board.kicad_pcb # 1"}),
            json!({"id": "a2", "kind": "act", "label": "Bash", "text": "GREP -n go board.kicad_pcb # 2\nmore lines"}),
            json!({"id": "a3", "kind": "act", "label": "Bash", "text": "rm -rf build/"}),
            json!({"id": "r1", "kind": "assistant", "text": "done", "stamp": "Finished 05:33 \u{b7} 1m 19s"}),
            json!({"id": "h1", "kind": "handoff", "title": "Moved to the bench", "note": "grep it later"}),
            json!({"id": "@live:x", "kind": "assistant", "text": "grep streaming"}),
        ];
        let find = |q: &str| local_matches(rows.iter().map(|e| (e["id"].as_str().unwrap(), row_text(e))), q);
        assert_eq!(find("grep"), ids(&["a1", "a2", "h1"]), "tool rows (case aside) and a card, never a row still streaming");
        assert_eq!(find("bash board"), ids(&["a1", "a2"]), "every word, anywhere in the row: its label too");
        assert_eq!(find("tool calls"), ids(&["fold:t1"]), "a fold's head");
        assert_eq!(find("worktree"), ids(&["w"])); assert_eq!(find("finished"), ids(&["r1"]), "a stamp");
        assert_eq!(find("approve"), ids(&["u1"])); assert!(find("xyzzy").is_empty()); assert!(find("  ").is_empty());
        assert!(local_matches(std::iter::empty(), "grep").is_empty());
    }
    #[test] fn the_hubs_matches_and_the_rows_held_are_one_list_in_thread_order() {
        let rows = ["u1", "a1", "a2", "u2", "a3"];
        assert_eq!(merge(&ids(&["old1", "u1", "old2", "u2"]), &rows, &ids(&["a1", "u2", "a3"])), ids(&["old1", "old2", "u1", "a1", "u2", "a3"]),
            "the hub's older messages first, where it put them; then the rows held in order; a message found by both once");
        assert_eq!(merge(&[], &rows, &ids(&["a3", "a1"])), ids(&["a1", "a3"]), "the rows held alone, in the thread's order");
        assert_eq!(merge(&ids(&["old1"]), &rows, &[]), ids(&["old1"]), "the hub's alone");
        assert!(merge(&[], &rows, &[]).is_empty()); assert!(merge(&[], &[], &[]).is_empty());
        assert_eq!(merge(&ids(&["u1"]), &["u1", "u1"], &ids(&["u1"])), ids(&["u1"]), "one stop per row even when ids repeat");
    }
    #[test] fn the_stop_on_show_stays_when_the_hubs_answer_comes() {
        let rows = ["u1", "a1", "a2"];
        let before = merge(&[], &rows, &ids(&["a1", "a2"]));
        let at = pick(&before, "", ""); assert_eq!(before[at], "a2", "the newest held row first");
        let after = merge(&ids(&["old", "u1"]), &rows, &ids(&["a1", "a2"]));
        assert_eq!(after[pick(&after, &before[0], "")], "a1", "the reader on a1 stays on a1 though the list grew in front of it");
        assert_eq!(pick(&after, "gone", ""), after.len() - 1, "a stop no longer there: the newest");
    }
    #[test] fn the_count_says_where_the_reader_is() {
        assert_eq!(count(2, 12, false, true, 0, 1), "3 of 12"); assert_eq!(count(0, 1000, true, true, 0, 0), "1 of 1000+");
        assert_eq!(count(2, 12, false, true, 2, 60), "3 of 12 \u{b7} 3/60", "the place within a message that holds more than one");
        assert_eq!(count(0, 0, false, true, 0, 0), "No matches"); assert_eq!(count(0, 0, false, false, 0, 0), "", "nothing before the hub answers");
    }
    #[test] fn the_words_are_found_in_the_message_and_put_a_third_down_the_list() {
        let b = ["Long report", "Paragraph 46: the anode on V46 reads 246 V", "Paragraph 47: the anode on V47 reads 247 V", "V47 again"];
        assert_eq!(match_in(&b, "V47"), Some((2, 27..30)), "the first block that holds it, and where");
        assert_eq!(match_in(&b, "v47"), Some((2, 27..30)), "case aside");
        assert_eq!(match_in(&b, "anode V47"), Some((1, 18..23)), "words apart: the first word's first place");
        assert_eq!(match_in(&b, "V99"), None); assert_eq!(match_in(&b, " "), None); assert_eq!(match_in(&[], "V47"), None);
        assert_eq!(match_in(&["Gr\u{fc}\u{df}e V47"], "v47"), Some((0, 8..11)), "byte offsets past non-ASCII letters");
        assert_eq!(scroll_into(px(1200.), px(600.)), px(1000.)); assert_eq!(scroll_into(px(100.), px(600.)), px(0.), "near the top: the message's top");
    }
    #[test] fn every_place_in_a_message_is_found_in_reading_order() {
        let b = ["Anode one", "the anode, the ANODE", "none here", "anodeanode"];
        assert_eq!(matches_in(&b, "anode"), vec![(0, 0..5), (1, 4..9), (1, 15..20), (3, 0..5), (3, 5..10)]);
        assert_eq!(matches_in(&b, "anode zz"), vec![(0, 0..5), (1, 4..9), (1, 15..20), (3, 0..5), (3, 5..10)], "words apart: the first word's places");
        assert_eq!(matches_in(&["aaaa"], "aa"), vec![(0, 0..2), (0, 2..4)], "places never overlap");
        assert!(matches_in(&b, "grid").is_empty()); assert!(matches_in(&[], "anode").is_empty());
    }
    #[test] fn enter_steps_through_every_place_then_the_next_message() {
        // 3 messages; the one on show (1) holds 3 places
        assert_eq!(step_at(1, 0, 3, 3, true), (1, At::Nth(1)), "the next place in the same message first");
        assert_eq!(step_at(1, 2, 3, 3, true), (2, At::First), "past its last: the next message's first");
        assert_eq!(step_at(2, 0, 1, 3, true), (0, At::First), "past the last message: round to the first");
        assert_eq!(step_at(1, 2, 3, 3, false), (1, At::Nth(1)), "back: the previous place in the same message");
        assert_eq!(step_at(1, 0, 3, 3, false), (0, At::Last), "back from its first: the previous message's last");
        assert_eq!(step_at(0, 0, 2, 3, false), (2, At::Last), "back from the first message: round to the last message's last");
        assert_eq!(step_at(1, 0, 0, 3, true), (2, At::First), "a message whose drawn text holds none: on to the next");
        assert_eq!(step_at(1, 0, 0, 3, false), (0, At::Last), "and back to the previous");
        assert_eq!(step_at(0, 4, 5, 1, true), (0, At::First), "one message: round to its own first place");
        assert_eq!(step_at(0, 0, 5, 1, false), (0, At::Last), "and back to its own last");
        assert_eq!(step_at(0, 0, 0, 0, true), (0, At::First), "no matches: nowhere to go");
        assert_eq!((place_of(At::First, 4), place_of(At::Last, 4), place_of(At::Nth(2), 4), place_of(At::Nth(9), 4), place_of(At::Last, 0)), (0, 3, 2, 3, 0));
    }
    #[test] fn the_hubs_answer_is_its_matches_in_order() {
        let r = json!({"q": "pine", "matches": [{"msg": "m1"}, {"msg": "m4"}], "total": 2, "more": false});
        assert_eq!(matches_of(&r), (vec!["m1".to_string(), "m4".to_string()], 2, false));
        assert_eq!(query_of(" p "), None); assert_eq!(query_of(" pi "), Some("pi".into()));
        let rows = vec![json!({"id": "a1"}), json!({"id": "m4"})]; assert_eq!(row_of(&rows, "m4"), Some(1)); assert_eq!(row_of(&rows, "m9"), None);
    }
}

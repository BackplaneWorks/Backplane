//! The few places where the window hands control to the screens: all of Backplane's screen-specific
//! methods live here so main.rs keeps one call at each seam.
use super::{asks, attach::{self, DEFAULT_CAP}, composer::ComposerEvent, history, place, timeline};
use crate::{hub::Command, Backplane};
use gpui::*;
use serde_json::Value;

/// The popup's highlighted row after Up (-1) or Down (1): it goes round.
pub fn popup_step(ix: usize, d: i32, n: usize) -> usize { (ix as i32 + d).rem_euclid(n.max(1) as i32) as usize }

impl Backplane {
    /// A composer event (paste, drop, picked files): start the uploads off the UI thread; say why when nothing starts.
    pub(crate) fn attach_event(&mut self, event: &ComposerEvent, cx: &mut Context<Self>) {
        if matches!(event, ComposerEvent::CopyTimeline) { return self.timeline_copy(cx); }
        if let ComposerEvent::PopupMove(d) = event { let n = self.popup_rows().len(); if n > 0 { self.screens.popup_ix = popup_step(self.screens.popup_ix, *d, n); self.screens.popup_scroll.scroll_to_item(self.screens.popup_ix); self.m("popup", serde_json::json!({"ix": self.screens.popup_ix, "rows": n})); cx.notify(); } return; }
        if matches!(event, ComposerEvent::PopupPick) { self.popup_pick(cx); return; }
        let cap = self.data["thread"]["cap"].as_u64().unwrap_or(DEFAULT_CAP);
        if let Err(e) = self.screens.attacher.event(&self.tx, &self.selected, cap, event) { self.error = e; }
        cx.notify();
    }
    pub(crate) fn pick_files(&mut self, cx: &mut Context<Self>) {
        // test hook: BACKPLANE_GPUI_PICK_FILES=<path>:<path> stands in for the system file dialog (a private display has no portal)
        if let Some(v) = std::env::var_os("BACKPLANE_GPUI_PICK_FILES") { let paths: Vec<std::path::PathBuf> = std::env::split_paths(&v).filter(|p| !p.as_os_str().is_empty()).collect(); self.attach_event(&ComposerEvent::Files(paths), cx); return; }
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
        cx.spawn(async move |weak, cx| {
            if let Ok(Ok(Some(paths))) = rx.await { let _ = weak.update(cx, |this, cx| this.attach_event(&ComposerEvent::Files(paths), cx)); }
        }).detach();
    }
    /// The composer's text changed: keep it as this thread's draft (written in the background) and tell the hub,
    /// which decides the popup rows (`thread.skills`) from it. True when the text really changed.
    pub(crate) fn draft_changed(&mut self, text: &str) -> bool {
        if self.drafts.get(&self.selected).map_or(text.is_empty(), |d| d == text) { return false; }
        let old = self.drafts.get(&self.selected).cloned().unwrap_or_default(); self.voice_typed(&old, text);
        if self.selected.is_empty() { return true; }
        if text.is_empty() { self.drafts.remove(&self.selected); } else { self.drafts.insert(self.selected.clone(), text.to_string()); }
        self.screens.drafts.set(&self.selected, text);
        true
    }
    /// A popup row was pressed: remember the draft at that moment and wait for the hub's completion of it.
    pub(crate) fn await_completion(&mut self, cx: &App) { self.screens.await_draft = Some((self.input.read(cx).content.to_string(), std::time::Instant::now())); }
    /// The hub's picture carries the completed draft after a row press. Wait for a draft that differs from the one at the
    /// press (a picture already in flight carries the old one), give up after 2 s, and never overwrite what was typed since.
    pub(crate) fn draft_from_hub(&mut self, cx: &mut Context<Self>) {
        let Some((at, since)) = self.screens.await_draft.clone() else { return };
        if since.elapsed() > std::time::Duration::from_secs(2) || self.input.read(cx).content != at { self.screens.await_draft = None; return; }
        let d = self.data["thread"]["draft"].as_str().unwrap_or("").to_string();
        if !d.is_empty() && d != at { self.screens.await_draft = None; self.input.update(cx, |i, cx| i.set(d, cx)); }
    }
    /// Send the draft to the hub: the first change of a burst goes at once (leading edge, so a popup is not held back), the rest of the burst
    /// is coalesced for 30 ms and only the newest text is kept, so the last text is never dropped.
    pub(crate) fn send_draft(&mut self, text: String, cx: &mut Context<Self>) {
        if self.screens.draft_timer { self.screens.pending_draft = Some(text); return; }
        self.screens.draft_timer = true;
        let _ = self.tx.try_send(crate::hub::Command::Draft(text));
        cx.spawn(async move |weak, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(30)).await;
            let _ = weak.update(cx, |this, _| {
                this.screens.draft_timer = false;
                if let Some(t) = this.screens.pending_draft.take() { let _ = this.tx.try_send(crate::hub::Command::Draft(t)); }
            });
        }).detach();
    }
    /// A picture arrived: the first selection restores its saved draft; open asks get their boxes.
    pub(crate) fn picture_arrived(&mut self, selected: &str, cx: &mut Context<Self>) {
        // every picture names the thread on screen, so an upload that ends later is reported only if it is still the one shown (a bot's chat is opened by its row, which never goes through thread_switch)
        if !selected.is_empty() { self.screens.attacher.show(selected); }
        if !selected.is_empty() && self.selected.is_empty() {
            if let Some(d) = self.drafts.get(selected).cloned() { if self.input.read(cx).content.is_empty() { self.input.update(cx, |i, cx| i.set(d, cx)); } }
        }
        self.place_restore(selected);
        asks::sync(self, cx);
    }
    /// Leaving `self.selected` for `raw`: flush its draft and remember its place. (The picture itself is the window's `thread_cache`.)
    pub(crate) fn thread_switch(&mut self, raw: &str) {
        self.screens.drafts.kick();
        self.screens.paging = None; // a page on its way was the last thread's: this one asks for its own
        if !self.selected.is_empty() && self.screens.seeking.is_none() {
            match self.timeline_place() { Some(p) => { self.screens.offsets.insert(self.selected.clone(), p); } None => { self.screens.offsets.remove(&self.selected); } }
        }
        self.screens.seeking = None;
        self.screens.attacher.show(raw);
        self.screens.images.retry();
    }
    /// Where the reader is in the thread on screen; none at its end.
    pub(crate) fn timeline_place(&self) -> Option<place::RowPlace> {
        let top = self.timeline.logical_scroll_top();
        place::kept_place(self.data["thread"]["entries"].as_array().map_or(&[], |r| r.as_slice()), top.item_ix, f32::from(top.offset_in_item))
    }
    /// The rows on screen are another thread's: the list starts over, at the end or at the place this thread was left.
    pub(crate) fn timeline_show(&mut self, raw: &str) {
        self.timeline.reset(self.data["thread"]["entries"].as_array().map_or(0, |r| r.len()));
        self.screens.born.clear(); self.screens.shown_at = std::time::Instant::now();
        self.screens.seeking = self.screens.offsets.get(raw).cloned();
        self.timeline_seek();
    }
    /// A place being looked for: taken as soon as its row is held, unless the reader has moved off the end in the meantime.
    fn timeline_seek(&mut self) {
        let Some((id, off)) = self.screens.seeking.clone() else { return };
        if self.timeline.logical_scroll_top().item_ix < self.timeline.item_count() { self.screens.seeking = None; return; }
        if let Some(ix) = self.data["thread"]["entries"].as_array().and_then(|r| r.iter().position(|e| e["id"].as_str() == Some(&id))) {
            self.timeline.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(off) });
            self.screens.seeking = None;
        }
    }
    /// The words of the popup rows the hub offers now (none while it offers none).
    pub(crate) fn popup_rows(&self) -> Vec<String> {
        if self.screens.popup_shut { return vec![]; }
        self.data["thread"]["skills"].as_array().into_iter().flatten().map(|s| s.get("word").or_else(|| s.get("name")).and_then(Value::as_str).unwrap_or("").to_string()).filter(|w| !w.is_empty()).collect()
    }
    /// Choose the highlighted row: the hub puts its word where the one being typed is.
    pub(crate) fn popup_pick(&mut self, cx: &mut Context<Self>) -> bool {
        let rows = self.popup_rows();
        let Some(word) = rows.get(self.screens.popup_ix.min(rows.len().saturating_sub(1))).cloned() else { return false };
        let _ = self.tx.try_send(Command::Action("skill".into(), word)); self.await_completion(cx); cx.notify(); true
    }
    /// Go to a message by its entry id (a reply's link); nothing when the rows held do not have it.
    pub(crate) fn timeline_goto(&mut self, id: &str) {
        if let Some(ix) = self.data["thread"]["entries"].as_array().and_then(|r| r.iter().position(|e| e["id"].as_str() == Some(id))) {
            self.timeline.scroll_to(ListOffset { item_ix: ix, offset_in_item: px(0.) });
        }
    }
    /// A line in the client's log for a wheel turn over the thread (at most one in 400 ms): how far, where the list is, how long it is, and whether a history read or a place
    /// being looked for is going on. For finding why the list stops following the wheel.
    pub(crate) fn scroll_note(&mut self, e: &ScrollWheelEvent) {
        let now = std::time::Instant::now();
        if self.screens.scroll_logged.map_or(false, |t| now.duration_since(t).as_millis() < 400) { return; }
        self.screens.scroll_logged = Some(now);
        let top = self.timeline.logical_scroll_top();
        let dy = f32::from(e.delta.pixel_delta(px(20.)).y);
        self.m("wheel", serde_json::json!({"dy": dy, "item": top.item_ix, "in_item": f32::from(top.offset_in_item), "count": self.timeline.item_count(),
            "y": -f32::from(self.timeline.scroll_px_offset_for_scrollbar().y), "max": f32::from(self.timeline.max_offset_for_scrollbar().height),
            "seeking": self.screens.seeking.is_some(), "paging": self.screens.paging.is_some(), "older": !crate::string(&self.data["thread"], "historyOlder").is_empty()}));
    }
    /// Whether the reader is far from the end of the thread (more than about a screen of rows below): the "Latest" chip shows then. Kept here,
    /// one window per process, so the timeline's part can show it without a field of its own.
    pub(crate) fn far_from_end(&self) -> bool { LATEST_FAR.load(std::sync::atomic::Ordering::Relaxed) }
    /// The newest reading of the list: true when what is below the screen is more than FAR px. Answers whether it changed.
    pub(crate) fn note_far(&self) -> bool {
        let (max, y) = (f32::from(self.timeline.max_offset_for_scrollbar().height), -f32::from(self.timeline.scroll_px_offset_for_scrollbar().y));
        let far = far_from(max, y);
        LATEST_FAR.swap(far, std::sync::atomic::Ordering::Relaxed) != far
    }
    /// The end of the thread, and following it again. (Past the last row: the list takes it for its end.)
    pub(crate) fn timeline_to_end(&self) { self.timeline.scroll_to(ListOffset { item_ix: self.timeline.item_count(), offset_in_item: px(0.) }); LATEST_FAR.store(false, std::sync::atomic::Ordering::Relaxed); }
    /// A picture was applied. `prev` is the thread that was on screen: this picture of the same thread is laid over the rows it held, so what was
    /// read of its history stays, and the list keeps the reader where they are (or at the end, if that is where they are).
    pub(crate) fn timeline_picture(&mut self, mut prev: Value, window: &Value) {
        let id = crate::string(&self.data["thread"], "id"); let raw = attach::raw_id(&id).to_string();
        if raw.is_empty() { self.timeline.reset(0); return; }
        let older = history::next_older("", window["older"].as_str().unwrap_or(""), window["hasMore"] == true, 0);
        let fresh = match self.data["thread"]["entries"].take() { Value::Array(a) => a, _ => vec![] };
        let prev_id = crate::string(&prev, "id");
        let held = if attach::raw_id(&prev_id) == raw && prev["loading"] != true { match prev["entries"].take() { Value::Array(a) => a, _ => vec![] } } else { vec![] };
        let at = history::merge_point(&held, &fresh);
        let (rows, top) = match at {
            Some(i) if !held.is_empty() => {
                let before = held[i..].to_vec();
                let rows = history::merge(held, fresh);
                let (a, b) = history::changed(&before, &rows[i..], 0);
                splice_keeping(&self.timeline, a.start + i..a.end + i, b.len());
                let top = if i > 0 { crate::string(&prev, "historyOlder") } else { older };
                (rows, top)
            }
            _ => { self.data["thread"]["entries"] = Value::Array(fresh); self.data["thread"]["historyOlder"] = Value::String(older); self.timeline_show(&raw); return; }
        };
        self.data["thread"]["entries"] = Value::Array(rows); self.data["thread"]["historyOlder"] = Value::String(top);
        self.timeline_seek();
    }
    /// A history page landed: laid into the rows held, the reader staying where they are.
    pub(crate) fn older_landed(&mut self, cursor: String, page: Option<Value>, cx: &mut Context<Self>) {
        // how long the page took, asked to landed (the wheel's notes say whether the reader waited for it at the top)
        if let Some(t) = self.screens.paging.take() { self.m("history", serde_json::json!({"ms": t.elapsed().as_millis() as u64, "rows": page.as_ref().and_then(|r| r["shown"]["thread"]["entries"].as_array()).map_or(0, |a| a.len())})); }
        let Some(mut r) = page else { cx.notify(); return };
        let (a, b) = (crate::string(&r["shown"]["thread"], "id"), crate::string(&self.data["thread"], "id"));
        if attach::raw_id(&a) != attach::raw_id(&b) { return; }
        let mut rows = match r["shown"]["thread"]["entries"].take() { Value::Array(a) => a, _ => vec![] };
        for row in &mut rows { row[history::PAGE_KEY] = Value::String(cursor.clone()); }
        let old = match self.data["thread"]["entries"].take() { Value::Array(a) => a, _ => vec![] };
        let (merged, x, y) = history::splice_changed(old, rows);
        splice_keeping(&self.timeline, x, y.len());
        if self.data["thread"]["historyOlder"] == cursor.as_str() {
            let next = history::next_older(&cursor, r["window"]["older"].as_str().unwrap_or(""), r["window"]["hasMore"] == true, merged.len());
            self.data["thread"]["historyOlder"] = Value::String(next);
        }
        self.data["thread"]["entries"] = Value::Array(merged);
        self.timeline_seek();
        // a page of a long thread is mostly tool calls folded into a few rows: while the reader is still within RUNWAY_ROWS of the top, ask for the next
        // page now rather than after the next frame, so scrolling up a thread of a thousand messages does not wait at the top for each page in turn
        if self.timeline.logical_scroll_top().item_ix < history::RUNWAY_ROWS { self.history_want_now(); }
        cx.notify();
    }
    /// The next older page, now (none while one is on its way, at the start of the thread, or on a project's page).
    fn history_want_now(&mut self) {
        let older = crate::string(&self.data["thread"], "historyOlder");
        if older.is_empty() || self.data["projectHome"].is_object() || self.screens.paging.is_some() { return; }
        if self.tx.try_send(Command::History(older)).is_ok() { self.screens.paging = Some(std::time::Instant::now()); }
    }
    /// After each frame: the reader is within reach of the top of the rows held and more are there, so ask for them before they get there.
    pub(crate) fn history_want(&mut self) {
        let older = crate::string(&self.data["thread"], "historyOlder");
        if older.is_empty() { if self.data["thread"]["loading"] != true { self.screens.seeking = None; } return; } // the start of the thread: the place is not there
        if self.data["projectHome"].is_object() || self.screens.paging.is_some_and(|t| t.elapsed().as_secs() < 10) { return; }
        let (max, y) = (f32::from(self.timeline.max_offset_for_scrollbar().height), -f32::from(self.timeline.scroll_px_offset_for_scrollbar().y));
        if (self.screens.seeking.is_some() || timeline::near_top(y, max)) && self.tx.try_send(Command::History(older)).is_ok() { self.screens.paging = Some(std::time::Instant::now()); }
    }
    /// A run of tool calls opened or shut. A row read for history is outside the hub's newest page, so its page is read again to show it.
    pub(crate) fn fold_click(&self, value: String) {
        let page = self.data["thread"]["entries"].as_array().into_iter().flatten().find(|e| e["value"].as_str() == Some(&value)).map(|e| crate::string(e, history::PAGE_KEY)).unwrap_or_default();
        let _ = self.tx.try_send(Command::Action("fold".into(), value));
        if !page.is_empty() { let _ = self.tx.try_send(Command::History(page)); }
    }
    /// A picture was applied: the drafts, the asks' boxes and the settings fields follow what the hub just drew.
    pub(crate) fn picture_applied(&mut self, cx: &mut Context<Self>) {
        self.draft_from_hub(cx);
        // the pressed rail kind gives way when the hub marks it (or after 2 s): a streamed picture arriving first does not flicker it back
        if let Some(p) = self.screens.rail_pending.clone() {
            let hub_on = super::viewer::active_kind(&self.data["thread"]["viewer"], None);
            if hub_on == p || self.screens.rail_at.map_or(true, |t| t.elapsed() > std::time::Duration::from_secs(2)) { self.screens.rail_pending = None; self.screens.rail_at = None; }
        }
        super::settings::sync(self, cx);
        super::onboard::sync(self, cx);
        super::projects::sync(self, cx);
        super::plotview::report_state(self);
        super::histbar::follow_focus(self);
        super::filesui::sync(self, cx);
        self.browser_page(cx);
        // a form or a room page opened: its first field takes the keys; closed: back to the composer
        let open = super::bots::focus_target(&self.data);
        if open != self.screens.bots_open { self.screens.bots_open = open.clone(); if open.is_empty() { self.want_focus = true; } else { self.screens.focus_field = Some(open); } }
        // settings opened: the keys go into it; closed: back to the composer
        let open = super::settings::is_open(self);
        if open != self.screens.settings_was_open { self.screens.settings_was_open = open; if open { self.screens.focus_settings = true; } else { self.want_focus = true; } }
    }
    /// A plot-lane frame arrived on the socket: a solid (3D) frame goes to the 3D page, anything else to the plot store.
    pub(crate) fn plot_received(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if self.screens.solid.is_none() {
            let page = cx.new(crate::viewers::solid::SolidPage::new);
            // the page's Retry: the same source asked for again (a failed source is read again when it is watched)
            cx.subscribe(&page, |this, _, ev: &crate::viewers::solid::SolidEvent, _| { let crate::viewers::solid::SolidEvent::Retry(key) = ev;
                if let Some(p) = crate::viewers::solid::retry_watch(key) { let _ = this.tx.try_send(Command::Rpc("kicad.watch".into(), super::settings::Secret(p), "watch")); } }).detach();
            self.screens.solid = Some(page);
        }
        let solid = self.screens.solid.clone().unwrap();
        if let Some(k) = crate::viewers::solid::peek_key(&bytes) { eprintln!("GPUI_LOG {{\"plot_frame\":{{\"key\":{},\"bytes\":{}}}}}", serde_json::Value::String(k), bytes.len()); }
        let Some(bytes) = solid.update(cx, |p, _| p.accept(bytes)) else { cx.notify(); return };
        if let Err(e) = self.screens.plot.receive(&bytes) { eprintln!("GPUI_LOG {{\"plot\":\"{e}\"}}"); }
    }
    /// The Browser page exists while the Browser kind is open (one per thread, polling only while shown); the owner is `browser_owner` (the bot's id on a bot's screen, else the thread).
    pub(crate) fn browser_page(&mut self, cx: &mut Context<Self>) {
        let open = self.data["thread"]["viewer"]["open"].as_str() == Some("browser") || (self.data["bot"]["kind"] == "bot" && self.data["bot"]["tab"] == "browser");
        let owner = browser_owner(&self.data, &self.selected);
        if let Some((o, page)) = &self.screens.browser { if *o != owner || !open { let page = page.clone(); page.update(cx, |p, cx| p.set_visible(false, cx)); if *o != owner { self.screens.browser = None; } } }
        if !open || owner.is_empty() { return; }
        if self.screens.browser.is_none() {
            let Some(t) = self.screens.attacher.target.clone() else { return };
            let (base, tok) = (t.base(), t.token.clone());
            let o = owner.clone(); let hubtx = self.tx.clone();
            let page = cx.new(move |cx| crate::viewers::browser::BrowserPage::new(cx, base, o, tok, hubtx));
            self.screens.browser = Some((owner, page));
        }
        let light = self.light();
        if let Some((_, page)) = &self.screens.browser { page.update(cx, |p, cx| { p.set_light(light, cx); p.set_visible(true, cx) }); }
    }

}

/// Who owns the page the Browser tab shows: a bot's id on a bot's screen, else the thread's own id (as the hub's `BT.World.page` names it),
/// without the `<hub>|` a phone-style id may carry.
pub fn browser_owner(data: &serde_json::Value, selected: &str) -> String {
    if data["bot"]["kind"] == "bot" { if let Some(id) = data["bot"]["id"].as_str().filter(|s| !s.is_empty()) { return id.rsplit('|').next().unwrap_or(id).to_string(); } }
    selected.rsplit('|').next().unwrap_or(selected).to_string()
}

/// How far below the screen the thread may go on before "Latest" is offered (about a screen of rows).
pub const FAR: f32 = 600.0;
static LATEST_FAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `max` the list's scroll range, `y` how far down it is (both px): far when more than FAR is left below.
pub fn far_from(max: f32, y: f32) -> bool { max - y > FAR }

#[cfg(test)]
mod tests {
    #[core::prelude::v1::test] fn latest_is_offered_only_when_the_reader_is_well_above_the_end() {
        assert!(!super::far_from(10_000., 10_000.), "at the end"); assert!(!super::far_from(10_000., 9_500.), "a little above it");
        assert!(super::far_from(10_000., 2_000.), "far up a long thread"); assert!(!super::far_from(0., 0.), "a thread shorter than the screen");
    }

    use super::*;
    use core::prelude::v1::test;
    #[test] fn the_popup_cursor_goes_round() {
        assert_eq!(popup_step(0, 1, 3), 1); assert_eq!(popup_step(2, 1, 3), 0, "past the last, the first"); assert_eq!(popup_step(0, -1, 3), 2, "over the top, the last"); assert_eq!(popup_step(0, 1, 0), 0);
    }
    #[test] fn the_browser_pages_owner_is_the_bot_on_a_bots_screen_else_the_thread_without_its_hub() {
        let d = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
        assert_eq!(browser_owner(&d(r#"{"bot":{"kind":"bot","id":"b7"}}"#), "t1"), "b7");
        assert_eq!(browser_owner(&d(r#"{"bot":null}"#), "t1"), "t1");
        assert_eq!(browser_owner(&d(r#"{}"#), "127.0.0.1:3833|t1"), "t1", "a qualified thread id is stripped");
        assert_eq!(browser_owner(&d(r#"{"bot":{"kind":"bot","id":"h:1|b7"}}"#), "t1"), "b7");
    }
}

/// Rows `old` replaced by `count` new ones. The list resets the offset inside the top row to 0 when that row is among those replaced, which threw a reader
/// scrolled inside a streaming message back to its top on every delta (they could never scroll down). A row replaced in place keeps its offset.
pub(crate) fn splice_keeping(list: &ListState, old: std::ops::Range<usize>, count: usize) {
    let top = list.logical_scroll_top();
    let was = list.item_count();
    list.splice(old.clone(), count);
    if top.item_ix < was && old.contains(&top.item_ix) && top.item_ix < old.start + count { list.scroll_to(top); }
}

#[cfg(test)]
mod splice_tests {
    use super::*;
    fn list(n: usize) -> ListState { ListState::new(n, ListAlignment::Bottom, px(100.)) }
    #[core::prelude::v1::test] fn a_row_grown_in_place_keeps_the_reader_inside_it() {
        let l = list(5); l.scroll_to(ListOffset { item_ix: 4, offset_in_item: px(30.) });
        for _ in 0..3 { splice_keeping(&l, 4..5, 1); }
        let t = l.logical_scroll_top(); assert_eq!((t.item_ix, f32::from(t.offset_in_item)), (4, 30.));
        l.scroll_by(px(10.)); splice_keeping(&l, 4..5, 1);
        assert_eq!(f32::from(l.logical_scroll_top().offset_in_item), 40., "scrolled down stays");
    }
    #[core::prelude::v1::test] fn following_the_end_stays_following_and_rows_above_shift_the_top() {
        let l = list(5); splice_keeping(&l, 4..5, 1); splice_keeping(&l, 5..5, 1);
        assert_eq!(l.logical_scroll_top().item_ix, l.item_count(), "at the end it follows");
        l.scroll_to(ListOffset { item_ix: 3, offset_in_item: px(7.) }); splice_keeping(&l, 0..0, 2);
        assert_eq!(l.logical_scroll_top().item_ix, 5);
    }
}

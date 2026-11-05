//! The few places where the window hands control to the screens: all of Backplane's screen-specific
//! methods live here so main.rs keeps one call at each seam.
use super::{asks, attach::{self, DEFAULT_CAP}, composer::ComposerEvent, timeline};
use crate::Backplane;
use gpui::*;

impl Backplane {
    /// A composer event (paste, drop, picked files): start the uploads off the UI thread; say why when nothing starts.
    pub(crate) fn attach_event(&mut self, event: &ComposerEvent, cx: &mut Context<Self>) {
        if matches!(event, ComposerEvent::CopyTimeline) { return self.timeline_copy(cx); }
        let cap = self.data["thread"]["cap"].as_u64().unwrap_or(DEFAULT_CAP);
        if let Err(e) = self.screens.attacher.event(&self.tx, &self.selected, cap, event) { self.error = e; }
        cx.notify();
    }
    pub(crate) fn pick_files(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
        cx.spawn(async move |weak, cx| {
            if let Ok(Ok(Some(paths))) = rx.await { let _ = weak.update(cx, |this, cx| this.attach_event(&ComposerEvent::Files(paths), cx)); }
        }).detach();
    }
    /// The composer's text changed: keep it as this thread's draft (written in the background) and tell the hub,
    /// which decides the popup rows (`thread.skills`) from it. True when the text really changed.
    pub(crate) fn draft_changed(&mut self, text: &str) -> bool {
        if self.drafts.get(&self.selected).map_or(text.is_empty(), |d| d == text) { return false; }
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
        if !selected.is_empty() && self.selected.is_empty() {
            self.screens.attacher.show(selected);
            if let Some(d) = self.drafts.get(selected).cloned() { if self.input.read(cx).content.is_empty() { self.input.update(cx, |i, cx| i.set(d, cx)); } }
        }
        self.place_restore(selected);
        asks::sync(self, cx);
    }
    /// Leaving `self.selected` for `raw`: flush its draft, remember its scroll, show the target's remembered scroll.
    /// (The picture itself is the window's `thread_cache`.)
    pub(crate) fn thread_switch(&mut self, raw: &str) {
        self.screens.drafts.kick();
        let (y, max) = (f32::from(self.scroll.offset().y), f32::from(self.scroll.max_offset().height));
        if !self.selected.is_empty() {
            if timeline::at_end(y, max) { self.screens.offsets.remove(&self.selected); } else { self.screens.offsets.insert(self.selected.clone(), y); }
        }
        self.screens.attacher.show(raw);
        self.screens.images.retry();
        match self.screens.offsets.get(raw).copied() {
            Some(y) => { self.scroll.set_offset(point(px(0.), px(y))); self.screens.restoring = true; }
            None => { self.timeline_to_end(); self.screens.restoring = false; }
        }
    }
    pub(crate) fn timeline_to_end(&self) { self.scroll.set_offset(point(px(0.), px(-1_000_000.))); }
    /// A picture was applied: the timeline goes to its end only as `timeline::follows` says, so a reader who scrolled up stays where they are.
    pub(crate) fn timeline_follow(&mut self) {
        let id = crate::string(&self.data["thread"], "id"); let raw = attach::raw_id(&id).to_string();
        if raw.is_empty() { return; }
        let same = raw == self.selected;
        let (y, max) = (f32::from(self.scroll.offset().y), f32::from(self.scroll.max_offset().height));
        if timeline::follows(same, self.screens.offsets.contains_key(&raw), self.screens.restoring, y, max) { self.timeline_to_end(); }
        if same { self.screens.restoring = false; }
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
        super::projects::sync(self, cx);
        self.browser_page(cx);
        // a form or a room page opened: its first field takes the keys; closed: back to the composer
        let open = super::bots::focus_target(&self.data);
        if open != self.screens.bots_open { self.screens.bots_open = open.clone(); if open.is_empty() { self.want_focus = true; } else { self.screens.focus_field = Some(open); } }
        // settings opened: the keys go into it; closed: back to the composer
        let open = self.data["settings"].is_object();
        if open != self.screens.settings_was_open { self.screens.settings_was_open = open; if open { self.screens.focus_settings = true; } else { self.want_focus = true; } }
    }
    /// A plot-lane frame arrived on the socket: a solid (3D) frame goes to the 3D page, anything else to the plot store.
    pub(crate) fn plot_received(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        if self.screens.solid.is_none() { self.screens.solid = Some(cx.new(crate::viewers::solid::SolidPage::new)); }
        let solid = self.screens.solid.clone().unwrap();
        if let Some(k) = crate::viewers::solid::peek_key(&bytes) { eprintln!("GPUI_LOG {{\"plot_frame\":{{\"key\":{},\"bytes\":{}}}}}", serde_json::Value::String(k), bytes.len()); }
        let Some(bytes) = solid.update(cx, |p, _| p.accept(bytes)) else { cx.notify(); return };
        if let Err(e) = self.screens.plot.receive(&bytes) { eprintln!("GPUI_LOG {{\"plot\":\"{e}\"}}"); }
    }
    /// The Browser page exists while the Browser kind is open (one per thread, polling only while shown); the owner is `browser_owner` (the bot's id on a bot's screen, else the thread).
    pub(crate) fn browser_page(&mut self, cx: &mut Context<Self>) {
        let open = self.data["thread"]["viewer"]["open"].as_str() == Some("browser");
        let owner = browser_owner(&self.data, &self.selected);
        if let Some((o, page)) = &self.screens.browser { if *o != owner || !open { let page = page.clone(); page.update(cx, |p, _| p.set_visible(false)); if *o != owner { self.screens.browser = None; } } }
        if !open || owner.is_empty() { return; }
        if self.screens.browser.is_none() {
            let Some(t) = self.screens.attacher.target.clone() else { return };
            let (base, tok) = (format!("http://{}", t.addr), t.token.clone());
            let o = owner.clone();
            let page = cx.new(move |cx| crate::viewers::browser::BrowserPage::new(cx, base, o, tok));
            self.screens.browser = Some((owner, page));
        }
        if let Some((_, page)) = &self.screens.browser { page.update(cx, |p, _| p.set_visible(true)); }
    }

}

/// Who owns the page the Browser tab shows: a bot's id on a bot's screen, else the thread's own id (as the hub's `BT.World.page` names it),
/// without the `<hub>|` a phone-style id may carry.
pub fn browser_owner(data: &serde_json::Value, selected: &str) -> String {
    if data["bot"]["kind"] == "bot" { if let Some(id) = data["bot"]["id"].as_str().filter(|s| !s.is_empty()) { return id.rsplit('|').next().unwrap_or(id).to_string(); } }
    selected.rsplit('|').next().unwrap_or(selected).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn the_browser_pages_owner_is_the_bot_on_a_bots_screen_else_the_thread_without_its_hub() {
        let d = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
        assert_eq!(browser_owner(&d(r#"{"bot":{"kind":"bot","id":"b7"}}"#), "t1"), "b7");
        assert_eq!(browser_owner(&d(r#"{"bot":null}"#), "t1"), "t1");
        assert_eq!(browser_owner(&d(r#"{}"#), "127.0.0.1:3833|t1"), "t1", "a qualified thread id is stripped");
        assert_eq!(browser_owner(&d(r#"{"bot":{"kind":"bot","id":"h:1|b7"}}"#), "t1"), "b7");
    }
}

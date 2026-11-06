//! Browser tab: the hub's newest browser frame for a bot (GET /bots/<id>/screen.jpg, the JPEG the helper wrote). A poller thread exists only while the
//! page does (dropped with it); it fetches every 700 ms, decodes only when the bytes changed, and the page redraws only then, so an idle page
//! (an unchanged frame) costs no repaint. Input to the page (bots.browser clicks/keys, teach mode) is not wired yet.
use gpui::{prelude::*, *};
use crate::{art, theme};
use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, time::{Duration, Instant}};

pub const POLL_MS: u64 = 700;
pub const MAX_JPEG: u64 = 8 << 20;

/// BGRA RenderImage from JPEG bytes; None when they are not a decodable JPEG.
pub fn decode_jpeg(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).ok()?;
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() { p.0.swap(0, 2); } // gpui's image container holds BGRA
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
fn fnv(bytes: &[u8]) -> u64 { bytes.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3)) }

pub struct BrowserPage { #[cfg(feature = "cef")] pub cef: Option<Entity<super::cef_page::CefPage>>, bar: Bar, pub light: bool, open_error: Option<String>, opening_since: Instant, image: Option<Arc<RenderImage>>, retired: Vec<Arc<RenderImage>>, pub status: String, stop: Arc<AtomicBool>, visible: Arc<AtomicBool> }
impl BrowserPage {
    /// `base` like http://127.0.0.1:3821 (http only for now: this client has no TLS, so an https base never starts the poller), `owner` the page's owner as the hub names it (a bot's id, or the thread's id
    /// for a thread's page: the host passes `BT.World.page`'s value), `token` the hub token (a Bearer header, never logged). The poller fetches only while
    /// `set_visible(true)`; mount the page with the tab and call `set_visible(false)` when another tab is shown.
    pub fn new(cx: &mut Context<Self>, base: String, owner: String, token: String, hub: tokio::sync::mpsc::Sender<crate::hub::Command>) -> Self {
        let hub_tx = hub.clone(); let owner_bar = owner.clone();
        // with the `cef` feature and a usable runtime the page is embedded Chromium (cef_page.rs); otherwise the hub's JPEG frames below
        #[cfg(feature = "cef")]
        let (cef, cef_note) = match super::cef_page::engine_port() {
            Ok(port) => { let o = owner.clone(); (Some(cx.new(move |cx| super::cef_page::CefPage::new(cx, o, hub, port))), String::new()) }
            Err(e) => { eprintln!("GPUI_LOG {{\"cef\":{}}}", serde_json::Value::String(e.clone())); (None, format!("Embedded browser unavailable ({e}); showing the hub's frames.")) }
        };
        #[cfg(feature = "cef")]
        let use_poller = cef.is_none();
        #[cfg(not(feature = "cef"))]
        let use_poller = true;
        let https = base.starts_with("https://");
        let stop = Arc::new(AtomicBool::new(false)); let visible = Arc::new(AtomicBool::new(false)); let vis = visible.clone(); // fetches only after set_visible(true)
        let (tx, rx) = async_channel::bounded::<Result<Option<Arc<RenderImage>>, String>>(2); // Ok(None): a good fetch of the frame already shown
        let flag = stop.clone(); let hub_open = hub_tx.clone();
        if !https && use_poller { std::thread::Builder::new().name("browser-frames".into()).spawn(move || {
            let url = format!("{}/bots/{}/screen.jpg", base.trim_end_matches('/'), owner);
            let (mut last, mut failing) = (0u64, false); let mut opened: Option<std::time::Instant> = None;
            while !flag.load(Ordering::Relaxed) {
                if !vis.load(Ordering::Relaxed) { std::thread::sleep(Duration::from_millis(POLL_MS)); continue } // another tab is shown: no requests
                let out = match ureq::get(&url).set("Authorization", &format!("Bearer {token}")).timeout(Duration::from_secs(3)).call() {
                    Ok(resp) => {
                        let mut body = Vec::new();
                        match std::io::Read::read_to_end(&mut std::io::Read::take(resp.into_reader(), MAX_JPEG), &mut body) {
                            Ok(_) if fnv(&body) == last => if failing { failing = false; Some(Ok(None)) } else { None }, // the same frame: nothing to draw (but a recovery clears the status)
                            Ok(_) => { last = fnv(&body); failing = false; Some(decode_jpeg(&body).map(Some).ok_or_else(|| "frame is not a JPEG".to_string())) }
                            Err(e) => Some(Err(format!("read: {e}"))),
                        }
                    }
                    Err(ureq::Error::Status(404, _)) => {
                        // no page exists for this owner yet: ask the hub to open one (at most every 5 s), its first frame follows
                        if opened.map_or(true, |t| t.elapsed() > Duration::from_secs(5)) { opened = Some(std::time::Instant::now());
                            let _ = hub_open.try_send(crate::hub::Command::Rpc("bots.browser".into(), crate::screens::settings::Secret(serde_json::json!({"bot": owner, "op": "open"})), "browser")); }
                        Some(Err("Opening the browser…".into()))
                    }
                    Err(ureq::Error::Status(c, _)) => Some(Err(format!("hub answered {c}"))),
                    Err(ureq::Error::Transport(t)) => Some(Err(format!("hub unreachable: {}", t.kind()))),
                };
                if matches!(out, Some(Err(_))) { failing = true; }
                if let Some(msg) = out { if tx.try_send(msg).is_err() && tx.is_closed() { return } }
                std::thread::sleep(Duration::from_millis(POLL_MS));
            }
        }).ok(); }
        cx.spawn(async move |weak, cx| {
            while let Ok(msg) = rx.recv().await {
                if weak.update(cx, |this, cx| { match msg { Ok(Some(i)) => { if let Some(old) = this.image.replace(i) { this.retired.push(old); } this.status.clear(); } Ok(None) => this.status.clear(), Err(e) => this.status = e } cx.notify(); }).is_err() { break }
            }
        }).detach();
        #[cfg(feature = "cef")]
        let cef_note2 = cef_note;
        #[cfg(not(feature = "cef"))]
        let cef_note2 = String::new();
        let status = if !cef_note2.is_empty() { cef_note2 } else if https { "https hubs are not supported by this page yet (the client has no TLS)".to_string() } else { "Waiting for the first frame".to_string() };
        BrowserPage { #[cfg(feature = "cef")] cef, bar: Bar::new(cx, hub_tx.clone(), owner_bar), light: false, open_error: None, opening_since: Instant::now(), image: None, retired: Vec::new(), status, stop, visible }
    }
    /// The helper's `tabs` event for this page (info `browser.tabs.<owner>`): the tab strip and the address of the active tab.
    /// The hub refused to open or drive the page (a toolbar or open request answered an error): said in the tab with what to do, until a frame arrives.
    pub fn open_failed(&mut self, why: &str, cx: &mut Context<Self>) { if self.image.is_none() { self.open_error = Some(why.to_string()); cx.notify(); } }
    fn retry(&mut self, cx: &mut Context<Self>) { self.open_error = None; self.opening_since = Instant::now(); self.bar.rpc(serde_json::json!({"bot": self.bar.owner, "op": "open"})); cx.notify(); }
    pub fn set_tabs(&mut self, text: &str, cx: &mut Context<Self>) { self.bar.set_tabs(text); cx.notify(); }
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        #[cfg(feature = "cef")]
        if let Some(c) = &self.cef { let v = visible; c.update(cx, |p, _| p.set_visible(v)); } self.visible.store(visible, Ordering::Relaxed); }
    /// Give the images back to the atlas (call before dropping the page, with the window).
    pub fn release(&mut self, window: &mut Window) { for i in self.retired.drain(..).chain(self.image.take()) { let _ = window.drop_image(i); } }
}
impl Drop for BrowserPage { fn drop(&mut self) { self.stop.store(true, Ordering::Relaxed); } }
/// What the empty tab says: what is wrong and what to do about it, never an endless "Opening…".
fn open_message(error: &Option<String>, waited: Duration) -> Option<(String, String)> {
    if let Some(e) = error {
        let hint = if e.contains("backplane-browser") { "The hub has no browser helper. Put backplane-browser next to backplane-hub (scripts/build-browser.sh builds it), install Backplane's helper, or set BACKPLANE_BROWSER=/path to it, then press Retry." }
            else if e.to_lowercase().contains("chrom") || e.to_lowercase().contains("executable") { "The helper found no Chrome to run. Install Google Chrome (or set BACKPLANE_CHROME=/path), then press Retry." }
            else { "Press Retry, or ask the agent to open a page." };
        return Some((format!("The browser could not open: {}", e.chars().take(200).collect::<String>()), hint.to_string()));
    }
    (waited > Duration::from_secs(8)).then(|| ("The browser has not opened in 8 seconds.".to_string(), "The hub's browser helper may be missing or still starting (it downloads Chrome the first time). Press Retry; if it keeps failing, check that backplane-browser sits next to backplane-hub.".to_string()))
}
impl Render for BrowserPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.retired.drain(..) { let _ = window.drop_image(old); } // a painted RenderImage stays in the atlas until it is dropped
        #[cfg(feature = "cef")]
        if let Some(c) = &self.cef { return div().id("browser-page").size_full().child(c.clone()).into_any_element(); }
        let c = |n: &str| theme::color(self.light, n);
        let status = self.status.clone();
        let size = self.image.as_ref().and_then(|i| { let s = i.size(0); Some((s.width.0 as f32, s.height.0 as f32)) });
        let content = match &self.image {
            // a dead hub must not look like a live, frozen page: the status shows over the last frame
            Some(i) => div().id("browser-view").size_full().relative().bg(c("bg")).child(img(i.clone()).size_full().object_fit(ObjectFit::Contain))
                .when(!status.is_empty(), |d| d.child(div().absolute().bottom_2().left_2().px_2().py_1().text_size(px(12.)).bg(c("panel")).border_1().border_color(c("line")).text_color(c("bad")).child(status))).into_any_element(),
            None => {
                let said = open_message(&self.open_error, self.opening_since.elapsed());
                let waiting = said.is_none() && status == "Opening the browser…";
                let line = said.as_ref().map(|m| m.0.clone()).unwrap_or_else(|| status.clone());
                div().id("browser-view").size_full().bg(c("bg")).flex().flex_col().items_center().justify_center().gap_3().px_6().text_size(px(13.)).text_color(c("dim"))
                    .child(div().max_w(px(480.)).text_color(c(if said.is_some() { "text" } else { "dim" })).child(line))
                    .children(said.as_ref().map(|m| div().max_w(px(480.)).text_size(px(12.)).child(m.1.clone())))
                    .children(said.is_some().then(|| div().id("browser-retry").px_4().py_1().cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).text_color(c("text")).hover(|s| s.border_color(c("accent"))).child("Retry")
                        .on_click(cx.listener(|this, _, _, cx| this.retry(cx)))))
                    .when(waiting, |d| d.child(div().text_size(px(11.)).text_color(c("faint")).child("starting the hub's browser…")))
                    .into_any_element()
            }
        };
        let light = self.light;
        let bar = self.bar.render(light, size, content, window, cx);
        div().id("browser-page").size_full().child(bar).into_any_element()
    }
}

/// The normal browser chrome over the hub's frames: back, forward, reload, an address field, tabs (select, close, new) and the person's mouse, wheel and keys on the page.
/// Everything goes to the hub as `bots.browser` (the toolbar commands and the person's input; the first one takes the page over from the agent, and "Hand back"
/// returns it), the tabs come back as the helper's `tabs` event (info `browser.tabs.<owner>`).
pub struct Bar { hub: tokio::sync::mpsc::Sender<crate::hub::Command>, owner: String, tabs: Vec<(String, String, String, bool)>, url: String, text: String, edit: bool, taken: bool, fresh: bool,
    focus: FocusHandle, bar_focus: FocusHandle, bounds: Arc<std::sync::Mutex<Option<Bounds<Pixels>>>>, image: (f32, f32) }
impl Bar {
    fn new(cx: &mut Context<BrowserPage>, hub: tokio::sync::mpsc::Sender<crate::hub::Command>, owner: String) -> Self {
        Bar { hub, owner, tabs: Vec::new(), url: String::new(), text: String::new(), edit: false, taken: false, fresh: false, focus: cx.focus_handle(), bar_focus: cx.focus_handle(), bounds: Arc::default(), image: (1.0, 1.0) }
    }
    fn rpc(&self, p: serde_json::Value) { let _ = self.hub.try_send(crate::hub::Command::Rpc("bots.browser".into(), crate::screens::settings::Secret(p), "browser")); }
    fn toolbar(&mut self, command: &str, extra: serde_json::Value) {
        let mut p = serde_json::json!({"bot": self.owner, "op": "toolbar", "command": command}); if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) { for (k, v) in e { o.insert(k.clone(), v.clone()); } }
        self.taken = true; self.rpc(p);
    }
    fn input(&mut self, p: serde_json::Value) { if !self.taken { self.taken = true; self.rpc(serde_json::json!({"bot": self.owner, "op": "takeover"})); } let mut p = p; p["bot"] = self.owner.clone().into(); self.rpc(p); }
    pub fn set_tabs(&mut self, text: &str) {
        eprintln!("GPUI_LOG {{\"browser_tabs\":{}}}", serde_json::Value::String(text.chars().take(600).collect()));
        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { self.tabs.clear(); return };
        self.tabs = v["tabs"].as_array().into_iter().flatten().map(|t| (t["id"].as_str().unwrap_or("").to_string(), t["title"].as_str().unwrap_or("").to_string(), t["url"].as_str().unwrap_or("").to_string(), t["active"].as_bool().unwrap_or(false))).collect();
        self.url = v["url"].as_str().unwrap_or("").to_string(); if !self.edit { self.text = self.url.clone(); }
    }
    /// 0..1000 over the shown frame, which the view letterboxes (Contain); None outside the frame.
    fn thousandths(&self, p: Point<Pixels>) -> Option<(u32, u32)> {
        let b = (*self.bounds.lock().unwrap())?; let (iw, ih) = self.image; let (bw, bh) = (f32::from(b.size.width), f32::from(b.size.height));
        let k = (bw / iw).min(bh / ih); let (w, h) = (iw * k, ih * k); let (ox, oy) = ((bw - w) / 2.0, (bh - h) / 2.0);
        let (x, y) = (f32::from(p.x - b.origin.x) - ox, f32::from(p.y - b.origin.y) - oy);
        (x >= 0.0 && y >= 0.0 && x <= w && y <= h).then(|| ((x / w * 1000.0) as u32, (y / h * 1000.0) as u32))
    }
    fn go(&mut self) {
        let t = self.text.trim().to_string(); if t.is_empty() { return }
        let t = if t.contains(' ') || (!t.contains('.') && !t.contains(':') && !t.starts_with("localhost")) { format!("https://www.google.com/search?q={}", t.replace(' ', "+")) } else { t };
        self.toolbar("navigate", serde_json::json!({"url": t})); self.edit = false;
    }
    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<BrowserPage>) {
        let k = &e.keystroke; let m = &k.modifiers;
        if self.bar_focus.is_focused(window) {
            // the address is selected when the field is entered: the first thing typed replaces it
            if self.fresh && !matches!(k.key.as_str(), "enter" | "escape" | "left" | "right" | "tab") && !(m.control && k.key != "v" && k.key != "a") { self.text.clear(); self.fresh = false; }
            match k.key.as_str() {
                "enter" => self.go(), "escape" => { self.edit = false; self.text = self.url.clone(); window.focus(&self.focus); }
                "backspace" => { self.text.pop(); self.edit = true; }
                "v" if m.control || m.platform => { if let Some(t) = cx.read_from_clipboard().and_then(|c| c.text()) { self.text.push_str(t.lines().next().unwrap_or("")); self.edit = true; } }
                "a" if m.control || m.platform => { self.text.clear(); self.edit = true; }
                _ => if let Some(c) = &k.key_char { if !m.control && !m.platform { self.text.push_str(c); self.edit = true; } },
            }
            cx.stop_propagation(); return;
        }
        if (m.control || m.platform) && k.key == "l" { window.focus(&self.bar_focus); self.edit = true; cx.stop_propagation(); return }
        if (m.control || m.platform) && !m.alt { return }
        match &k.key_char { Some(c) if !c.is_empty() && k.key != "enter" => self.input(serde_json::json!({"op": "type", "text": c})), _ => { let key = match k.key.as_str() { "enter" => "Enter", "backspace" => "Backspace", "tab" => "Tab", "escape" => "Escape", "left" => "ArrowLeft", "right" => "ArrowRight", "up" => "ArrowUp", "down" => "ArrowDown", "delete" => "Delete", "home" => "Home", "end" => "End", "pageup" => "PageUp", "pagedown" => "PageDown", _ => "" }; if !key.is_empty() { self.input(serde_json::json!({"op": "press", "key": key})); } } }
        cx.stop_propagation();
    }
    /// A square icon button of the toolbar: hover raises it, disabled dims it and takes no click.
    fn icon_button(light: bool, id: &'static str, name: &'static str, on: bool) -> Stateful<Div> {
        let c = |n: &str| theme::color(light, n);
        div().id(id).w(px(28.)).h(px(28.)).flex().items_center().justify_center().flex_shrink_0().border_1().border_color(c("panel"))
            .child(art::icon(name, 16., c(if on { "text" } else { "faint" })))
            .when(on, |d| d.cursor_pointer().hover(|s| s.bg(c("raise")).border_color(c("line")))).when(!on, |d| d.opacity(0.6))
    }
    fn render(&mut self, light: bool, size: Option<(f32, f32)>, content: AnyElement, window: &mut Window, cx: &mut Context<BrowserPage>) -> AnyElement {
        if let Some(s) = size { self.image = s; }
        let c = |n: &str| theme::color(light, n);
        let focused = self.bar_focus.is_focused(window); let bounds = self.bounds.clone();
        let have = !self.tabs.is_empty();
        // the tab strip is always there, even with one tab: title, close, and a + at its end
        let mut strip = div().h(px(32.)).flex().flex_row().items_end().bg(c("panel")).border_b_1().border_color(c("line")).text_size(px(12.)).px_1().gap_px().overflow_hidden();
        for (i, (id, title, url, on)) in self.tabs.iter().cloned().enumerate() {
            let label: String = (if title.is_empty() { url.trim_start_matches("https://").trim_start_matches("http://").to_string() } else { title }).chars().take(24).collect();
            let (sel, close) = (id.clone(), id);
            strip = strip.child(div().h(px(28.)).min_w(px(96.)).max_w(px(200.)).flex().flex_row().items_center().gap_1().pl_3().pr_1()
                .border_1().border_b_0().border_color(c(if on { "line" } else { "panel" })).bg(c(if on { "bg" } else { "panel" })).text_color(c(if on { "text" } else { "dim" }))
                .child(div().id(("btab", i)).flex_1().h_full().flex().items_center().cursor_pointer().overflow_hidden().child(if label.is_empty() { "New tab".to_string() } else { label })
                    .on_click(cx.listener(move |this, _, _, cx| { this.bar.toolbar("tabs", serde_json::json!({"selectId": sel})); cx.notify(); })))
                .child(div().id(("btabx", i)).w(px(20.)).h(px(20.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c("raise"))).child(art::icon("x", 12., c("dim")))
                    .on_click(cx.listener(move |this, _, _, cx| { this.bar.toolbar("tabs", serde_json::json!({"closeId": close})); cx.notify(); }))));
        }
        strip = strip.child(div().id("br-new").w(px(28.)).h(px(28.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c("raise"))).child(art::icon("plus", 14., c("text")))
            .on_click(cx.listener(|this, _, w, cx| { this.bar.toolbar("tabs", serde_json::json!({"new": true})); this.bar.text.clear(); this.bar.edit = true; this.bar.fresh = false; w.focus(&this.bar.bar_focus); cx.notify(); })));
        let taken = self.taken;
        let field_border = if focused { c("accent") } else { c("line") };
        let chrome = div().h(px(40.)).flex().flex_row().items_center().gap_1().px_2().bg(c("bg")).border_b_1().border_color(c("line"))
            .child(Self::icon_button(light, "br-back", "chevron-left", have).on_click(cx.listener(|this, _, _, _| this.bar.toolbar("back", serde_json::json!({})))))
            .child(Self::icon_button(light, "br-fwd", "chevron-right", have).on_click(cx.listener(|this, _, _, _| this.bar.toolbar("forward", serde_json::json!({})))))
            .child(Self::icon_button(light, "br-reload", "undo", have).on_click(cx.listener(|this, _, _, _| this.bar.toolbar("reload", serde_json::json!({})))))
            .child(div().id("br-url").flex_1().h(px(28.)).px_2().flex().flex_row().items_center().gap_2().text_size(px(13.)).bg(c("panel")).border_1().border_color(field_border)
                .track_focus(&self.bar_focus).key_context("BrowserBar").cursor_text()
                .on_key_down(cx.listener(|this, e, w, cx| { this.bar.key(e, w, cx); cx.notify(); }))
                .on_click(cx.listener(|this, _, w, cx| { w.focus(&this.bar.bar_focus); this.bar.edit = true; this.bar.fresh = true; cx.notify(); }))
                .child(art::icon("globe", 14., c("dim")))
                .child(div().flex_1().overflow_hidden().text_color(c(if self.text.is_empty() { "dim" } else { "text" }))
                    .child(format!("{}{}", if self.text.is_empty() { "Search or enter address".to_string() } else { self.text.clone() }, if focused { "▏" } else { "" }))))
            .when(taken, |d| d.child(div().id("br-hand").h(px(28.)).px_3().flex().items_center().cursor_pointer().bg(c("accent")).text_color(c("on-accent")).text_size(px(12.)).child("Hand back")
                .on_click(cx.listener(|this, _, _, cx| { this.bar.taken = false; this.bar.rpc(serde_json::json!({"bot": this.bar.owner, "op": "handback"})); cx.notify(); }))));
        let view = div().id("browser-frame").flex_1().relative().overflow_hidden().track_focus(&self.focus).key_context("BrowserView")
            .child(canvas(move |b, _, _| { *bounds.lock().unwrap() = Some(b); b }, |_, _, _, _| {}).absolute().size_full())
            .child(content)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, w, cx| { w.focus(&this.bar.focus); this.bar.edit = false; if let Some((x, y)) = this.bar.thousandths(e.position) { this.bar.input(serde_json::json!({"op": "click", "x": x, "y": y})); } cx.notify(); }))
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, _| { let dy = match e.delta { ScrollDelta::Pixels(p) => -f32::from(p.y), ScrollDelta::Lines(l) => -l.y * 40.0 }; let dx = match e.delta { ScrollDelta::Pixels(p) => -f32::from(p.x), ScrollDelta::Lines(l) => -l.x * 40.0 }; this.bar.input(serde_json::json!({"op": "scroll", "dx": dx, "dy": dy})); }))
            .on_key_down(cx.listener(|this, e, w, cx| this.bar.key(e, w, cx)));
        div().size_full().flex().flex_col().bg(c("bg")).child(strip).child(chrome).child(view).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test] fn an_empty_tab_names_what_is_missing_then_times_out_instead_of_waiting_forever() {
        use super::open_message; use std::time::Duration;
        assert!(open_message(&None, Duration::from_secs(2)).is_none(), "still starting: no complaint yet");
        let (said, hint) = open_message(&None, Duration::from_secs(9)).unwrap(); assert!(said.contains("8 seconds") && hint.contains("Retry"));
        let (said, hint) = open_message(&Some("the Backplane browser could not start: no backplane-browser helper".into()), Duration::ZERO).unwrap();
        assert!(said.contains("could not open") && hint.contains("BACKPLANE_BROWSER") && hint.contains("Retry"));
    }
    // not `use super::*`: that glob brings gpui's own `test` attribute macro into scope and #[test] then expands into itself
    use super::{decode_jpeg, fnv};
    fn jpeg(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb(rgb)); let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Jpeg).unwrap(); out.into_inner()
    }
    #[test] fn decodes_a_jpeg_to_bgra() {
        let i = decode_jpeg(&jpeg(16, 8, [200, 40, 40])).unwrap();
        let b = i.as_bytes(0).unwrap(); assert_eq!(b.len(), 16 * 8 * 4);
        assert!(b[2] > 150 && b[0] < 100, "BGRA: red lands in byte 2, got {:?}", &b[..4]);
    }
    #[test] fn refuses_bytes_that_are_not_a_jpeg() { assert!(decode_jpeg(b"not a jpeg").is_none()); assert!(decode_jpeg(&[]).is_none()); }
    #[test] fn the_same_bytes_hash_the_same_and_a_new_frame_differs() { let a = jpeg(8, 8, [1, 2, 3]); assert_eq!(fnv(&a), fnv(&a.clone())); assert_ne!(fnv(&a), fnv(&jpeg(8, 8, [200, 2, 3]))); }
}

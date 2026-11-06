//! The Browser tab on embedded Chromium (feature `cef`): toolbar (back, forward, reload, address, new tab, hand back), tab strip, and the page as one
//! gpui image repainted from CEF's OSR frames, with the person's mouse, wheel, keys, IME text and focus going to the page. The agent drives the same page
//! over CDP: the engine's loopback port is handed to the hub (`bots.browser` op `attach`), and the person's first click takes the page over
//! (`takeover`; "Hand back" gives it back), the semantics of docs/browser-native.md.
use super::cef::{self, Cur, Engine, Tab};
use crate::{hub::Command, screens::settings::Secret};
use gpui::{prelude::*, *};
use std::{ops::Range, sync::{Arc, Mutex}};

pub struct CefPage {
    tabs: Vec<Arc<Tab>>, active: usize, focus: FocusHandle, bar_focus: FocusHandle, bar_text: String, bar_edit: bool,
    bounds: Arc<Mutex<Option<Bounds<Pixels>>>>, image: Option<Arc<RenderImage>>, retired: Vec<Arc<RenderImage>>,
    wake: async_channel::Sender<()>, hub: tokio::sync::mpsc::Sender<Command>, owner: String, port: u16,
    pub taken: bool, stats: (u32, u32, usize), buttons: u32, visible: bool, composing: bool, attached: Vec<String>, pub status: String, last_mouse: (i32, i32),
}

impl CefPage {
    pub fn new(cx: &mut Context<Self>, owner: String, hub: tokio::sync::mpsc::Sender<Command>, port: u16) -> Self {
        let (wake, rx) = async_channel::bounded::<()>(1);
        cx.spawn(async move |weak, cx| {
            while rx.recv().await.is_ok() {
                if weak.update(cx, |this, cx| this.pump(cx)).is_err() { break }
                cx.background_executor().timer(std::time::Duration::from_millis(16)).await; // at most ~60 repaints a second
            }
        }).detach();
        let mut me = CefPage { tabs: Vec::new(), active: 0, focus: cx.focus_handle(), bar_focus: cx.focus_handle(), bar_text: String::new(), bar_edit: false, bounds: Arc::default(), image: None, retired: Vec::new(), wake, hub, owner, port, taken: false, buttons: 0, visible: false, composing: false, attached: Vec::new(), status: String::new(), stats: (0, 0, 0), last_mouse: (0, 0) };
        me.new_tab("about:blank", cx);
        me
    }
    fn tab(&self) -> Option<&Arc<Tab>> { self.tabs.get(self.active) }
    fn new_tab(&mut self, url: &str, cx: &mut Context<Self>) {
        let (w, h) = self.bounds.lock().unwrap().map(|b| (f32::from(b.size.width) as u32, f32::from(b.size.height) as u32)).unwrap_or((900, 600));
        let t = Tab::open(url, w, h, 1.0, self.wake.clone());
        self.tabs.push(t); self.active = self.tabs.len() - 1; self.image = None; self.bar_text = url.to_string(); self.bar_edit = false; cx.notify();
    }
    fn rpc(&self, params: serde_json::Value) { let _ = self.hub.try_send(Command::Rpc("bots.browser".into(), Secret(params), "cef")); }
    /// Register the engine's CDP endpoint with the hub for this owner (the hub's helper attaches to the tabs named here), again when the tab set changes.
    fn attach(&mut self) {
        let Some(t) = self.tab() else { return };
        let id = t.id.load(std::sync::atomic::Ordering::Relaxed); if id == 0 { return }
        let url = t.st.lock().unwrap().url.clone();
        let key = format!("{id}|{url}"); if self.attached.last() == Some(&key) { return }
        self.attached.push(key);
        let targets = self.tabs.iter().map(|t| t.st.lock().unwrap().url.clone()).collect::<Vec<_>>();
        self.rpc(serde_json::json!({"bot": self.owner, "op": "attach", "endpoint": format!("http://127.0.0.1:{}", self.port), "active": self.active, "urls": targets}));
    }
    /// Frames, popups, closed tabs, address changes: everything CEF's callbacks left for the main thread.
    fn pump(&mut self, cx: &mut Context<Self>) {
        let mut popups = Vec::new();
        for t in &self.tabs { popups.append(&mut t.st.lock().unwrap().popups); }
        for u in popups { self.new_tab(&u, cx); }
        let before = self.tabs.len();
        let act = self.tabs.get(self.active).cloned();
        self.tabs.retain(|t| !t.st.lock().unwrap().closed);
        if self.tabs.len() != before { self.active = act.and_then(|a| self.tabs.iter().position(|t| Arc::ptr_eq(t, &a))).unwrap_or(self.tabs.len().saturating_sub(1)); self.image = None; }
        if self.tabs.is_empty() { self.new_tab("about:blank", cx); }
        if let Some(t) = self.tab().cloned() {
            if let Some(f) = t.take_frame() { self.stats = (f.w, f.h, f.bgra.chunks(4).step_by(97).filter(|p| p[0] < 240 || p[1] < 240 || p[2] < 240).count()); if let Some(buf) = image::RgbaImage::from_raw(f.w, f.h, f.bgra) { // CEF paints BGRA, which is what gpui's RenderImage holds
                if let Some(old) = self.image.replace(Arc::new(RenderImage::new(vec![image::Frame::new(buf)]))) { self.retired.push(old); } } }
            if !self.bar_edit { let u = t.st.lock().unwrap().url.clone(); if u != self.bar_text { self.bar_text = u; } }
            if t.ready() { self.attach(); }
        }
        cx.notify();
    }
    pub fn set_visible(&mut self, v: bool) {
        if self.visible != v { self.visible = v; if let Some(t) = self.tab() { t.hidden(!v); } }
    }
    pub fn release(&mut self, window: &mut Window) { for i in self.retired.drain(..).chain(self.image.take()) { let _ = window.drop_image(i); } }
    fn local(&self, p: Point<Pixels>) -> (i32, i32) {
        let o = self.bounds.lock().unwrap().map(|b| b.origin).unwrap_or_default();
        (f32::from(p.x - o.x) as i32, f32::from(p.y - o.y) as i32)
    }
    fn mods(m: &Modifiers, buttons: u32) -> u32 { cef::flags(m.shift, m.control, m.alt, false, false, false) | buttons }
    fn take_over(&mut self) { if !self.taken { self.taken = true; self.rpc(serde_json::json!({"bot": self.owner, "op": "takeover"})); } }
    fn hand_back(&mut self, cx: &mut Context<Self>) { if self.taken { self.taken = false; self.rpc(serde_json::json!({"bot": self.owner, "op": "handback"})); cx.notify(); } }
    fn go(&mut self, cx: &mut Context<Self>) {
        let raw = self.bar_text.trim().to_string(); if raw.is_empty() { return }
        let url = if raw.contains("://") || raw.starts_with("about:") || raw.starts_with("file:") { raw } else if raw.contains(' ') || !raw.contains('.') && !raw.starts_with("localhost") { format!("https://www.google.com/search?q={}", raw.replace(' ', "+")) } else if raw.starts_with("localhost") || raw.starts_with("127.") { format!("http://{raw}") } else { format!("https://{raw}") };
        self.take_over(); if let Some(t) = self.tab() { t.navigate(&url); }
        self.bar_edit = false; cx.notify();
    }
    fn key_down(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke; let m = &k.modifiers;
        if self.bar_focus.is_focused(window) {
            match k.key.as_str() {
                "enter" => self.go(cx),
                "escape" => { self.bar_edit = false; window.focus(&self.focus); }
                "backspace" => { self.bar_text.pop(); self.bar_edit = true; }
                "v" if m.control || m.platform => { if let Some(t) = cx.read_from_clipboard().and_then(|c| c.text()) { self.bar_text.push_str(t.lines().next().unwrap_or("")); self.bar_edit = true; } }
                "a" if m.control || m.platform => { self.bar_text.clear(); self.bar_edit = true; }
                _ => if let Some(c) = &k.key_char { if !m.control && !m.platform { self.bar_text.push_str(c); self.bar_edit = true; } },
            }
            cx.stop_propagation(); cx.notify(); return;
        }
        let Some(t) = self.tab().cloned() else { return };
        self.take_over();
        let mods = Self::mods(m, 0);
        if (m.control || m.platform) && !m.alt {
            let cmd = match k.key.as_str() { "c" => "copy", "x" => "cut", "v" => "paste", "a" => "all", "z" if m.shift => "redo", "z" => "undo", "y" => "redo", "l" => { window.focus(&self.bar_focus); self.bar_edit = true; cx.stop_propagation(); cx.notify(); return } , _ => "" };
            if !cmd.is_empty() { t.edit(cmd); cx.stop_propagation(); return }
        }
        if let Some(vk) = cef::vk(&k.key) {
            t.key(0, mods, vk, vk, vk as u16);
            if let Some(c) = &k.key_char { if !(m.control || m.platform) { for u in c.encode_utf16() { t.key(2, mods, vk, vk, u); } } }
            else if k.key == "enter" { t.key(2, mods, 13, 13, 13); }
        } else if let Some(c) = &k.key_char { for u in c.encode_utf16() { t.key(2, mods, 0, 0, u); } }
        cx.stop_propagation();
    }
    fn key_up(&mut self, e: &KeyUpEvent, window: &mut Window) {
        if self.bar_focus.is_focused(window) { return }
        if let (Some(t), Some(vk)) = (self.tab(), cef::vk(&e.keystroke.key)) { t.key(1, Self::mods(&e.keystroke.modifiers, 0), vk, vk, vk as u16); }
    }
    fn bar_button(id: &'static str, label: &'static str, on: bool) -> Stateful<Div> {
        div().id(id).px_2().py_1().text_size(px(13.)).text_color(rgb(if on { 0xd7dbe3 } else { 0x596070 })).when(on, |d| d.cursor_pointer().hover(|s| s.bg(rgb(0x2a303b)))).child(label)
    }
}

fn cursor_of(c: Cur) -> CursorStyle {
    match c { Cur::Hand => CursorStyle::PointingHand, Cur::IBeam => CursorStyle::IBeam, Cur::Cross => CursorStyle::Crosshair, Cur::Move => CursorStyle::ClosedHand, Cur::ResizeH => CursorStyle::ResizeLeftRight, Cur::ResizeV => CursorStyle::ResizeUpDown,
        Cur::ResizeNwse => CursorStyle::ResizeUpLeftDownRight, Cur::ResizeNesw => CursorStyle::ResizeUpRightDownLeft, Cur::NotAllowed => CursorStyle::OperationNotAllowed, _ => CursorStyle::Arrow }
}

impl Render for CefPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.retired.drain(..) { let _ = window.drop_image(old); }
        let me = cx.entity();
        let (back, fwd, loading, title, err) = self.tab().map(|t| { let s = t.st.lock().unwrap(); (s.back, s.forward, s.loading, s.title.clone(), s.load_error.clone()) }).unwrap_or_default();
        let cursor = self.tab().map(|t| t.cursor()).unwrap_or(Cur::Default);
        let bar_focused = self.bar_focus.is_focused(window);
        let bounds = self.bounds.clone(); let tab_for_size = self.tab().cloned();
        let focus = self.focus.clone();
        let tabs_row = (self.tabs.len() > 1).then(|| {
            let mut row = div().flex().flex_row().bg(rgb(0x14171c)).text_size(px(12.));
            for (i, t) in self.tabs.iter().enumerate() {
                let st = t.st.lock().unwrap(); let label: String = (if st.title.is_empty() { st.url.clone() } else { st.title.clone() }).chars().take(24).collect();
                row = row.child(div().id(("cef-tab", i)).px_2().py_1().cursor_pointer().text_color(rgb(if i == self.active { 0xe6e9ef } else { 0x8a93a3 })).when(i == self.active, |d| d.bg(rgb(0x232831)))
                    .child(label).on_click(cx.listener(move |this, _, _, cx| { this.active = i; this.image = None; if let Some(t) = this.tab() { t.hidden(false); t.resize(1, 1, 1.0); } cx.notify(); })));
            }
            row
        });
        div().id("cef-page").size_full().flex().flex_col().bg(rgb(0x0f1115))
            .child(div().flex().flex_row().items_center().gap_1().px_1().bg(rgb(0x1a1e25))
                .child(Self::bar_button("cef-back", "‹", back).on_click(cx.listener(|this, _, _, _| { if let Some(t) = this.tab() { t.back(); } })))
                .child(Self::bar_button("cef-fwd", "›", fwd).on_click(cx.listener(|this, _, _, _| { if let Some(t) = this.tab() { t.forward(); } })))
                .child(Self::bar_button("cef-reload", if loading { "✕" } else { "⟳" }, true).on_click(cx.listener(|this, _, _, _| { if let Some(t) = this.tab() { if t.st.lock().unwrap().loading { t.stop() } else { t.reload() } } })))
                .child(div().id("cef-url").flex_1().px_2().py_1().text_size(px(13.)).bg(rgb(0x0f1115)).text_color(rgb(0xd7dbe3)).track_focus(&self.bar_focus).key_context("CefBar")
                    .on_key_down(cx.listener(|this, e, w, cx| this.key_down(e, w, cx)))
                    .on_click(cx.listener(|this, _, w, cx| { w.focus(&this.bar_focus); this.bar_edit = true; cx.notify(); }))
                    .child(format!("{}{}", if self.bar_text.is_empty() { "Search or enter address".to_string() } else { self.bar_text.clone() }, if bar_focused { "▏" } else { "" })))
                .child(Self::bar_button("cef-new", "+", true).on_click(cx.listener(|this, _, w, cx| { this.new_tab("about:blank", cx); w.focus(&this.bar_focus); this.bar_edit = true; this.bar_text.clear(); })))
                .when(self.taken, |d| d.child(Self::bar_button("cef-back-to-agent", "Hand back", true).on_click(cx.listener(|this, _, _, cx| this.hand_back(cx))))))
            .children(tabs_row)
            .child(div().id("cef-view").flex_1().relative().overflow_hidden().cursor(cursor_of(cursor)).track_focus(&focus).key_context("CefPage")
                .child(canvas(move |b, window, _| {
                    *bounds.lock().unwrap() = Some(b);
                    if let Some(t) = &tab_for_size { let (w, h) = (f32::from(b.size.width) as u32, f32::from(b.size.height) as u32); if t.size() != (w.max(1), h.max(1)) { t.resize(w, h, window.scale_factor()); } }
                    b
                }, move |b, _, window, cx| { window.handle_input(&focus, ElementInputHandler::new(b, me), cx); }).absolute().size_full())
                .children(self.image.clone().map(|i| img(i).absolute().size_full().object_fit(ObjectFit::Fill)))
                .when(!err.is_empty(), |d| d.child(div().absolute().top_2().left_2().px_2().py_1().text_size(px(12.)).bg(rgb(0x1a1e25)).text_color(rgb(0xf06a6a)).child(err)))
                .when(self.image.is_none() && !title.is_empty(), |d| d.child(div().absolute().top_2().left_2().text_color(rgb(0x8a93a3)).child(title)))
                .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, _| { let (x, y) = this.local(e.position); this.last_mouse = (x, y); if let Some(t) = this.tab() { t.mouse_move(x, y, Self::mods(&e.modifiers, this.buttons), false); } }))
                .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, w, cx| this.mouse(e, 0, w, cx)))
                .on_mouse_down(MouseButton::Middle, cx.listener(|this, e: &MouseDownEvent, w, cx| this.mouse(e, 1, w, cx)))
                .on_mouse_down(MouseButton::Right, cx.listener(|this, e: &MouseDownEvent, w, cx| this.mouse(e, 2, w, cx)))
                .on_mouse_up(MouseButton::Left, cx.listener(|this, e: &MouseUpEvent, _, _| this.mouse_up(e, 0)))
                .on_mouse_up(MouseButton::Middle, cx.listener(|this, e: &MouseUpEvent, _, _| this.mouse_up(e, 1)))
                .on_mouse_up(MouseButton::Right, cx.listener(|this, e: &MouseUpEvent, _, _| this.mouse_up(e, 2)))
                .on_mouse_up_out(MouseButton::Left, cx.listener(|this, e: &MouseUpEvent, _, _| this.mouse_up(e, 0)))
                .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, _| {
                    let (x, y) = this.local(e.position);
                    let (dx, dy) = match e.delta { ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y)), ScrollDelta::Lines(l) => (l.x * 40.0, l.y * 40.0) };
                    if let Some(t) = this.tab() { t.wheel(x, y, Self::mods(&e.modifiers, 0), dx as i32, dy as i32); }
                }))
                .on_key_down(cx.listener(|this, e, w, cx| this.key_down(e, w, cx)))
                .on_key_up(cx.listener(|this, e, w, _| this.key_up(e, w))))
    }
}
impl CefPage {
    /// Scripted input for the e2e (`{"cef":{...}}` steps): goto, click at view-local logical px, type, handback, report. Answers the JSON the metric carries.
    pub fn script(&mut self, v: &serde_json::Value, cx: &mut Context<Self>) -> serde_json::Value {
        use serde_json::json;
        let Some(t) = self.tab().cloned() else { return json!({"ok": false, "error": "no tab"}) };
        if let Some(u) = v["goto"].as_str() { t.navigate(u); return json!({"ok": true, "goto": u}) }
        if let Some(a) = v["click"].as_array() {
            let (x, y) = (a.first().and_then(|n| n.as_f64()).unwrap_or(0.) as i32, a.get(1).and_then(|n| n.as_f64()).unwrap_or(0.) as i32);
            self.take_over(); t.focus(true); t.mouse_move(x, y, 0, false); t.mouse_click(x, y, 16, 0, false, 1); t.mouse_click(x, y, 0, 0, true, 1); return json!({"ok": true, "click": [x, y]})
        }
        if let Some(text) = v["type"].as_str() { self.take_over(); for c in text.chars() { if let Some(vk) = cef::vk(&c.to_string()) { t.key(0, 0, vk, vk, vk as u16); } let mut b = [0u16; 2]; for u in c.encode_utf16(&mut b) { t.key(2, 0, 0, 0, *u); } } return json!({"ok": true, "typed": text}) }
        if v["handback"].as_bool() == Some(true) { self.hand_back(cx); return json!({"ok": true}) }
        let (w, h, lit) = self.stats;
        let s = t.st.lock().unwrap();
        json!({"ok": true, "url": s.url, "title": s.title, "loading": s.loading, "frame": [w, h], "non_white_samples": lit, "taken": self.taken, "view_x": self.bounds.lock().unwrap().map_or(0., |b| f32::from(b.origin.x)), "view_y": self.bounds.lock().unwrap().map_or(0., |b| f32::from(b.origin.y)), "tabs": self.tabs.len(), "cdp_port": self.port})
    }
    fn mouse(&mut self, e: &MouseDownEvent, b: u8, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus); self.bar_edit = false; self.take_over();
        self.buttons |= 1 << (4 + b); let (x, y) = self.local(e.position);
        if let Some(t) = self.tab() { t.focus(true); t.mouse_click(x, y, Self::mods(&e.modifiers, self.buttons), b, false, e.click_count as i32); }
        cx.notify();
    }
    fn mouse_up(&mut self, e: &MouseUpEvent, b: u8) {
        let (x, y) = self.local(e.position); let held = self.buttons; self.buttons &= !(1 << (4 + b));
        if let Some(t) = self.tab() { t.mouse_click(x, y, Self::mods(&e.modifiers, self.buttons), b, true, e.click_count as i32); }
        let _ = held;
    }
}
impl EntityInputHandler for CefPage {
    fn text_for_range(&mut self, _: Range<usize>, _: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> { None }
    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> { Some(UTF16Selection { range: 0..0, reversed: false }) }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> { self.composing.then_some(0..1) }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) { if self.composing { self.composing = false; if let Some(t) = self.tab() { t.ime_cancel(); } } }
    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.composing = false;
        if self.bar_focus.is_focused(window) { self.bar_text.push_str(text); self.bar_edit = true; cx.notify(); return }
        if let Some(t) = self.tab() { t.ime_commit(text); }
    }
    fn replace_and_mark_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: Option<Range<usize>>, window: &mut Window, _: &mut Context<Self>) {
        if self.bar_focus.is_focused(window) { return }
        self.composing = !text.is_empty(); if let Some(t) = self.tab() { t.ime_compose(text); }
    }
    fn bounds_for_range(&mut self, _: Range<usize>, element_bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        Some(Bounds::new(element_bounds.origin + point(px(self.last_mouse.0 as f32), px(self.last_mouse.1 as f32)), size(px(1.), px(18.))))
    }
    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> { None }
}
pub fn engine_port() -> Result<u16, String> { match Engine::get() { Ok(e) => Ok(e.cdp_port), Err(e) => Err(e.clone()) } }

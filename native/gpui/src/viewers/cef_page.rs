//! The Browser tab on embedded Chromium (feature `cef`): toolbar (back, forward, reload, address, new tab, hand back), tab strip, and the page as one
//! gpui image repainted from CEF's OSR frames, with the person's mouse, wheel, keys, IME text and focus going to the page. The agent drives the same page
//! over CDP: the engine's loopback port is handed to the hub (`bots.browser` op `attach`), and the person's first click takes the page over
//! (`takeover`; "Hand back" gives it back), the semantics of docs/browser-native.md.
use super::cef::{self, Cur, Engine, ImplFileDialogCallback as _, Tab};
use crate::{hub::Command, screens::settings::Secret};
use gpui::{prelude::*, *};
use std::{ops::Range, sync::{Arc, Mutex}};

pub struct CefPage {
    tabs: Vec<Arc<Tab>>, active: usize, focus: FocusHandle, bar_focus: FocusHandle, bar_text: String, bar_edit: bool,
    bounds: Arc<Mutex<Option<Bounds<Pixels>>>>, image: Option<Arc<RenderImage>>, retired: Vec<Arc<RenderImage>>,
    wake: async_channel::Sender<()>, hub: tokio::sync::mpsc::Sender<Command>, owner: String, port: u16,
    pub taken: bool, pub light: bool, fresh: bool, sample: [u8; 3], lat_probe: Option<(std::time::Instant, [u8; 3])>, lat_ms: f64, stats: (u32, u32, usize), buttons: u32, visible: bool, composing: bool, attached: Vec<String>, pub status: String, last_mouse: (i32, i32),
}

impl CefPage {
    pub fn new(cx: &mut Context<Self>, owner: String, hub: tokio::sync::mpsc::Sender<Command>, port: u16) -> Self {
        if cef::EXTERNAL_PUMP { drive_pump(cx); }
        let (wake, rx) = async_channel::bounded::<()>(1);
        cx.spawn(async move |weak, cx| {
            while rx.recv().await.is_ok() {
                if weak.update(cx, |this, cx| this.pump(cx)).is_err() { break }
                cx.background_executor().timer(std::time::Duration::from_millis(16)).await; // at most ~60 repaints a second
            }
        }).detach();
        let mut me = CefPage { tabs: Vec::new(), active: 0, focus: cx.focus_handle(), bar_focus: cx.focus_handle(), bar_text: String::new(), bar_edit: false, bounds: Arc::default(), image: None, retired: Vec::new(), wake, hub, owner, port, taken: false, light: false, fresh: false, sample: [0; 3], lat_probe: None, lat_ms: -1.0, buttons: 0, visible: false, composing: false, attached: Vec::new(), status: String::new(), stats: (0, 0, 0), last_mouse: (0, 0) };
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
    /// OSR has no native dialog: gpui's path picker answers CEF's file dialog.
    fn file_dialog(&mut self, d: cef::Dialog, cx: &mut Context<Self>) {
        // test hook (no portal on a bare Xvfb): answer with this file instead of asking the person
        if let Some(f) = std::env::var_os("BACKPLANE_CEF_TEST_FILE").filter(|f| !f.is_empty()) { let mut l = cef::CefStringList::new(); l.append(&f.to_string_lossy()); d.cb.cont(Some(&mut l)); return }
        let rx: std::pin::Pin<Box<dyn std::future::Future<Output = Option<Vec<std::path::PathBuf>>>>> = if d.save {
            let r = cx.prompt_for_new_path(&crate::plat::user_home().unwrap_or_default(), None);
            Box::pin(async move { r.await.ok().and_then(|r| r.ok()).flatten().map(|p| vec![p]) })
        } else {
            let r = cx.prompt_for_paths(PathPromptOptions { files: !d.folder, directories: d.folder, multiple: d.multiple, prompt: None });
            Box::pin(async move { r.await.ok().and_then(|r| r.ok()).flatten() })
        };
        let cb = d.cb;
        cx.spawn(async move |_, _| {
            let paths = rx.await;
            match paths { Some(p) if !p.is_empty() => { let mut l = cef::CefStringList::new(); for x in p { l.append(&x.to_string_lossy()); } cb.cont(Some(&mut l)); } _ => cb.cancel() }
        }).detach();
    }
    /// Frames, popups, closed tabs, address changes: everything CEF's callbacks left for the main thread.
    fn pump(&mut self, cx: &mut Context<Self>) {
        let (mut popups, mut dialogs) = (Vec::new(), Vec::new());
        for t in &self.tabs { let mut st = t.st.lock().unwrap(); popups.append(&mut st.popup_tabs); dialogs.append(&mut st.dialogs); if !st.download.is_empty() { self.status = st.download.clone(); } }
        for t in popups { self.tabs.push(t); self.active = self.tabs.len() - 1; self.image = None; self.bar_edit = false; }
        for d in dialogs { self.file_dialog(d, cx); }
        let before = self.tabs.len();
        let act = self.tabs.get(self.active).cloned();
        self.tabs.retain(|t| !t.st.lock().unwrap().closed);
        if self.tabs.len() != before { self.active = act.and_then(|a| self.tabs.iter().position(|t| Arc::ptr_eq(t, &a))).unwrap_or(self.tabs.len().saturating_sub(1)); self.image = None; }
        if self.tabs.is_empty() { self.new_tab("about:blank", cx); }
        if let Some(t) = self.tab().cloned() {
            if let Some(f) = t.take_frame() {
                // the probe pixel (page x,y 350,350, a square the bench page flips on click) and a resize probe: how long from the input to a painted frame that shows it
                let px = |x: u32, y: u32| -> [u8; 3] { let i = ((y.min(f.h.saturating_sub(1)) * f.w + x.min(f.w.saturating_sub(1))) * 4) as usize; f.bgra.get(i..i + 3).map_or([0; 3], |p| [p[0], p[1], p[2]]) };
                self.sample = px(350, 350);
                if let Some((t0, old)) = self.lat_probe { if self.sample != old { self.lat_ms = t0.elapsed().as_secs_f64() * 1e3; self.lat_probe = None; } }
                self.stats = (f.w, f.h, f.bgra.chunks(4).step_by(97).filter(|p| p[0] < 240 || p[1] < 240 || p[2] < 240).count()); if let Some(buf) = image::RgbaImage::from_raw(f.w, f.h, f.bgra) { // CEF paints BGRA, which is what gpui's RenderImage holds
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
    fn mods(m: &Modifiers, buttons: u32) -> u32 { cef::flags(m.shift, m.control, m.alt, false, false, false) | if m.platform { cef::COMMAND } else { 0 } | buttons }
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
            // the address is selected when the field is entered: the first thing typed replaces it
            if self.fresh && !matches!(k.key.as_str(), "enter" | "escape" | "left" | "right" | "tab") && !(m.control && k.key != "v" && k.key != "a") { self.bar_text.clear(); self.fresh = false; }
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
        if k.key == "f12" || ((m.control || m.platform) && m.shift && k.key == "i") { t.devtools(); cx.stop_propagation(); return }
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
    fn act(&mut self, a: super::chrome::Act, window: &mut Window, cx: &mut Context<Self>) {
        use super::chrome::Act::*;
        match a {
            Back => { if let Some(t) = self.tab() { t.back(); } } Forward => { if let Some(t) = self.tab() { t.forward(); } }
            Reload => { if let Some(t) = self.tab() { if t.st.lock().unwrap().loading { t.stop() } else { t.reload() } } }
            NewTab => { self.new_tab("about:blank", cx); window.focus(&self.bar_focus); self.bar_edit = true; self.bar_text.clear(); }
            Select(i) => { if let Ok(i) = i.parse::<usize>() { if i < self.tabs.len() { self.active = i; self.image = None; if let Some(t) = self.tab() { t.hidden(false); t.resize(1, 1, 1.0); } } } cx.notify(); }
            Close(i) => { if let Some(t) = i.parse::<usize>().ok().and_then(|i| self.tabs.get(i)) { t.close(); } }
            FocusBar => { window.focus(&self.bar_focus); self.bar_edit = true; self.bar_text = self.tab().map(|t| t.st.lock().unwrap().url.clone()).unwrap_or_default(); self.fresh = true; cx.notify(); }
            HandBack => self.hand_back(cx),
        }
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
        let bar_focused = self.bar_focus.is_focused(window); let status = self.status.clone();
        let bounds = self.bounds.clone(); let tab_for_size = self.tab().cloned();
        let focus = self.focus.clone();
        let tabs: Vec<super::chrome::TabView> = self.tabs.iter().enumerate().map(|(i, t)| { let st = t.st.lock().unwrap(); super::chrome::TabView { id: i.to_string(), title: st.title.clone(), url: st.url.clone(), active: i == self.active } }).collect();
        let cur_url = self.tab().map(|t| t.st.lock().unwrap().url.clone()).unwrap_or_default();
        let value = if bar_focused && self.bar_edit { self.bar_text.clone() } else if cur_url == "about:blank" { String::new() } else { cur_url };
        let (strip, chrome) = super::chrome::render(super::chrome::In { light: self.light, tabs: &tabs, value, focused: bar_focused, taken: self.taken, have: !self.tabs.is_empty(), loading, can_back: back, can_forward: fwd, bar_focus: &self.bar_focus }, cx,
            |this: &mut CefPage, a, w, cx| this.act(a, w, cx), |this, e, w, cx| this.key_down(e, w, cx));
        let c = |n: &str| crate::theme::color(self.light, n);
        div().id("cef-page").size_full().flex().flex_col().bg(c("bg")).child(strip).child(chrome)
            .child(div().id("cef-view").flex_1().relative().overflow_hidden().cursor(cursor_of(cursor)).track_focus(&focus).key_context("CefPage")
                .child(canvas(move |b, window, _| {
                    *bounds.lock().unwrap() = Some(b);
                    if let Some(t) = &tab_for_size { let (w, h) = (f32::from(b.size.width) as u32, f32::from(b.size.height) as u32); if t.size() != (w.max(1), h.max(1)) { t.resize(w, h, window.scale_factor()); } }
                    b
                }, move |b, _, window, cx| { window.handle_input(&focus, ElementInputHandler::new(b, me), cx); }).absolute().size_full())
                .children(self.image.clone().map(|i| img(i).absolute().size_full().object_fit(ObjectFit::Fill)))
                .when(!err.is_empty(), |d| d.child(div().absolute().top_2().left_2().px_2().py_1().text_size(px(12.)).bg(rgb(0x1a1e25)).text_color(rgb(0xf06a6a)).child(err)))
                .when(!status.is_empty(), |d| d.child(div().absolute().bottom_2().left_2().px_2().py_1().text_size(px(12.)).bg(rgb(0x1a1e25)).text_color(rgb(0xd7dbe3)).child(status.clone())))
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
                .key_context("Viewer").on_key_down(cx.listener(|this, e, w, cx| this.key_down(e, w, cx)))
                .on_key_up(cx.listener(|this, e, w, _| this.key_up(e, w))))
    }
}
impl CefPage {
    /// Scripted input for the e2e (`{"cef":{...}}` steps): goto, click at view-local logical px, type, handback, report. Answers the JSON the metric carries.
    pub fn script(&mut self, v: &serde_json::Value, cx: &mut Context<Self>) -> serde_json::Value {
        use serde_json::json;
        let Some(t) = self.tab().cloned() else { return json!({"ok": false, "error": "no tab"}) };
        if let Some(i) = v["tab"].as_u64() { if (i as usize) < self.tabs.len() { self.active = i as usize; self.image = None; if let Some(t) = self.tab() { t.hidden(false); } } cx.notify(); return json!({"ok": true, "tab": i}) }
        if let Some(u) = v["goto"].as_str() { t.navigate(u); return json!({"ok": true, "goto": u}) }
        if let Some(a) = v["click"].as_array() {
            let (x, y) = (a.first().and_then(|n| n.as_f64()).unwrap_or(0.) as i32, a.get(1).and_then(|n| n.as_f64()).unwrap_or(0.) as i32);
            self.take_over(); t.focus(true); t.mouse_move(x, y, 0, false); t.mouse_click(x, y, 16, 0, false, 1); t.mouse_click(x, y, 0, 0, true, 1); return json!({"ok": true, "click": [x, y]})
        }
        if let Some(text) = v["type"].as_str() { self.take_over(); for c in text.chars() { if let Some(vk) = cef::vk(&c.to_string()) { t.key(0, 0, vk, vk, vk as u16); } let mut b = [0u16; 2]; for u in c.encode_utf16(&mut b) { t.key(2, 0, 0, 0, *u); } } return json!({"ok": true, "typed": text}) }
        if v["latency"].as_bool() == Some(true) { self.lat_ms = -1.0; self.lat_probe = Some((std::time::Instant::now(), self.sample)); t.mouse_move(350, 350, 0, false); t.mouse_click(350, 350, 16, 0, false, 1); t.mouse_click(350, 350, 0, 0, true, 1); return json!({"ok": true, "latency": "probing"}) }
        if v["handback"].as_bool() == Some(true) { self.hand_back(cx); return json!({"ok": true}) }
        let (w, h, lit) = self.stats;
        let s = t.st.lock().unwrap();
        json!({"ok": true, "url": s.url, "title": s.title, "loading": s.loading, "frame": [w, h], "non_white_samples": lit, "taken": self.taken, "paints": t.paints.load(std::sync::atomic::Ordering::Relaxed), "click_to_pixel_ms": self.lat_ms, "resize_to_frame_ms": t.resize_ms(), "view_x": self.bounds.lock().unwrap().map_or(0., |b| f32::from(b.origin.x)), "view_y": self.bounds.lock().unwrap().map_or(0., |b| f32::from(b.origin.y)), "tabs": self.tabs.len(), "cdp_port": self.port, "pumped": cef::pumped()})
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
/// macOS: CEF's work runs on gpui's main thread when CEF asks for it (cef.rs `PUMP`), once for the process, for as long as it runs. A request
/// for later waits for its time unless a sooner one comes; a delay is never longer than 1/30 s (CEF's own clients cap it the same way).
fn drive_pump<T: 'static>(cx: &mut Context<T>) {
    static ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if ON.swap(true, std::sync::atomic::Ordering::SeqCst) { return }
    let rx = cef::pump_requests(); let ex = cx.background_executor().clone();
    cx.spawn(async move |_, _| {
        use futures_util::future::{select, Either};
        let mut due: Option<std::time::Instant> = Some(std::time::Instant::now());
        loop {
            let at = |d: i64| std::time::Instant::now() + std::time::Duration::from_millis(d.clamp(0, 33) as u64);
            match due {
                None => match rx.recv().await { Ok(d) => due = Some(at(d)), Err(_) => return },
                Some(t) => {
                    let wait = t.saturating_duration_since(std::time::Instant::now());
                    if wait.is_zero() { due = None; cef::pump_work(); continue }
                    match select(std::pin::pin!(rx.recv()), std::pin::pin!(ex.timer(wait))).await {
                        Either::Left((Ok(d), _)) => { let n = at(d); if n < t { due = Some(n); } }
                        Either::Left((Err(_), _)) => return,
                        Either::Right(_) => { due = None; cef::pump_work(); }
                    }
                }
            }
        }
    }).detach();
}
pub fn engine_port() -> Result<u16, String> { match Engine::get() { Ok(e) => Ok(e.cdp_port), Err(e) => Err(e.clone()) } }

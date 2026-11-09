//! The Browser tab on embedded Chromium (feature `cef`): toolbar (back, forward, reload, address, new tab, hand back), tab strip, and the page.
//! Windowed (cef.rs): the page is Chromium's own native child window, placed over the pane after every frame gpui draws (`after_frame`: the pane's
//! marker hitbox in the frame just drawn) and hidden while the pane is not drawn or anything drawn later covers it (a popover, a menu, a dialog);
//! the keyboard moves between gpui and Chromium with gpui's focus, and the client's own shortcuts pressed in the page come back to gpui first.
//! Off screen (the fallback): one gpui image repainted from CEF's frames, with the person's mouse, wheel, keys, IME text and focus going to the page. The agent drives the same page
//! over CDP: the engine's loopback port is handed to the hub (`bots.browser` op `attach`), and the person's first click takes the page over
//! (`takeover`; "Hand back" gives it back), the semantics of docs/browser-native.md.
use super::cef::{self, native, Cur, Engine, ImplFileDialogCallback as _, Tab};
use crate::{hub::Command, screens::settings::Secret};
use gpui::{prelude::*, *};
use std::{ops::Range, sync::{Arc, Mutex}};

pub struct CefPage {
    tabs: Vec<Arc<Tab>>, active: usize, focus: FocusHandle, bar_focus: FocusHandle, bar_text: String, bar_edit: bool,
    bounds: Arc<Mutex<Option<Bounds<Pixels>>>>, image: Option<Arc<RenderImage>>, retired: Vec<Arc<RenderImage>>,
    wake: async_channel::Sender<()>, hub: tokio::sync::mpsc::Sender<Command>, owner: String, port: u16,
    pub taken: bool, pub light: bool, fresh: bool, sample: [u8; 3], lat_probe: Option<(std::time::Instant, [u8; 3])>, lat_ms: f64, stats: (u32, u32, usize), buttons: u32, visible: bool, composing: bool, attached: Vec<String>, pub status: String, last_mouse: (i32, i32),
    /// windowed: the client window (None until the first render decides; Some(None): off screen), the pane's marker hitbox in the last frame,
    /// where each tab's child was put (by tab), whether something drawn later covers the pane, gpui keys from the page waiting for a window
    host: Option<Option<native::Host>>, marker: Arc<Mutex<Option<HitboxId>>>, placed: std::collections::HashMap<usize, (Option<native::R>, u64)>, covered: bool,
    from_page: Vec<(String, cef::KeyEvent)>, lat_native: Arc<Mutex<f64>>, want_focus: bool, want_yank: bool, dispatching: Arc<std::sync::atomic::AtomicBool>, subs: Vec<Subscription>,
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
        let mut me = CefPage { tabs: Vec::new(), active: 0, focus: cx.focus_handle(), bar_focus: cx.focus_handle(), bar_text: String::new(), bar_edit: false, bounds: Arc::default(), image: None, retired: Vec::new(), wake, hub, owner, port, taken: false, light: false, fresh: false, sample: [0; 3], lat_probe: None, lat_ms: -1.0, buttons: 0, visible: false, composing: false, attached: Vec::new(), status: String::new(), stats: (0, 0, 0), last_mouse: (0, 0),
            host: None, marker: Arc::default(), placed: Default::default(), covered: false, from_page: Vec::new(), lat_native: Arc::new(Mutex::new(-1.0)), want_focus: false, want_yank: false, dispatching: Arc::default(), subs: Vec::new() };
        me
    }
    fn windowed(&self) -> bool { matches!(self.host, Some(Some(_))) }
    /// First render: windowed when the client's window can be Chromium's parent and BACKPLANE_CEF_OSR is not set, else off screen; said in the log.
    fn decide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use raw_window_handle::HasWindowHandle;
        let raw = HasWindowHandle::window_handle(window).ok().map(|h| h.as_raw());
        let host = raw.and_then(native::Host::of);
        let why = if std::env::var_os("BACKPLANE_CEF_OSR").is_some_and(|v| !v.is_empty() && v != "0") { "BACKPLANE_CEF_OSR is set" } else if host.is_none() { "the window cannot parent a native child (Wayland?)" } else { "" }; // i18n-ok: the log says why (never on screen)
        let host = if why.is_empty() { host } else { None };
        eprintln!("GPUI_LOG {{\"cef_mode\":\"{}\",\"why\":{},\"host\":\"{:?}\"}}", if host.is_some() { "windowed" } else { "offscreen" }, serde_json::Value::String(why.into()), host);
        self.host = Some(host);
        if host.is_some() {
            let weak = cx.entity().downgrade();
            window.on_frame_drawn(move |window, cx| weak.update(cx, |this, cx| this.after_frame(window, cx)).is_ok());
            // the keyboard follows gpui's focus: onto the page, Chromium's window has it; off the page (only while this window is the active one,
            // so another program's never is taken), the client's window has it again
            self.subs.push(cx.on_focus(&self.focus, window, |this, _, _| { if let Some(t) = this.tab() { t.give_keyboard(true); } }));
            self.subs.push(cx.on_blur(&self.focus, window, |this, window, _| { if window.is_window_active() { if let Some(t) = this.tab() { t.give_keyboard(false); } } }));
        }
    }
    /// windowed, after every frame gpui drew: each tab's child over the pane (the active one, when the pane was drawn and nothing drawn later covers
    /// it) or hidden (the rest). A sliver under 12 px (a splitter's grip at the pane's edge) does not count as covering.
    fn after_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let seen = self.marker.lock().unwrap().and_then(|id| window.hitbox_drawn_after(id));
        let over = seen.as_ref().map_or(false, |(_, o)| o.iter().any(|b| f32::from(b.size.width) >= 12. && f32::from(b.size.height) >= 12.));
        if over != self.covered { self.covered = over; cx.notify(); }
        let scale = if cfg!(target_os = "macos") { 1.0 } else { window.scale_factor() };
        let at = seen.filter(|_| !over).map(|(b, _)| native::R { x: (f32::from(b.origin.x) * scale).round() as i32, y: (f32::from(b.origin.y) * scale).round() as i32,
            w: (f32::from(b.size.width) * scale).round() as i32, h: (f32::from(b.size.height) * scale).round() as i32 }).filter(|r| r.w > 0 && r.h > 0);
        for (i, t) in self.tabs.iter().enumerate() {
            if t.outer() == 0 { continue }
            let want = if i == self.active { at } else { None };
            let key = Arc::as_ptr(t) as usize;
            // (again once Chromium's own window exists: the first placement may have found only the container)
            if self.placed.get(&key) != Some(&(want, t.native())) { t.place(want); self.placed.insert(key, (want, t.native())); }
        }
        self.placed.retain(|k, _| self.tabs.iter().any(|t| Arc::as_ptr(t) as usize == *k));
    }
    fn tab(&self) -> Option<&Arc<Tab>> { self.tabs.get(self.active) }
    fn new_tab(&mut self, url: &str, cx: &mut Context<Self>) {
        let (w, h) = self.bounds.lock().unwrap().map(|b| (f32::from(b.size.width) as u32, f32::from(b.size.height) as u32)).unwrap_or((900, 600));
        let t = Tab::open(url, w, h, 1.0, self.wake.clone(), self.host.flatten());
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
        let (mut popups, mut dialogs, mut person) = (Vec::new(), Vec::new(), false);
        for t in &self.tabs { let mut st = t.st.lock().unwrap(); popups.append(&mut st.popup_tabs); dialogs.append(&mut st.dialogs); if !st.download.is_empty() { self.status = st.download.clone(); }
            // Chromium took the keyboard: on the person's click the page takes gpui's focus too; by itself (a page shown, a script's focus())
            // it gives the keyboard back unless the page already has gpui's focus
            let got = std::mem::take(&mut st.got_focus);
            if std::mem::take(&mut st.person) { person = true; self.want_focus = true; } else if got { self.want_yank = true; }
            self.from_page.append(&mut st.keys);
            if std::mem::take(&mut st.took_focus) { self.want_focus = false; } }
        for t in popups { self.tabs.push(t); self.active = self.tabs.len() - 1; self.image = None; self.bar_edit = false; }
        for d in dialogs { self.file_dialog(d, cx); }
        if person { self.take_over(); }
        let before = self.tabs.len();
        let act = self.tabs.get(self.active).cloned();
        self.tabs.retain(|t| !t.st.lock().unwrap().closed);
        if self.tabs.len() != before { self.active = act.and_then(|a| self.tabs.iter().position(|t| Arc::ptr_eq(t, &a))).unwrap_or(self.tabs.len().saturating_sub(1)); self.image = None; }
        if self.tabs.is_empty() && self.host.is_some() { self.new_tab("about:blank", cx); }
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
    /// windowed: the keyboard goes back to the client too, so the person's next click into the page is seen as theirs (on_set_focus)
    fn hand_back(&mut self, cx: &mut Context<Self>) { if self.taken { self.taken = false; self.rpc(serde_json::json!({"bot": self.owner, "op": "handback"})); if self.windowed() { if let Some(t) = self.tab() { t.give_keyboard(false); } } cx.notify(); } }
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
        if self.windowed() && (m.control || m.platform) && !m.alt && !m.shift && k.key == "l" { window.focus(&self.bar_focus); self.bar_edit = true; cx.stop_propagation(); cx.notify(); return }
        // windowed: a chord the page passed to the client: the client's bindings have it (and the page gets it back when none does)
        if self.windowed() && self.dispatching.load(std::sync::atomic::Ordering::Relaxed) { return }
        // windowed with the keyboard on the client's window while the page has gpui's focus: the page takes the keyboard, with this key
        if self.windowed() { t.give_keyboard(true); }
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

/// Windows: the windows under the client's (the report's diagnosis of a windowed page); elsewhere none.
fn hwnds(t: &Tab) -> Vec<String> { #[cfg(windows)] { t.host.map(native::tree).unwrap_or_default() } #[cfg(not(windows))] { let _ = t; Vec::new() } }
fn cursor_of(c: Cur) -> CursorStyle {
    match c { Cur::Hand => CursorStyle::PointingHand, Cur::IBeam => CursorStyle::IBeam, Cur::Cross => CursorStyle::Crosshair, Cur::Move => CursorStyle::ClosedHand, Cur::ResizeH => CursorStyle::ResizeLeftRight, Cur::ResizeV => CursorStyle::ResizeUpDown,
        Cur::ResizeNwse => CursorStyle::ResizeUpLeftDownRight, Cur::ResizeNesw => CursorStyle::ResizeUpRightDownLeft, Cur::NotAllowed => CursorStyle::OperationNotAllowed, _ => CursorStyle::Arrow }
}

impl Render for CefPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.retired.drain(..) { let _ = window.drop_image(old); }
        if self.host.is_none() { self.decide(window, cx); }
        if self.tabs.is_empty() { self.new_tab("about:blank", cx); }
        if std::mem::take(&mut self.want_focus) && !self.focus.is_focused(window) { window.focus(&self.focus); self.bar_edit = false; }
        if std::mem::take(&mut self.want_yank) && !self.focus.is_focused(window) && window.is_window_active() { if let Some(t) = self.tab() { t.give_keyboard(false); } }
        if !self.from_page.is_empty() {
            let keys = std::mem::take(&mut self.from_page); let flag = self.dispatching.clone(); let tab = self.tab().cloned();
            window.defer(cx, move |window, cx| for (ks, e) in keys {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
                let done = Keystroke::parse(&ks).map_or(false, |k| window.dispatch_keystroke(k, cx));
                flag.store(false, std::sync::atomic::Ordering::Relaxed);
                eprintln!("GPUI_LOG {{\"cef_key\":{},\"client_took\":{done}}}", serde_json::Value::String(ks));
                if !done { if let Some(t) = &tab { t.reinject(&e); } }
            });
        }
        let windowed = self.windowed(); let covered = self.covered; let marker = self.marker.clone();
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
        // windowed: nothing gpui draws shows over Chromium's window, so the load error and the download line go at the end of the tab strip
        // (never in a row of their own: the page would move whenever one appeared)
        let strip = if windowed && (!err.is_empty() || !status.is_empty()) {
            div().flex().flex_row().items_center().bg(c("panel")).child(div().flex_1().min_w_0().child(strip))
                .child(div().flex_none().max_w(px(360.)).overflow_hidden().whitespace_nowrap().px_2().text_size(px(12.))
                    .text_color(if err.is_empty() { rgb(0xd7dbe3) } else { rgb(0xf06a6a) }).child(crate::i18n::trs(if err.is_empty() { &status } else { &err }))).into_any_element()
        } else { strip };
        div().id("cef-page").size_full().flex().flex_col().bg(c("bg")).child(strip).child(chrome)

            .child(div().id("cef-view").flex_1().relative().overflow_hidden().cursor(cursor_of(cursor)).track_focus(&focus).key_context("CefPage")
                .child(canvas(move |b, window, _| {
                    *bounds.lock().unwrap() = Some(b);
                    // windowed: where the pane is in this frame (after_frame puts Chromium's window there); a hitbox, so a cached replay keeps it
                    if windowed { *marker.lock().unwrap() = Some(window.insert_hitbox(b, HitboxBehavior::Normal).id); }
                    if let Some(t) = &tab_for_size { let (w, h) = (f32::from(b.size.width) as u32, f32::from(b.size.height) as u32); if t.size() != (w.max(1), h.max(1)) { t.resize(w, h, window.scale_factor()); } }
                    b
                }, move |b, _, window, cx| { window.handle_input(&focus, ElementInputHandler::new(b, me), cx); }).absolute().size_full())
                .children(self.image.clone().map(|i| img(i).absolute().size_full().object_fit(ObjectFit::Fill)))
                .when(windowed && covered && !title.is_empty(), |d| d.child(div().absolute().top_2().left_2().text_color(rgb(0x8a93a3)).child(title.clone())))
                .when(!windowed && !err.is_empty(), |d| d.child(div().absolute().top_2().left_2().px_2().py_1().text_size(px(12.)).bg(rgb(0x1a1e25)).text_color(rgb(0xf06a6a)).child(crate::i18n::trs(&err))))
                .when(!windowed && !status.is_empty(), |d| d.child(div().absolute().bottom_2().left_2().px_2().py_1().text_size(px(12.)).bg(rgb(0x1a1e25)).text_color(rgb(0xd7dbe3)).child(crate::i18n::trs(&status))))
                .when(!windowed && self.image.is_none() && !title.is_empty(), |d| d.child(div().absolute().top_2().left_2().text_color(rgb(0x8a93a3)).child(title)))
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
        let Some(t) = self.tab().cloned() else { return json!({"ok": false, "error": "no tab"}) }; // i18n-ok: an e2e answer
        if let Some(i) = v["tab"].as_u64() { if (i as usize) < self.tabs.len() { self.active = i as usize; self.image = None; if let Some(t) = self.tab() { t.hidden(false); } } cx.notify(); return json!({"ok": true, "tab": i}) }
        if let Some(u) = v["goto"].as_str() { t.navigate(u); return json!({"ok": true, "goto": u}) }
        if v["back"].as_bool() == Some(true) { t.back(); return json!({"ok": true, "back": true}) }
        if v["forward"].as_bool() == Some(true) { t.forward(); return json!({"ok": true, "forward": true}) }
        if let Some(a) = v["click"].as_array() {
            let (x, y) = (a.first().and_then(|n| n.as_f64()).unwrap_or(0.) as i32, a.get(1).and_then(|n| n.as_f64()).unwrap_or(0.) as i32);
            self.take_over(); t.focus(true); t.mouse_move(x, y, 0, false); t.mouse_click(x, y, 16, 0, false, 1); t.mouse_click(x, y, 0, 0, true, 1); return json!({"ok": true, "click": [x, y]})
        }
        if let Some(text) = v["type"].as_str() { self.take_over(); for c in text.chars() { if let Some(vk) = cef::vk(&c.to_string()) { t.key(0, 0, vk, vk, vk as u16); } let mut b = [0u16; 2]; for u in c.encode_utf16(&mut b) { t.key(2, 0, 0, 0, *u); } } return json!({"ok": true, "typed": text}) }
        if v["latency"].as_bool() == Some(true) && t.windowed() { t.probe_click(350, 350, self.lat_native.clone()); return json!({"ok": true, "latency": "probing"}) }
        if v["latency"].as_bool() == Some(true) { self.lat_ms = -1.0; self.lat_probe = Some((std::time::Instant::now(), self.sample)); t.mouse_move(350, 350, 0, false); t.mouse_click(350, 350, 16, 0, false, 1); t.mouse_click(350, 350, 0, 0, true, 1); return json!({"ok": true, "latency": "probing"}) }
        if v["handback"].as_bool() == Some(true) { self.hand_back(cx); return json!({"ok": true}) }
        // windowed: what Chromium's own window shows (its size, and its pixels read back from the window system)
        // (Windows and macOS: a GPU-composited child's pixels cannot be read back here, -1; the e2e checks the screen instead)
        let (w, h, lit) = if t.windowed() { native::geometry(t.native()).map_or((0, 0, 0i64), |(r, _)| (r.w as u32, r.h as u32, if cfg!(target_os = "linux") { native::painted(t.native()) as i64 } else { -1 })) } else { let (w, h, l) = self.stats; (w, h, l as i64) };
        let s = t.st.lock().unwrap();
        json!({"ok": true, "url": s.url, "title": s.title, "loading": s.loading, "frame": [w, h], "non_white_samples": lit, "taken": self.taken, "paints": t.paints.load(std::sync::atomic::Ordering::Relaxed), "click_to_pixel_ms": if t.windowed() { *self.lat_native.lock().unwrap() } else { self.lat_ms }, "resize_to_frame_ms": t.resize_ms(), "view_x": self.bounds.lock().unwrap().map_or(0., |b| f32::from(b.origin.x)), "view_y": self.bounds.lock().unwrap().map_or(0., |b| f32::from(b.origin.y)), "tabs": self.tabs.len(), "cdp_port": self.port, "pumped": cef::pumped(), "windowed": self.windowed(), "covered": self.covered, "active": self.active,
            "hwnds": hwnds(&t),
            "natives": self.tabs.iter().map(|t| native::geometry(t.outer()).map_or(serde_json::Value::Null, |(r, on)| json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h, "shown": on}))).collect::<Vec<_>>(),
            "native": if t.windowed() { native::geometry(t.outer()).map_or(serde_json::Value::Null, |(r, on)| json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h, "shown": on, "painted": native::painted(t.native())})) } else { serde_json::Value::Null }})
    }
    /// The `{"browser":{"op":...}}` step on embedded Chromium (BrowserPage::script names the ops); answers the same report as the frame page.
    pub fn browser_op(&mut self, v: &serde_json::Value, cx: &mut Context<Self>) -> serde_json::Value {
        use serde_json::json;
        let op = v["op"].as_str().unwrap_or("report");
        let r = match (op, self.tab().cloned()) {
            ("new_tab", _) => { self.new_tab(v["url"].as_str().unwrap_or("about:blank"), cx); json!({"ok": true}) }
            ("close_tab", _) => match self.tabs.get(v["index"].as_u64().unwrap_or(0) as usize) { Some(t) => { t.close(); json!({"ok": true}) } None => json!({"ok": false, "error": "no such tab"}) }, // i18n-ok: an e2e answer
            ("report", _) => json!({"ok": true}),
            /* i18n-ok: an e2e answer */ (_, None) => json!({"ok": false, "error": "no tab"}),
            ("goto", Some(t)) => { self.take_over(); t.navigate(v["url"].as_str().unwrap_or("about:blank")); json!({"ok": true}) }
            ("back", Some(t)) => { t.back(); json!({"ok": true}) }
            ("forward", Some(t)) => { t.forward(); json!({"ok": true}) }
            ("reload", Some(t)) => { t.reload(); json!({"ok": true}) }
            ("type", Some(t)) if t.windowed() => { self.take_over(); let u = t.st.lock().unwrap().url.clone(); cef::cdp(self.port, u, vec![serde_json::json!({"method": "Input.insertText", "params": {"text": v["text"].as_str().unwrap_or("")}})]); json!({"ok": true}) } // i18n-ok: CDP method names (test ops)
            ("press", Some(t)) if t.windowed() => { self.take_over(); let u = t.st.lock().unwrap().url.clone(); let k = v["key"].as_str().unwrap_or("Enter").to_string(); // i18n-ok: a CDP key name
                let vk = cef::vk(&k.to_lowercase()).unwrap_or(0); let text = if k == "Enter" { "\r" } else { "" };
                cef::cdp(self.port, u, vec![json!({"method": "Input.dispatchKeyEvent", "params": {"type": "keyDown", "key": k, "windowsVirtualKeyCode": vk, "text": text}}), json!({"method": "Input.dispatchKeyEvent", "params": {"type": "keyUp", "key": k, "windowsVirtualKeyCode": vk}})]); json!({"ok": true}) } // i18n-ok: CDP method names (test ops)
            ("type", Some(t)) => { self.take_over(); t.focus(true); for c in v["text"].as_str().unwrap_or("").chars() { if let Some(vk) = cef::vk(&c.to_string()) { t.key(0, 0, vk, vk, vk as u16); } let mut b = [0u16; 2]; for u in c.encode_utf16(&mut b) { t.key(2, 0, 0, 0, *u); } if let Some(vk) = cef::vk(&c.to_string()) { t.key(1, 0, vk, vk, vk as u16); } } json!({"ok": true}) }
            /* i18n-ok: key names and an e2e answer */ ("press", Some(t)) => { self.take_over(); let k = match v["key"].as_str().unwrap_or("Enter") { "Enter" => "enter", "Backspace" => "backspace", "Tab" => "tab", "Escape" => "escape", k => k }; match cef::vk(k) { Some(vk) => { t.key(0, 0, vk, vk, vk as u16); if vk == 13 { t.key(2, 0, 13, 13, 13); } t.key(1, 0, vk, vk, vk as u16); json!({"ok": true}) } None => json!({"ok": false, "error": "no key code"}) } }
            ("scroll", Some(t)) if t.windowed() => { self.take_over(); let u = t.st.lock().unwrap().url.clone(); let (w, h) = t.size();
                cef::cdp(self.port, u, vec![json!({"method": "Input.dispatchMouseEvent", "params": {"type": "mouseWheel", "x": w / 2, "y": h / 2, "deltaX": 0, "deltaY": v["dy"].as_f64().unwrap_or(400.0)}})]); json!({"ok": true}) } // i18n-ok: a CDP method name (test op)
            ("scroll", Some(t)) => { self.take_over(); let (w, h) = t.size(); t.mouse_move(w as i32 / 2, h as i32 / 2, 0, false); t.wheel(w as i32 / 2, h as i32 / 2, 0, 0, -(v["dy"].as_f64().unwrap_or(400.0) as i32)); json!({"ok": true}) }
            ("click", Some(t)) => { let (w, h) = t.size(); let (x, y) = ((v["x"].as_u64().unwrap_or(500) as u32 * w / 1000) as i32, (v["y"].as_u64().unwrap_or(500) as u32 * h / 1000) as i32);
                self.take_over(); t.focus(true); t.mouse_move(x, y, 0, false); t.mouse_click(x, y, 16, 0, false, 1); t.mouse_click(x, y, 0, 0, true, 1); json!({"ok": true, "at": [x, y]}) }
            /* i18n-ok: an e2e answer */ (o, _) => json!({"ok": false, "error": format!("unknown op {o}")}),
        };
        let mut r = r; r["op"] = op.into();
        let (w, h, lit) = match self.tab() { Some(t) if t.windowed() => native::geometry(t.native()).map_or((0, 0, 0i64), |(r, _)| (r.w as u32, r.h as u32, if cfg!(target_os = "linux") { native::painted(t.native()) as i64 } else { -1 })), _ => { let (w, h, l) = self.stats; (w, h, l as i64) } };
        if let Some(t) = self.tab() { let s = t.st.lock().unwrap(); r["url"] = s.url.clone().into(); r["title"] = s.title.clone().into(); r["loading"] = s.loading.into(); r["load_error"] = s.load_error.clone().into(); }
        r["tabs"] = self.tabs.len().into(); r["frame"] = json!([w, h]); r["non_white_samples"] = lit.into(); r["status"] = self.status.clone().into();
        r
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
/// The page going away (its thread left, the window closing) closes its browsers: CEF's client holds each `Tab`, and the `Tab` holds its browser, so
/// nothing else would, and every renderer the page ran would stay alive until the process quits.
impl Drop for CefPage { fn drop(&mut self) { for t in self.tabs.drain(..) { t.close(); } } }
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
/// macOS: CEF's work runs on gpui's main thread (cef.rs `PUMP`), once for the process, for as long as it runs, the way CEF's own cefclient pumps it:
/// at the time CEF asks for (`on_schedule_message_pump_work`), and otherwise at least every 1/30 s, because CEF does not ask for all of its work
/// (with only its requests the pump ran twice and the page never loaded). This wakes the main thread, it draws nothing: a frame is drawn only
/// when CEF paints one.
fn drive_pump<T: 'static>(cx: &mut Context<T>) {
    static ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if ON.swap(true, std::sync::atomic::Ordering::SeqCst) { return }
    const MAX: std::time::Duration = std::time::Duration::from_millis(33);
    let rx = cef::pump_requests(); let ex = cx.background_executor().clone();
    cx.spawn(async move |_, _| {
        use futures_util::future::{select, Either};
        let mut due = std::time::Instant::now();
        loop {
            let wait = due.saturating_duration_since(std::time::Instant::now());
            if !wait.is_zero() {
                match select(std::pin::pin!(rx.recv()), std::pin::pin!(ex.timer(wait))).await {
                    Either::Left((Ok(d), _)) => { let n = std::time::Instant::now() + std::time::Duration::from_millis(d.max(0) as u64).min(MAX); if n < due { due = n; } continue }
                    Either::Left((Err(_), _)) => return,
                    Either::Right(_) => {}
                }
            }
            cef::pump_work();
            due = std::time::Instant::now() + MAX;
            while let Ok(d) = rx.try_recv() { due = due.min(std::time::Instant::now() + std::time::Duration::from_millis(d.max(0) as u64)); }
        }
    }).detach();
}
pub fn engine_port() -> Result<u16, String> { match Engine::get() { Ok(e) => Ok(e.cdp_port), Err(e) => Err(e.clone()) } }

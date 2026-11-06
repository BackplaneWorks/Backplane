//! The floating terminal: the open thread's shell (the hub's `term.open`, in the thread's worktree or project folder) in a panel over the page.
//! Ctrl+J or a bare backtick shows it and gives it the keyboard; pressed again with the keyboard in it they put it away, and with the keyboard
//! elsewhere they bring the keyboard back to it. Shift+Esc in it puts it away and the message box has the keyboard again; outside it, Shift+Esc
//! gives the keyboard back to it. Ctrl+Shift+C copies what the mouse selected, Ctrl+Shift+V pastes. Every other key is the shell's (keys.rs mutes
//! the client's shortcuts in the `Terminal` context, so Ctrl+C, Ctrl+K, Esc and a backtick reach the program).
//! The screen is drawn by the hub's own emulator (host/src/term/vt.rs, shared by path): the hub sends every byte the shell prints as a `term`
//! frame, and a snapshot (`term.snap`) when this client has no screen for it yet (a shell started before, or a reconnect).
//! Bounded: at most MAX_SCREENS screens are kept (the oldest other one goes), each the emulator's grid and 2048-row scrollback.
use crate::{hub::Command, screens::settings::Secret, theme, vt::{self, Vt}, Backplane};
use gpui::{actions, prelude::*, *};
use serde_json::json;
use std::{cell::Cell, collections::HashMap, rc::Rc};

actions!(term, [Copy, Paste]);

pub const MAX_SCREENS: usize = 8;
/// One term.input request carries at most this many bytes (the hub takes up to 256 KB); a long paste goes as several.
const MAX_INPUT: usize = 65536;
pub const FONT_PX: f32 = 13.;
pub const LINE_PX: f32 = 17.;
const PAD: f32 = 10.;
const HEAD: f32 = 28.;

struct Screen { vt: Vt, pid: i32, used: u64 }
/// A selection by the mouse: (absolute row, column) at each end; rows count from the top of the scrollback.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sel { pub a: (usize, usize), pub b: (usize, usize) }
impl Sel {
    /// The two ends in reading order.
    pub fn span(&self) -> ((usize, usize), (usize, usize)) { if self.a <= self.b { (self.a, self.b) } else { (self.b, self.a) } }
}

pub struct Term {
    pub open: bool,
    /// The thread whose shell is on show ("" before the first).
    pub key: String,
    screens: HashMap<String, Screen>,
    /// The panel's keyboard (None only in tests).
    pub focus: Option<FocusHandle>,
    /// The grid's cols x rows as last told to the hub.
    pub size: (u16, u16),
    cell: (f32, f32),
    grid: Rc<Cell<Bounds<Pixels>>>,
    pub sel: Option<Sel>,
    dragging: bool,
    /// Rows scrolled back into the scrollback (0: the live screen).
    pub scroll: usize,
    tick: u64,
    /// The harness's `keys` step, typed on the next frame.
    pub script: Option<String>,
}

/// What Ctrl+J (or a backtick) does now.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Toggle { Show, Focus, Hide }
pub fn toggle(open: bool, focused: bool) -> Toggle { match (open, focused) { (false, _) => Toggle::Show, (true, false) => Toggle::Focus, (true, true) => Toggle::Hide } }
/// What Shift+Esc does now: None when the terminal is shut.
pub fn dismiss(open: bool, focused: bool) -> Option<Toggle> { match (open, focused) { (false, _) => None, (true, true) => Some(Toggle::Hide), (true, false) => Some(Toggle::Focus) } }

/// A key name of GPUI as the X11 keysym the emulator takes (None: a key that types text).
pub fn keysym(key: &str) -> Option<u32> {
    Some(match key {
        "backspace" => 0xff08, "tab" => 0xff09, "enter" => 0xff0d, "escape" => 0xff1b, "home" => 0xff50, "left" => 0xff51, "up" => 0xff52, "right" => 0xff53,
        "down" => 0xff54, "pageup" => 0xff55, "pagedown" => 0xff56, "end" => 0xff57, "insert" => 0xff63, "delete" => 0xffff,
        f if f.len() > 1 && f.starts_with('f') => { let n: u32 = f[1..].parse().ok()?; if !(1..=12).contains(&n) { return None } 0xffbd + n }
        _ => return None,
    })
}
fn one(s: &str) -> Option<char> { let mut it = s.chars(); let c = it.next()?; it.next().is_none().then_some(c) }
/// The bytes a key press sends to the shell ("" for none: a Cmd chord, a key with no text).
pub fn bytes_for(k: &Keystroke, vt: &Vt) -> Vec<u8> {
    let m = &k.modifiers;
    if m.platform { return vec![] }
    let typed = k.key_char.as_deref().and_then(one);
    // a Mac's Option types a character of its own (Option+e is é): that is the text, with no Meta prefix
    let alt = m.alt && !(cfg!(target_os = "macos") && typed.is_some_and(|c| Some(c) != one(&k.key)));
    let mods = (m.shift as u32) | ((m.control as u32) << 2) | ((alt as u32) << 3);
    if let Some(ks) = keysym(&k.key) { return vt.key(ks, 0, mods) }
    let ch = if k.key == "space" { Some(' ') } else if m.control || alt { one(&k.key).map(|c| if m.shift && !m.control { typed.unwrap_or(c) } else { c }) } else { typed.or_else(|| one(&k.key)) };
    match ch { Some(c) => vt.key(0, c as u32, mods), None => vec![] }
}
/// The grid that fits `w` x `h` px of cells `cw` x `lh` px.
pub fn grid_size(w: f32, h: f32, cw: f32, lh: f32) -> (u16, u16) {
    let f = |a: f32, b: f32, max: usize| ((a / b.max(1.)).floor().max(1.) as usize).min(max) as u16;
    (f(w, cw, vt::MAX_COLS), f(h, lh, vt::MAX_ROWS))
}
/// The panel's place and size in a window of `vw` x `vh`: (x, y, w, h).
pub fn panel(vw: f32, vh: f32) -> (f32, f32, f32, f32) {
    let w = (vw - 160.).min(1000.).max(320.); let h = (vh * 0.48).min(460.).max(180.);
    ((vw - w) / 2., (vh - h - 150.).max(40.), w, h)
}
/// The text of one absolute row (the scrollback's first row is 0).
fn row_text(vt: &Vt, abs: usize) -> String {
    let (hist, rows) = (vt.history(), vt.rows());
    if abs >= hist + rows { return String::new() }
    let start = abs.min(hist);
    vt.lines(hist - start).get(abs - start).map(|l| l.runs.iter().map(|r| r.text.as_str()).collect()).unwrap_or_default()
}
/// What a selection covers, rows joined by newlines (trailing blanks are not part of a row).
pub fn selected_text(vt: &Vt, s: &Sel) -> String {
    let ((r0, c0), (r1, c1)) = s.span();
    let mut out = vec![];
    for r in r0..=r1 {
        let t: Vec<char> = row_text(vt, r).chars().collect();
        let (from, to) = (if r == r0 { c0 } else { 0 }, if r == r1 { c1 + 1 } else { usize::MAX });
        out.push(t.iter().skip(from).take(to.saturating_sub(from)).collect::<String>().trim_end().to_string());
    }
    out.join("\n")
}

impl Term {
    pub fn new(focus: FocusHandle) -> Term { Term { focus: Some(focus), ..Term::new_detached() } }
    pub fn new_detached() -> Term {
        Term { open: false, key: String::new(), screens: HashMap::new(), focus: None, size: (100, 24), cell: (8., LINE_PX), grid: Rc::new(Cell::new(Bounds::default())), sel: None, dragging: false, scroll: 0, tick: 0, script: None }
    }
    pub fn focused(&self, window: &Window) -> bool { self.focus.as_ref().is_some_and(|f| f.is_focused(window)) }
    pub fn take_keys(&self, window: &mut Window) { if let Some(f) = &self.focus { f.focus(window) } }
    pub fn vt(&self) -> Option<&Vt> { self.screens.get(&self.key).map(|s| &s.vt) }
    #[cfg(test)]
    pub fn has(&self, key: &str) -> bool { self.screens.contains_key(key) }
    /// A screen for `key`, made blank at this size when there is none (and the oldest other one dropped past MAX_SCREENS). True when new.
    pub fn ensure(&mut self, key: &str) -> bool {
        self.tick += 1; let tick = self.tick;
        if let Some(s) = self.screens.get_mut(key) { s.used = tick; return false }
        while self.screens.len() >= MAX_SCREENS {
            let Some(old) = self.screens.iter().filter(|(k, _)| *k != &self.key).min_by_key(|(_, s)| s.used).map(|(k, _)| k.clone()) else { break };
            self.screens.remove(&old);
        }
        self.screens.insert(key.into(), Screen { vt: Vt::new(self.size.0 as usize, self.size.1 as usize), pid: 0, used: tick });
        true
    }
    /// A `term` frame of the hub. Frames of terminals this client holds no screen for are not its (a chat terminal, another thread's shell
    /// it never opened). A new process (another pid) starts on a blank screen; a snapshot (`reset`) replaces the screen.
    /// Answers the emulator's replies (a cursor report...) for the shell on show, and whether the screen's size differs from the panel's.
    pub fn frame(&mut self, key: &str, pid: Option<i32>, bytes: &[u8], reset: Option<(usize, usize)>) -> (Vec<u8>, bool) {
        let (cols, rows) = (self.size.0 as usize, self.size.1 as usize);
        let Some(s) = self.screens.get_mut(key) else { return (vec![], false) };
        if let Some((c, r)) = reset { s.vt = Vt::new(c, r); }
        else if let Some(p) = pid.filter(|p| *p > 0) { if s.pid > 0 && s.pid != p { s.vt = Vt::new(cols, rows); } }
        if let Some(p) = pid.filter(|p| *p > 0) { s.pid = p; }
        s.vt.feed(bytes);
        let replies = s.vt.replies().to_vec(); s.vt.drain();
        let off = (s.vt.cols(), s.vt.rows()) != (cols, rows);
        if key == self.key {
            if self.scroll > 0 { self.scroll = self.scroll.min(s.vt.history()); }
            (if self.open { replies } else { vec![] }, off)
        } else { (vec![], false) }
    }
    /// The panel's grid changed to `cols` x `rows`: every screen is cut to it. True when that is news for the hub.
    pub fn resize(&mut self, cols: u16, rows: u16) -> bool {
        let fresh = self.size != (cols, rows);
        self.size = (cols, rows);
        for s in self.screens.values_mut() { if (s.vt.cols(), s.vt.rows()) != (cols as usize, rows as usize) { s.vt.resize(cols as usize, rows as usize) } }
        fresh
    }
    /// The cell under a window position: (absolute row, column).
    fn cell_at(&self, p: Point<Pixels>) -> Option<(usize, usize)> {
        let vt = self.vt()?; let b = self.grid.get();
        let (x, y) = (f32::from(p.x - b.origin.x).max(0.), f32::from(p.y - b.origin.y).max(0.));
        let col = ((x / self.cell.0) as usize).min(vt.cols().saturating_sub(1));
        let row = ((y / self.cell.1) as usize).min(vt.rows().saturating_sub(1));
        Some((vt.history() - self.scroll.min(vt.history()) + row, col))
    }
}

impl Backplane {
    fn term_rpc(&self, m: &str, p: serde_json::Value) { let _ = self.tx.try_send(Command::Rpc(m.into(), Secret(p), "term")); }
    fn term_send(&mut self, bytes: Vec<u8>) {
        if bytes.is_empty() || self.screens.term.key.is_empty() { return }
        let key = self.screens.term.key.clone();
        for chunk in bytes.chunks(MAX_INPUT) { self.term_rpc("term.input", json!({"thread": key, "b": chunk})); }
    }
    /// Show the open thread's shell and give it the keyboard (the hub starts it, or only resizes one that runs).
    fn term_show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = crate::screens::attach::raw_id(&self.selected).to_string();
        if key.is_empty() { self.error = "Open a thread first: the terminal works in its folder.".into(); cx.notify(); return }
        let t = &mut self.screens.term; t.open = true; t.key = key.clone(); t.sel = None; t.scroll = 0;
        let fresh = t.ensure(&key); let (c, r) = t.size;
        self.term_rpc("term.open", json!({"thread": key, "cols": c, "rows": r}));
        if fresh { self.term_rpc("term.snap", json!({"key": key})); }
        self.screens.term.take_keys(window); self.ui_epoch += 1; cx.notify();
    }
    /// Put the terminal away; the shell runs on, and the message box has the keyboard.
    pub(crate) fn term_hide(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.screens.term.open { return false }
        let t = &mut self.screens.term; t.open = false; t.dragging = false;
        self.want_focus = true; self.ui_epoch += 1; cx.notify(); true
    }
    fn term_act(&mut self, what: Toggle, window: &mut Window, cx: &mut Context<Self>) {
        match what { Toggle::Show => self.term_show(window, cx), Toggle::Focus => { self.screens.term.take_keys(window); cx.notify(); } Toggle::Hide => { self.term_hide(cx); } }
    }
    /// Ctrl+J, a backtick, the rail's Terminal button.
    pub(crate) fn term_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let t = &self.screens.term; let what = toggle(t.open, t.focused(window)); self.term_act(what, window, cx);
    }
    /// Shift+Esc.
    pub(crate) fn term_dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let t = &self.screens.term; if let Some(what) = dismiss(t.open, t.focused(window)) { self.term_act(what, window, cx); }
    }
    /// A `term` frame from the hub.
    pub(crate) fn term_frame(&mut self, key: &str, pid: Option<i32>, bytes: &[u8], reset: Option<(usize, usize)>, cx: &mut Context<Self>) {
        let (replies, off) = self.screens.term.frame(key, pid, bytes, reset);
        if off { let (c, r) = self.screens.term.size; self.screens.term.resize(c, r); self.term_rpc("term.resize", json!({"thread": key, "cols": c, "rows": r})); }
        self.term_send(replies);
        if key == self.screens.term.key && self.screens.term.open { cx.notify(); }
    }
    /// A new connection to the hub (the first, or after a drop): what was printed meanwhile is lost, so the shell on show is asked for again.
    pub(crate) fn term_rejoin(&mut self) {
        let t = &self.screens.term;
        if !t.open || t.key.is_empty() { return }
        let (key, (c, r)) = (t.key.clone(), t.size);
        self.term_rpc("term.open", json!({"thread": key, "cols": c, "rows": r}));
        self.term_rpc("term.snap", json!({"key": key}));
    }
    /// The harness's keystrokes, each dispatched as a key press after this frame (outside the window's update, as a real one).
    pub(crate) fn term_script(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keys) = self.screens.term.script.take() else { return };
        for k in keys.split_whitespace().filter_map(|k| Keystroke::parse(k).ok()) {
            window.defer(cx, move |window, cx| { window.dispatch_keystroke(k, cx); });
        }
    }
    /// Every frame: the panel follows the thread on screen, and its grid the window's size.
    pub(crate) fn term_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.screens.term.open { return }
        let key = crate::screens::attach::raw_id(&self.selected).to_string();
        if !key.is_empty() && key != self.screens.term.key {
            let focused = self.screens.term.focused(window);
            self.term_show(window, cx);
            if !focused { self.want_focus = true; }
        }
        let font = window.text_system().resolve_font(&crate::art::mono_font());
        let cw = window.text_system().advance(font, px(FONT_PX), 'm').map(|s| f32::from(s.width)).unwrap_or(FONT_PX * 0.6).max(1.);
        let v = window.viewport_size();
        let (_, _, w, h) = panel(f32::from(v.width), f32::from(v.height));
        let (c, r) = grid_size(w - 2. * PAD, h - HEAD - 2. * PAD, cw, LINE_PX);
        self.screens.term.cell = (cw, LINE_PX);
        if self.screens.term.resize(c, r) { let key = self.screens.term.key.clone(); self.term_rpc("term.resize", json!({"thread": key, "cols": c, "rows": r})); }
    }
    fn term_key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(vt) = self.screens.term.vt() else { return };
        let bytes = bytes_for(&e.keystroke, vt);
        if bytes.is_empty() { return }
        let t = &mut self.screens.term; t.scroll = 0; t.sel = None;
        self.term_send(bytes); cx.stop_propagation(); cx.notify();
    }
    fn term_copy(&mut self, cx: &mut Context<Self>) {
        let t = &self.screens.term;
        let (Some(vt), Some(s)) = (t.vt(), t.sel) else { return };
        let text = selected_text(vt, &s);
        if !text.is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(text)); }
    }
    fn term_paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        let Some(vt) = self.screens.term.vt() else { return };
        let bytes = vt.paste(&text); self.screens.term.scroll = 0; self.term_send(bytes); cx.notify();
    }
    fn term_wheel(&mut self, e: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let t = &mut self.screens.term; let Some(hist) = t.vt().map(|v| v.history()) else { return };
        let lines = match e.delta { ScrollDelta::Pixels(p) => f32::from(p.y) / LINE_PX, ScrollDelta::Lines(l) => l.y * 3. };
        let n = lines.round() as i64;
        if n == 0 { return }
        t.scroll = (t.scroll as i64 + n).clamp(0, hist as i64) as usize; cx.notify();
    }
}

/// The panel, while it is open.
pub fn view(cx: &mut Context<Backplane>, window: &mut Window, light: bool, l: &crate::local::Local, title: &str, t: &Term) -> Option<AnyElement> {
    if !t.open { return None }
    let c = |n: &str| theme::color(light, n);
    let v = window.viewport_size();
    let (x, y, w, h) = panel(f32::from(v.width), f32::from(v.height));
    let (cw, lh) = t.cell;
    let focused = t.focused(window);
    let focus = t.focus.clone()?; let grid = t.grid.clone();
    let (lines, cursor, rows_on, hist, scroll, sel, name) = match t.vt() {
        Some(vt) => (vt.lines(t.scroll), vt.cursor(), vt.rows(), vt.history(), t.scroll.min(vt.history()), t.sel, vt.title().to_string()),
        None => (vec![], vt::VCur { x: 0, y: 0, visible: false }, 0, 0, 0, None, String::new()),
    };
    let fg = |x: u32| rgb(x);
    let mut body = div().relative().font(crate::art::mono_font()).text_size(px(FONT_PX)).line_height(px(lh)).text_color(fg(vt::DEFAULT_FG)).whitespace_nowrap().overflow_hidden().size_full();
    body = body.child(canvas(move |b, _, _| grid.set(b), |_, _, _, _| {}).absolute().size_full());
    // the selection, under the text
    if let Some(s) = sel {
        let ((r0, c0), (r1, c1)) = s.span(); let top = hist - scroll;
        for r in r0.max(top)..=r1.min(top + rows_on.saturating_sub(1)) {
            if r1 < top { break }
            let (from, to) = (if r == r0 { c0 } else { 0 }, if r == r1 { c1 + 1 } else { t.size.0 as usize });
            body = body.child(div().absolute().left(px(from as f32 * cw)).top(px((r - top) as f32 * lh)).w(px((to.saturating_sub(from)) as f32 * cw)).h(px(lh)).bg(rgba(0x5b8cff55)));
        }
    }
    for (i, line) in lines.iter().enumerate() {
        let mut row = div().absolute().left_0().top(px(i as f32 * lh)).h(px(lh)).flex();
        for r in &line.runs {
            row = row.child(div().text_color(fg(r.fg)).when(r.bg != vt::DEFAULT_BG, |d| d.bg(fg(r.bg))).when(r.bold, |d| d.font_weight(FontWeight::BOLD)).when(r.underline, |d| d.underline()).child(r.text.clone()));
        }
        body = body.child(row);
    }
    if cursor.visible && scroll == 0 && rows_on > 0 {
        let cur = div().absolute().left(px(cursor.x as f32 * cw)).top(px(cursor.y as f32 * lh)).w(px(cw)).h(px(lh));
        body = body.child(if focused { cur.bg(rgba(0xd9dce4aa)) } else { cur.border_1().border_color(rgb(vt::DEFAULT_FG)) });
    }
    let hint = format!("{} hides · Shift+Esc back to the message · {} copy · {} paste", crate::keys::hint(l, "term"), crate::keys::show("secondary-shift-c", crate::keys::mac()), crate::keys::show("secondary-shift-v", crate::keys::mac()));
    let label = if !name.is_empty() { name } else if title.is_empty() { "Terminal".to_string() } else { format!("Terminal · {title}") };
    let label = if scroll > 0 { format!("{label} · {scroll} lines back") } else { label };
    Some(div().id("floating-terminal").absolute().left(px(x)).top(px(y)).w(px(w)).h(px(h)).flex().flex_col()
        .bg(rgb(vt::DEFAULT_BG)).border_1().border_color(if focused { c("accent") } else { c("line") }).shadow_lg()
        .track_focus(&focus).key_context("Terminal")
        .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| this.term_key(e, cx)))
        .on_action(cx.listener(|this, _: &Copy, _, cx| this.term_copy(cx)))
        .on_action(cx.listener(|this, _: &Paste, _, cx| this.term_paste(cx)))
        .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| this.term_wheel(e, cx)))
        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| {
            let t = &mut this.screens.term; t.take_keys(window);
            t.sel = t.cell_at(e.position).map(|p| Sel { a: p, b: p }); t.dragging = t.sel.is_some(); cx.stop_propagation(); cx.notify();
        }))
        .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
            let t = &mut this.screens.term; if !t.dragging { return }
            if let (Some(p), Some(s)) = (t.cell_at(e.position), t.sel.as_mut()) { if s.b != p { s.b = p; cx.notify(); } }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| { let t = &mut this.screens.term; t.dragging = false; if t.sel.is_some_and(|s| s.a == s.b) { t.sel = None; } cx.notify(); }))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| this.screens.term.dragging = false))
        .child(div().h(px(HEAD)).flex_shrink_0().px_3().flex().items_center().gap_3().border_b_1().border_color(rgb(0x2a2f3a)).text_size(px(11.))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_color(rgb(vt::DEFAULT_FG)).child(label))
            .child(div().text_color(rgb(0x8a90a0)).whitespace_nowrap().child(hint))
            .child(div().id("term-close").cursor_pointer().px_1().text_color(rgb(0x8a90a0)).hover(|s| s.text_color(rgb(vt::DEFAULT_FG))).child("✕")
                .on_click(cx.listener(|this, _, _, cx| { this.term_hide(cx); }))))
        .child(div().flex_1().min_h_0().p(px(PAD)).child(body))
        .into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    fn ks(s: &str) -> Keystroke { Keystroke::parse(s).unwrap() }
    /// A keystroke as the platform hands it over: the key, and the text it typed.
    fn typed(s: &str, ch: &str) -> Keystroke { let mut k = ks(s); k.key_char = Some(ch.into()); k }
    fn b(k: &Keystroke) -> Vec<u8> { bytes_for(k, &Vt::new(80, 24)) }

    #[test] fn keys_become_the_bytes_a_terminal_sends() {
        assert_eq!(b(&typed("a", "a")), b"a"); assert_eq!(b(&typed("shift-a", "A")), b"A"); assert_eq!(b(&typed("`", "`")), b"`", "a backtick is the shell's");
        assert_eq!(b(&typed("space", " ")), b" "); assert_eq!(b(&ks("ctrl-space")), [0]);
        assert_eq!(b(&ks("ctrl-c")), [3]); assert_eq!(b(&ks("ctrl-d")), [4]); assert_eq!(b(&ks("ctrl-k")), [11]); assert_eq!(b(&ks("ctrl-l")), [12]);
        assert_eq!(b(&ks("enter")), [13]); assert_eq!(b(&ks("backspace")), [127]); assert_eq!(b(&ks("tab")), [9]); assert_eq!(b(&ks("shift-tab")), b"\x1b[Z");
        assert_eq!(b(&ks("escape")), [27]); assert_eq!(b(&ks("up")), b"\x1b[A"); assert_eq!(b(&ks("left")), b"\x1b[D"); assert_eq!(b(&ks("ctrl-right")), b"\x1b[1;5C");
        assert_eq!(b(&ks("home")), b"\x1b[H"); assert_eq!(b(&ks("delete")), b"\x1b[3~"); assert_eq!(b(&ks("pageup")), b"\x1b[5~"); assert_eq!(b(&ks("f1")), b"\x1bOP"); assert_eq!(b(&ks("f5")), b"\x1b[15~");
        assert_eq!(b(&typed("é", "é")), "é".as_bytes());
        if !cfg!(target_os = "macos") { assert_eq!(b(&typed("alt-b", "b")), b"\x1bb", "Alt is Meta"); }
        assert_eq!(b(&ks("cmd-c")), b"", "a Cmd chord is the app's"); assert_eq!(b(&ks("f13")), b"");
    }
    #[test] fn arrows_follow_application_cursor_mode() {
        let mut vt = Vt::new(80, 24); vt.feed(b"\x1b[?1h");
        assert_eq!(bytes_for(&ks("up"), &vt), b"\x1bOA");
    }
    #[test] fn toggling_shows_then_takes_the_keyboard_then_hides() {
        assert_eq!(toggle(false, false), Toggle::Show); assert_eq!(toggle(true, false), Toggle::Focus); assert_eq!(toggle(true, true), Toggle::Hide);
        assert_eq!(dismiss(false, false), None); assert_eq!(dismiss(true, true), Some(Toggle::Hide)); assert_eq!(dismiss(true, false), Some(Toggle::Focus));
    }
    #[test] fn the_grid_fits_the_panel() {
        assert_eq!(grid_size(800., 340., 8., 17.), (100, 20)); assert_eq!(grid_size(3., 3., 8., 17.), (1, 1)); assert_eq!(grid_size(1e6, 1e6, 1., 1.), (vt::MAX_COLS as u16, vt::MAX_ROWS as u16));
        let (x, y, w, h) = panel(1400., 900.); assert!(x > 0. && y >= 40. && w <= 1000. && h <= 460. && y + h <= 900.);
    }
    #[test] fn frames_draw_only_screens_it_holds_and_a_new_shell_starts_blank() {
        let mut t = Term { size: (20, 4), ..Term::new_detached() };
        assert_eq!(t.frame("t1", Some(9), b"hi", None), (vec![], false), "a screen it never opened is not drawn");
        assert!(t.ensure("t1")); t.key = "t1".into(); t.open = true;
        t.frame("t1", Some(9), b"hello", None);
        assert_eq!(row_text(t.vt().unwrap(), t.vt().unwrap().history()), "hello");
        let (replies, _) = t.frame("t1", Some(9), b"\x1b[6n", None); assert_eq!(replies, b"\x1b[1;6R", "the cursor report goes back to the shell");
        t.frame("t1", Some(10), b"new", None); assert_eq!(row_text(t.vt().unwrap(), 0), "new", "another pid: a new shell on a blank screen");
        t.frame("t1", None, b"\x1b[2;1Hsnap", Some((30, 6))); let vt = t.vt().unwrap(); assert_eq!((vt.cols(), vt.rows()), (30, 6)); assert_eq!(row_text(vt, 1), "snap");
        assert_eq!(t.frame("t1", None, b"", None).1, true, "a snapshot of another size is cut to the panel's");
        t.open = false; assert_eq!(t.frame("t1", Some(10), b"\x1b[6n", None).0, b"", "a hidden terminal leaves the replies to whoever shows it");
    }
    #[test] fn screens_are_bounded() {
        let mut t = Term::new_detached();
        for i in 0..20 { t.ensure(&format!("t{i}")); t.key = format!("t{i}"); }
        assert_eq!(t.screens.len(), MAX_SCREENS); assert!(t.has("t19") && !t.has("t0"));
    }
    #[test] fn a_selection_copies_its_text() {
        let mut vt = Vt::new(20, 3); vt.feed(b"one two\r\nthree   \r\nfour");
        assert_eq!(selected_text(&vt, &Sel { a: (0, 4), b: (0, 6) }), "two");
        assert_eq!(selected_text(&vt, &Sel { a: (2, 1), b: (0, 4) }), "two\nthree\nfo", "either direction, trailing blanks dropped");
        let mut vt = Vt::new(10, 2); vt.feed(b"a\r\nb\r\nc\r\nd"); assert_eq!(vt.history(), 2);
        assert_eq!(selected_text(&vt, &Sel { a: (0, 0), b: (3, 0) }), "a\nb\nc\nd", "rows in the scrollback too");
    }
}

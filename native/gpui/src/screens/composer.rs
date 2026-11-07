//! The multi-line composer: wrapped text, caret and selection painted from GPUI's shaped lines, mouse
//! (click, drag, double and triple click), the system input method, undo/redo, and paste / drop of images
//! and files. The text model is `edit::Editor`; this file only maps GPUI input onto it. Attachments leave
//! as `ComposerEvent`s; the hub decides what is attached (screens::attach).
use super::edit::Editor;
use crate::theme;
use gpui::{
    actions, fill, point, prelude::*, px, relative, rgb, rgba, size, App, Bounds, ClipboardEntry, ClipboardItem, Context,
    ContentMask, CursorStyle, ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle,
    Focusable, GlobalElementId, ImageFormat, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle, WrappedLine, Window,
};
use std::ops::Range;
use std::path::PathBuf;

actions!(composer, [Backspace, Delete, WordBackspace, WordDelete, Left, Right, WordLeft, WordRight, SelLeft, SelRight, SelWordLeft, SelWordRight,
    Up, Down, SelUp, SelDown, Home, End, DocStart, DocEnd, SelHome, SelEnd, SelDocStart, SelDocEnd, SelectAll, Copy, Cut, Paste, Undo, Redo, Newline, CharPalette, Go, Tab]);

pub const MAX_LINES: f32 = 12.0;

/// `*` for every byte of every character, so a byte offset in the secret is the same offset in what is shown.
pub fn mask(s: &str) -> String { let mut o = String::with_capacity(s.len()); for ch in s.chars() { if ch == '\n' { o.push('\n'); } else { for _ in 0..ch.len_utf8() { o.push('*'); } } } o }

pub enum ComposerEvent {
    /// A clipboard image (encoded bytes and extension).
    Image { bytes: Vec<u8>, ext: &'static str },
    /// The drawing pad's picture (RGBA, encoded off the UI thread).
    Drawing(Vec<u8>),
    /// Files dropped on the composer or chosen in the picker.
    Files(Vec<PathBuf>),
    /// Enter in an ask's box (key context AskInput).
    Submit,
    /// Ctrl-C with nothing selected in the box: the window copies the timeline's selection.
    CopyTimeline,
    /// The suggestion popup is open and Up (-1) or Down (1) was pressed: the highlighted row moves instead of the caret.
    PopupMove(i32),
    /// The popup is open and Tab was pressed: the highlighted row is chosen.
    PopupPick,
}

/// Reference words drawn as chips in the draft (the colours are the accent over its own tint).
#[derive(Clone, Default)]
pub struct Chips { pub words: Vec<String>, pub fg: gpui::Hsla, pub bg: gpui::Hsla }
fn chip_runs(line: &str, base: &TextRun, chips: &Chips) -> Vec<TextRun> {
    let (mut runs, mut at) = (vec![], 0);
    for (i, e) in super::md::word_ranges(line, &chips.words) {
        if i > at { runs.push(TextRun { len: i - at, ..base.clone() }); }
        runs.push(TextRun { len: e - i, color: chips.fg, background_color: Some(chips.bg), ..base.clone() }); at = e;
    }
    if at < line.len() { runs.push(TextRun { len: line.len() - at, ..base.clone() }); }
    if runs.is_empty() { runs.push(TextRun { len: line.len(), ..base.clone() }); }
    runs
}

/// One logical line, shaped on its own so an empty line (and a trailing newline) is a row too.
struct Row { start: usize, len: usize, top: Pixels, line: Option<WrappedLine> }
impl Row {
    fn height(&self, lh: Pixels) -> Pixels { self.line.as_ref().map_or(lh, |l| l.size(lh).height.max(lh)) }
    fn pos(&self, off: usize, lh: Pixels) -> Point<Pixels> { self.line.as_ref().and_then(|l| l.position_for_index(off.min(self.len), lh)).unwrap_or_default() }
    fn hit(&self, p: Point<Pixels>, lh: Pixels) -> usize {
        match &self.line { Some(l) => match l.closest_index_for_position(p, lh) { Ok(i) | Err(i) => i.min(self.len) }, None => 0 }
    }
}

/// Shape every logical line of `text` for `width`, with the input method's composition underlined.
fn shape_rows(window: &mut Window, text: &str, base: &TextRun, marked: Option<&Range<usize>>, font_size: Pixels, width: Option<Pixels>, lh: Pixels, chips: &Chips) -> Vec<Row> {
    let (mut rows, mut start, mut top) = (Vec::new(), 0usize, px(0.));
    for l in text.split('\n') {
        let line = if l.is_empty() { None } else {
            let m = marked.and_then(|m| { let (a, b) = (m.start.max(start), m.end.min(start + l.len())); (a < b).then(|| a - start..b - start) });
            let runs = if m.is_none() && !chips.words.is_empty() { chip_runs(l, base, chips) } else { runs_for(l.len(), base, m.as_ref()) };
            window.text_system().shape_text(SharedString::from(l.to_string()), font_size, &runs, width, None).ok().and_then(|v| v.into_iter().next())
        };
        let row = Row { start, len: l.len(), top, line };
        top += row.height(lh); start += l.len() + 1; rows.push(row);
    }
    rows
}

pub struct Composer {
    focus: FocusHandle,
    ed: Editor,
    pub content: SharedString,
    /// Set by the window each frame: the suggestion popup is showing, so Up/Down/Tab drive it.
    pub popup_open: bool,
    /// Words the hub knows as references (`&claude/opus`...): painted as chips.
    pub chips: Vec<String>,
    placeholder: SharedString,
    rows: Vec<Row>,
    bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
    scroll_y: Pixels,
    selecting: bool,
    blink: super::blink::Blink,
    pub light: bool,
    /// Show every character as `*` (same byte length, so offsets stay exact); copy and cut give nothing.
    pub masked: bool,
    ctx: &'static str,
}

impl EventEmitter<ComposerEvent> for Composer {}

pub fn ext_of(f: ImageFormat) -> &'static str {
    match f { ImageFormat::Png => "png", ImageFormat::Jpeg => "jpg", ImageFormat::Webp => "webp", ImageFormat::Gif => "gif", ImageFormat::Bmp => "bmp", ImageFormat::Tiff => "tiff", ImageFormat::Svg => "svg", #[allow(unreachable_patterns)] _ => "png" }
}

impl Composer {
    pub fn new(cx: &mut Context<Self>) -> Self { Self::with_context(cx, "Composer") }
    /// A composer with its own key context: "Composer" sends on Enter (main.rs), "AskInput" emits Submit.
    pub fn with_context(cx: &mut Context<Self>, ctx: &'static str) -> Self {
        Self { blink: Default::default(), light: false, masked: false, ctx, focus: cx.focus_handle(), ed: Editor::default(), content: "".into(), popup_open: false, chips: vec![], placeholder: "Ask anything: @bot, >thread, %project, #room, &model, $skill, /btw".into(),
            rows: vec![], bounds: None, line_height: px(20.), scroll_y: px(0.), selecting: false }
    }
    pub fn set(&mut self, text: String, cx: &mut Context<Self>) { self.ed.set(&text); self.sync(cx); }
    pub fn dictate(&mut self, range: std::ops::Range<usize>, with: &str, caret: usize, cx: &mut Context<Self>) { self.ed.dictate(range, with, caret); self.sync(cx); }
    pub fn set_placeholder(&mut self, text: &str) { self.placeholder = text.to_string().into(); }
    /// Move the caret one character: `dir` < 0 back, else forward.
    pub fn caret(&mut self, dir: i32, cx: &mut Context<Self>) { self.edit(cx, |e| if dir < 0 { e.left(false, false) } else { e.right(false, false) }); }
    pub fn reset(&mut self) { self.ed.set(""); self.content = "".into(); self.rows.clear(); self.scroll_y = px(0.); }
    pub fn editor(&self) -> &Editor { &self.ed }
    pub fn select_everything(&mut self, cx: &mut Context<Self>) { self.edit(cx, |e| e.select_all()); }
    fn sync(&mut self, cx: &mut Context<Self>) { self.content = self.ed.text().to_string().into(); cx.notify(); }
    fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Editor)) { f(&mut self.ed); self.sync(cx); }

    // ---- actions
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.backspace(false)); }
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.delete(false)); }
    fn word_backspace(&mut self, _: &WordBackspace, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.backspace(true)); }
    fn word_delete(&mut self, _: &WordDelete, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.delete(true)); }
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.left(false, false)); }
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.right(false, false)); }
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.left(false, true)); }
    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.right(false, true)); }
    fn sel_left(&mut self, _: &SelLeft, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.left(true, false)); }
    fn sel_right(&mut self, _: &SelRight, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.right(true, false)); }
    fn sel_word_left(&mut self, _: &SelWordLeft, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.left(true, true)); }
    fn sel_word_right(&mut self, _: &SelWordRight, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.right(true, true)); }
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.home(false)); }
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.end(false)); }
    fn sel_home(&mut self, _: &SelHome, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.home(true)); }
    fn sel_end(&mut self, _: &SelEnd, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.end(true)); }
    fn doc_start(&mut self, _: &DocStart, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.doc_start(false)); }
    fn doc_end(&mut self, _: &DocEnd, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.doc_end(false)); }
    fn sel_doc_start(&mut self, _: &SelDocStart, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.doc_start(true)); }
    fn sel_doc_end(&mut self, _: &SelDocEnd, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.doc_end(true)); }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.select_all()); }
    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| { e.undo(); }); }
    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| { e.redo(); }); }
    fn go(&mut self, _: &Go, _: &mut Window, cx: &mut Context<Self>) { cx.emit(ComposerEvent::Submit); }
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) { self.edit(cx, |e| e.newline()); }
    fn palette(&mut self, _: &CharPalette, window: &mut Window, _: &mut Context<Self>) { window.show_character_palette(); }
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) { if self.popup_open { cx.emit(ComposerEvent::PopupMove(-1)); } else { self.vertical(-1, false, cx); } }
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) { if self.popup_open { cx.emit(ComposerEvent::PopupMove(1)); } else { self.vertical(1, false, cx); } }
    fn tab(&mut self, _: &Tab, _: &mut Window, cx: &mut Context<Self>) { if self.popup_open { cx.emit(ComposerEvent::PopupPick); } }
    fn sel_up(&mut self, _: &SelUp, _: &mut Window, cx: &mut Context<Self>) { self.vertical(-1, true, cx); }
    fn sel_down(&mut self, _: &SelDown, _: &mut Window, cx: &mut Context<Self>) { self.vertical(1, true, cx); }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.masked { return; }
        if !self.ed.selection().is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(self.ed.selected().to_string())); } else { cx.emit(ComposerEvent::CopyTimeline); }
    }
    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if self.masked { return; }
        let s = self.ed.cut(); if !s.is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(s)); self.sync(cx); }
    }
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else { return };
        let mut text = String::new();
        for entry in item.entries() {
            match entry {
                ClipboardEntry::Image(image) => cx.emit(ComposerEvent::Image { bytes: image.bytes.clone(), ext: ext_of(image.format) }),
                ClipboardEntry::String(s) => text.push_str(s.text()),
                #[allow(unreachable_patterns)] _ => {}
            }
        }
        if !text.is_empty() { self.edit(cx, |e| e.insert(&text)); }
    }

    // ---- geometry
    /// Move by one visual row, so wrapped lines behave as lines.
    fn vertical(&mut self, dir: i32, extend: bool, cx: &mut Context<Self>) {
        let at = self.point_of(self.ed.head());
        let target = at.map(|p| point(p.x, p.y + self.line_height * (dir as f32 + 0.5) - self.line_height * 0.5));
        match target {
            Some(t) if self.index_at_local(t).is_some() => {
                let i = self.index_at_local(t).unwrap();
                // at the first or last row the key goes to the document edge, as in a textarea; below the last row the hit test gives 0, not a place on that row
                let below = self.rows.last().map_or(false, |r| t.y >= r.top + r.height(self.line_height));
                let to = if dir < 0 && t.y < px(0.) { 0 } else if dir > 0 && (below || i == self.ed.head()) { self.ed.text().len() } else { i };
                self.edit(cx, |e| e.place(to, extend));
            }
            _ => self.edit(cx, |e| if dir < 0 { e.up(extend) } else { e.down(extend) }),
        }
    }
    /// Position of a byte offset in the text's own coordinates (before scroll).
    fn point_of(&self, offset: usize) -> Option<Point<Pixels>> {
        let row = self.rows.iter().rev().find(|r| r.start <= offset)?;
        let p = row.pos(offset - row.start, self.line_height);
        Some(point(p.x, row.top + p.y))
    }
    fn index_at_local(&self, p: Point<Pixels>) -> Option<usize> {
        if self.rows.is_empty() { return None; }
        if p.y < px(0.) { return Some(0); }
        let row = self.rows.iter().rev().find(|r| r.top <= p.y).unwrap_or(&self.rows[0]);
        Some(row.start + row.hit(point(p.x, p.y - row.top), self.line_height))
    }
    fn index_at(&self, window_pos: Point<Pixels>) -> usize {
        let Some(b) = self.bounds else { return 0 };
        self.index_at_local(point(window_pos.x - b.left(), window_pos.y - b.top() + self.scroll_y)).unwrap_or(0)
    }

    // ---- mouse
    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.selecting = true;
        let i = self.index_at(e.position);
        match e.click_count {
            2 => self.edit(cx, |ed| ed.select_word_at(i)),
            n if n >= 3 => self.edit(cx, |ed| ed.select_line_at(i)),
            _ => self.edit(cx, |ed| ed.place(i, e.modifiers.shift)),
        }
    }
    fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting { let i = self.index_at(e.position); self.edit(cx, |ed| ed.place(i, true)); }
    }
    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) { self.selecting = false; }

    // ---- utf16 for the input method
    fn to16(&self, offset: usize) -> usize { let t = self.ed.text(); let mut o = offset.min(t.len()); while !t.is_char_boundary(o) { o -= 1; } t[..o].encode_utf16().count() }
    fn from16(&self, offset: usize) -> usize {
        let (mut u8, mut u16) = (0, 0);
        for ch in self.ed.text().chars() { if u16 >= offset { break; } u16 += ch.len_utf16(); u8 += ch.len_utf8(); }
        u8
    }
    fn r16(&self, r: &Range<usize>) -> Range<usize> { self.to16(r.start)..self.to16(r.end) }
    fn r8(&self, r: &Range<usize>) -> Range<usize> { self.from16(r.start)..self.from16(r.end) }
}

impl EntityInputHandler for Composer {
    fn text_for_range(&mut self, r: Range<usize>, actual: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let r8 = self.r8(&r); let (a, b) = (r8.start.min(r8.end), r8.end.max(r8.start)); actual.replace(self.r16(&(a..b))); self.ed.text().get(a..b).map(str::to_string)
    }
    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let sel = self.ed.selection();
        Some(UTF16Selection { range: self.r16(&sel), reversed: self.ed.head() < sel.end && !sel.is_empty() })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> { self.ed.marked.as_ref().map(|r| self.r16(r)) }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) { self.ed.marked = None; }
    fn replace_text_in_range(&mut self, r: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        let r8 = r.map(|r| self.r8(&r));
        self.ed.ime(r8, text, false, None); self.sync(cx);
    }
    fn replace_and_mark_text_in_range(&mut self, r: Option<Range<usize>>, text: &str, sel: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        let r8 = r.map(|r| self.r8(&r));
        // the selection is relative to the new text, in utf16
        let sel8 = sel.map(|s| { let t: String = text.to_string(); let f = |o: usize| { let (mut b, mut u) = (0, 0); for ch in t.chars() { if u >= o { break; } u += ch.len_utf16(); b += ch.len_utf8(); } b }; f(s.start)..f(s.end) });
        self.ed.ime(r8, text, true, sel8); self.sync(cx);
    }
    fn bounds_for_range(&mut self, r: Range<usize>, bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let r8 = self.r8(&r);
        let a = self.point_of(r8.start)?; let b = self.point_of(r8.end).unwrap_or(a);
        Some(Bounds::from_corners(point(bounds.left() + a.x, bounds.top() + a.y - self.scroll_y), point(bounds.left() + b.x.max(a.x + px(1.)), bounds.top() + a.y - self.scroll_y + self.line_height)))
    }
    fn character_index_for_point(&mut self, p: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let i = self.index_at(p); Some(self.to16(i))
    }
}

struct Element_ { input: Entity<Composer> }
struct Prepaint { rows: Vec<Row>, carets: Vec<gpui::PaintQuad>, sel: Vec<gpui::PaintQuad>, empty: bool, scroll: Pixels }

impl IntoElement for Element_ { type Element = Self; fn into_element(self) -> Self { self } }

fn runs_for(len: usize, base: &TextRun, marked: Option<&Range<usize>>) -> Vec<TextRun> {
    match marked {
        Some(m) if m.end <= len => [(m.start, false), (m.end - m.start, true), (len - m.end, false)].into_iter().filter(|(l, _)| *l > 0)
            .map(|(l, under)| TextRun { len: l, underline: under.then(|| UnderlineStyle { color: Some(base.color), thickness: px(1.), wavy: false }), ..base.clone() }).collect(),
        _ => vec![TextRun { len, ..base.clone() }],
    }
}


impl gpui::Element for Element_ {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;
    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }
    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let input = self.input.read(cx);
        let text: SharedString = if input.content.is_empty() { input.placeholder.clone() } else if input.masked { mask(&input.content).into() } else { input.content.clone() };
        let style = window.text_style();
        let (font_size, lh) = (style.font_size.to_pixels(window.rem_size()), window.line_height());
        let base = TextRun { len: text.len(), font: style.font(), color: style.color, background_color: None, underline: None, strikethrough: None };
        let layout = window.request_measured_layout(Style { size: gpui::Size { width: relative(1.).into(), height: gpui::Length::Auto }, ..Default::default() }, move |known, avail, window, _| {
            let w = known.width.or(match avail.width { gpui::AvailableSpace::Definite(w) => Some(w), _ => None });
            let rows = shape_rows(window, &text, &base, None, font_size, w, lh, &Chips::default());
            let h = rows.last().map_or(lh, |r| r.top + r.height(lh));
            size(w.unwrap_or(px(100.)), h.max(lh).min(lh * MAX_LINES))
        });
        (layout, ())
    }
    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Prepaint {
        let input = self.input.read(cx);
        let light = input.light;
        let style = window.text_style();
        let (font_size, lh) = (style.font_size.to_pixels(window.rem_size()), window.line_height());
        let empty = input.content.is_empty();
        let (text, color) = if empty { (input.placeholder.clone(), theme::color(light, "faint").into()) } else if input.masked { (mask(&input.content).into(), style.color) } else { (input.content.clone(), style.color) };
        let base = TextRun { len: text.len(), font: style.font(), color, background_color: None, underline: None, strikethrough: None };
        let marked = if empty { None } else { input.ed.marked.clone() };
        let chips = if empty || input.masked { Chips::default() } else { let a = theme::color(light, "accent"); let mut bg: gpui::Hsla = a.into(); bg.a = 0.18; Chips { words: input.chips.clone(), fg: a.into(), bg } };
        let rows = shape_rows(window, &text, &base, marked.as_ref(), font_size, Some(bounds.size.width), lh, &chips);
        let total = rows.last().map_or(lh, |r| r.top + r.height(lh));
        // keep the caret in view
        let head = input.ed.head();
        let mut scroll = input.scroll_y;
        let caret_y = if empty { Some(px(0.)) } else { rows.iter().rev().find(|r| r.start <= head).map(|r| r.top + r.pos(head - r.start, lh).y) };
        if let Some(y) = caret_y { if y < scroll { scroll = y; } else if y + lh > scroll + bounds.size.height { scroll = y + lh - bounds.size.height; } }
        scroll = scroll.min((total - bounds.size.height).max(px(0.))).max(px(0.));
        let origin = point(bounds.left(), bounds.top() - scroll);
        let (accent, mut sel, mut carets) = (theme::color(light, "accent"), Vec::new(), Vec::new());
        let range = input.ed.selection();
        if empty || range.is_empty() {
            let at = if empty { Some(point(px(0.), px(0.))) } else { rows.iter().rev().find(|r| r.start <= head).map(|r| { let p = r.pos(head - r.start, lh); point(p.x, r.top + p.y) }) };
            if let Some(p) = at { carets.push(fill(Bounds::new(point(origin.x + p.x, origin.y + p.y), size(px(2.), lh)), accent)); }
        } else {
            let c = super::select::highlight(light); let w = bounds.size.width;
            for r in &rows {
                let end = r.start + r.len;
                let (a, b) = (range.start.max(r.start), range.end.min(end));
                if range.end < r.start || range.start > end || a > b { continue; }
                let (pa, pb) = (r.pos(a - r.start, lh), r.pos(b - r.start, lh));
                let nl = range.end > end && b == end; // the selection continues past this line's newline
                let right = |x: Pixels| if nl { w.max(x) } else { x };
                if pa.y == pb.y { sel.push(fill(Bounds::from_corners(point(origin.x + pa.x, origin.y + r.top + pa.y), point(origin.x + right(pb.x.max(pa.x + px(if nl { 6. } else { 0. }))), origin.y + r.top + pa.y + lh)), c)); }
                else {
                    let mut y = pa.y;
                    sel.push(fill(Bounds::from_corners(point(origin.x + pa.x, origin.y + r.top + y), point(origin.x + w, origin.y + r.top + y + lh)), c));
                    y += lh;
                    while y < pb.y { sel.push(fill(Bounds::from_corners(point(origin.x, origin.y + r.top + y), point(origin.x + w, origin.y + r.top + y + lh)), c)); y += lh; }
                    sel.push(fill(Bounds::from_corners(point(origin.x, origin.y + r.top + pb.y), point(origin.x + right(pb.x), origin.y + r.top + pb.y + lh)), c));
                }
            }
        }
        Prepaint { rows, carets, sel, empty, scroll }
    }
    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), pre: &mut Prepaint, window: &mut Window, cx: &mut App) {
        let focus = self.input.read(cx).focus.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.input.clone()), cx);
        let lh = window.line_height();
        let origin = point(bounds.left(), bounds.top() - pre.scroll);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for q in pre.sel.drain(..) { window.paint_quad(q); }
            for r in &pre.rows { if let Some(l) = &r.line { let _ = l.paint(point(origin.x, origin.y + r.top), lh, TextAlign::Left, Some(bounds), window, cx); } }
            if focus.is_focused(window) && self.input.read(cx).blink.shown() { for q in pre.carets.drain(..) { window.paint_quad(q); } }
        });
        // the blink: solid after an edit or a move, then the platform's phases; one repaint per phase change, only while focused
        let focused = focus.is_focused(window);
        let sig = { let i = self.input.read(cx); super::blink::sig(&i.content, i.ed.head(), i.ed.selection().start) };
        let wait = self.input.update(cx, |i, _| { i.blink.seen(sig, focused); if focused && !i.blink.pending { let w = i.blink.wait(); if w.is_some() { i.blink.pending = true; } w } else { None } });
        if let Some(w) = wait {
            let input = self.input.clone();
            cx.spawn(async move |cx| { cx.background_executor().timer(w).await; let _ = input.update(cx, |i, cx| { i.blink.pending = false; cx.notify(); }); }).detach();
        }
        let rows = std::mem::take(&mut pre.rows);
        let (scroll, empty) = (pre.scroll, pre.empty);
        self.input.update(cx, |i, _| { i.rows = if empty { vec![] } else { rows }; i.bounds = Some(bounds); i.line_height = lh; i.scroll_y = scroll; });
    }
}

impl Render for Composer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui::div().flex().key_context(self.ctx).track_focus(&self.focus).cursor(CursorStyle::IBeam).w_full()
            .on_action(cx.listener(Self::backspace)).on_action(cx.listener(Self::delete)).on_action(cx.listener(Self::word_backspace)).on_action(cx.listener(Self::word_delete))
            .on_action(cx.listener(Self::left)).on_action(cx.listener(Self::right)).on_action(cx.listener(Self::word_left)).on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::sel_left)).on_action(cx.listener(Self::sel_right)).on_action(cx.listener(Self::sel_word_left)).on_action(cx.listener(Self::sel_word_right))
            .on_action(cx.listener(Self::tab)).on_action(cx.listener(Self::up)).on_action(cx.listener(Self::down)).on_action(cx.listener(Self::sel_up)).on_action(cx.listener(Self::sel_down))
            .on_action(cx.listener(Self::home)).on_action(cx.listener(Self::end)).on_action(cx.listener(Self::sel_home)).on_action(cx.listener(Self::sel_end))
            .on_action(cx.listener(Self::doc_start)).on_action(cx.listener(Self::doc_end)).on_action(cx.listener(Self::sel_doc_start)).on_action(cx.listener(Self::sel_doc_end))
            .on_action(cx.listener(Self::select_all)).on_action(cx.listener(Self::copy)).on_action(cx.listener(Self::cut)).on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo)).on_action(cx.listener(Self::redo)).on_action(cx.listener(Self::newline)).on_action(cx.listener(Self::go)).on_action(cx.listener(Self::palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down)).on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up)).on_mouse_move(cx.listener(Self::mouse_move))
            .line_height(px(20.)).text_size(px(13.))
            .child(gpui::div().w_full().child(Element_ { input: cx.entity() }))
    }
}

impl Focusable for Composer { fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() } }

pub fn bind(cx: &mut App) { cx.bind_keys(bindings()); }
pub fn bindings() -> Vec<KeyBinding> {
    let mut v: Vec<KeyBinding> = Vec::new();
    for ctx in ["Composer", "AskInput", "ImpInput", "PickInput", "PadLength"] {
    let c = Some(ctx);
    v.extend([
        KeyBinding::new("backspace", Backspace, c), KeyBinding::new("delete", Delete, c),
        KeyBinding::new("secondary-backspace", WordBackspace, c), KeyBinding::new("secondary-delete", WordDelete, c),
        KeyBinding::new("left", Left, c), KeyBinding::new("right", Right, c),
        KeyBinding::new("secondary-left", WordLeft, c), KeyBinding::new("secondary-right", WordRight, c),
        KeyBinding::new("shift-left", SelLeft, c), KeyBinding::new("shift-right", SelRight, c),
        KeyBinding::new("secondary-shift-left", SelWordLeft, c), KeyBinding::new("secondary-shift-right", SelWordRight, c),
        KeyBinding::new("tab", Tab, c), KeyBinding::new("up", Up, c), KeyBinding::new("down", Down, c), KeyBinding::new("shift-up", SelUp, c), KeyBinding::new("shift-down", SelDown, c),
        KeyBinding::new("home", Home, c), KeyBinding::new("end", End, c), KeyBinding::new("shift-home", SelHome, c), KeyBinding::new("shift-end", SelEnd, c),
        KeyBinding::new("secondary-home", DocStart, c), KeyBinding::new("secondary-end", DocEnd, c),
        KeyBinding::new("secondary-shift-home", SelDocStart, c), KeyBinding::new("secondary-shift-end", SelDocEnd, c),
        KeyBinding::new("secondary-a", SelectAll, c), KeyBinding::new("secondary-c", Copy, c), KeyBinding::new("secondary-x", Cut, c), KeyBinding::new("secondary-v", Paste, c),
        KeyBinding::new("secondary-z", Undo, c), KeyBinding::new("secondary-shift-z", Redo, c), KeyBinding::new("secondary-y", Redo, c),
        KeyBinding::new("shift-enter", Newline, c),
    ]);
    }
    v.extend([KeyBinding::new("enter", Go, Some("AskInput")), KeyBinding::new("enter", Go, Some("PickInput")), KeyBinding::new("enter", Go, Some("PadLength"))]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn the_mask_keeps_every_byte_offset_and_newline() {
        let s = "key-é日\nline2";
        let m = mask(s);
        assert_eq!(m.len(), s.len()); assert_eq!(m.find('\n'), s.find('\n')); assert!(m.chars().all(|c| c == '*' || c == '\n'));
        assert_eq!(mask(""), "");
    }
}

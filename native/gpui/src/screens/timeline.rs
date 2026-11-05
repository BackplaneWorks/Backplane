//! The timeline's text: Markdown blocks drawn as styled, wrapped, selectable text (selection runs across blocks and
//! messages), links, code blocks with Copy and light highlighting, tables, and images. What to draw comes from the hub's
//! nodes (screens::md); this file only lays them out and answers the pointer. The pure rules are in md.rs and select.rs.
use super::{images::Slot, md::{self, Block, Link, Span, Tok}, select::{self, Pos, Selection, Text}};
use crate::{art, hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{cell::RefCell, ops::Range, rc::Rc};

/// Everything the pointer needs about the blocks drawn this frame; rebuilt on every render, the selection survives.
#[derive(Default)]
pub struct Registry { pub blocks: Vec<Info>, pub sel: Selection, pub dragging: bool, pub entry: usize }
pub struct Info { pub text: String, pub entry: usize, pub links: Vec<(Range<usize>, Link)>, pub layout: Option<TextLayout> }
pub type Reg = Rc<RefCell<Registry>>;

impl Registry {
    pub fn texts(&self) -> Vec<Text> { self.blocks.iter().map(|b| Text { text: b.text.clone(), entry: b.entry }).collect() }
    /// The position under a window point: inside a block its character; elsewhere the nearest block, above it its start, below its end.
    pub fn hit(&self, p: Point<Pixels>) -> Option<Pos> {
        let mut best: Option<(Pixels, usize, &TextLayout)> = None;
        for (i, b) in self.blocks.iter().enumerate() {
            let Some(l) = &b.layout else { continue };
            let r = l.bounds();
            let dy = if p.y < r.top() { r.top() - p.y } else if p.y > r.bottom() { p.y - r.bottom() } else { px(0.) };
            if best.map_or(true, |(d, _, _)| dy < d) { best = Some((dy, i, l)); }
        }
        let (_, i, l) = best?;
        let r = l.bounds(); let len = self.blocks[i].text.len();
        let off = if p.y < r.top() { 0 } else if p.y > r.bottom() { len } else { match l.index_for_position(p) { Ok(o) | Err(o) => o.min(len) } };
        Some(Pos { block: i, off })
    }
    pub fn link_at(&self, pos: Pos) -> Option<&Link> { self.blocks.get(pos.block)?.links.iter().find(|(r, _)| r.contains(&pos.off)).map(|(_, l)| l) }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind { Body, Quote, Code, Head }

/// A block of text that paints its own selection and reports its layout to the registry.
pub struct SelText { spans: Vec<Span>, kind: Kind, light: bool, reg: Reg, ord: usize, styled: Option<StyledText> }

impl SelText {
    pub fn new(reg: &Reg, spans: Vec<Span>, kind: Kind, light: bool) -> Self {
        let mut r = reg.borrow_mut();
        let mut at = 0usize; let mut links = Vec::new();
        for s in &spans { if let Some(l) = &s.style.link { links.push((at..at + s.text.len(), l.clone())); } at += s.text.len(); }
        let entry = r.entry;
        r.blocks.push(Info { text: md::plain(&spans), entry, links, layout: None });
        let ord = r.blocks.len() - 1;
        drop(r);
        Self { spans, kind, light, reg: reg.clone(), ord, styled: None }
    }
    fn runs(&self, base: &TextStyle) -> Vec<TextRun> {
        let c = |n: &str| theme::color(self.light, n);
        let mono = art::mono_font();
        let mut runs = Vec::new();
        if self.kind == Kind::Code {
            // light highlighting over the whole block (the chat carries no language)
            let text = md::plain(&self.spans);
            for (r, tok) in md::highlight(&text) {
                let color = match tok { Tok::Keyword => c("accent"), Tok::Str => c("ok"), Tok::Comment => c("faint"), Tok::Number => c("dim"), Tok::Plain => c("text") };
                runs.push(TextRun { len: r.len(), font: mono.clone(), color: color.into(), background_color: None, underline: None, strikethrough: None });
            }
            return runs;
        }
        for s in &self.spans {
            if s.text.is_empty() { continue; }
            let mut font = base.font();
            if s.style.bold || self.kind == Kind::Head { font.weight = FontWeight::BOLD; }
            if s.style.italic { font.style = FontStyle::Italic; }
            if s.style.code { font = mono.clone(); }
            let color = if s.style.link.is_some() { c("accent") } else if self.kind == Kind::Quote { c("dim") } else { c("text") };
            runs.push(TextRun {
                len: s.text.len(), font, color: color.into(),
                background_color: s.style.code.then(|| c("raise").into()),
                underline: s.style.link.is_some().then(|| UnderlineStyle { color: Some(color.into()), thickness: px(1.), wavy: false }),
                strikethrough: None,
            });
        }
        runs
    }
}

impl IntoElement for SelText { type Element = Self; fn into_element(self) -> Self { self } }

impl Element for SelText {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }
    fn request_layout(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let text = md::plain(&self.spans);
        let runs = self.runs(&window.text_style());
        let mut styled = StyledText::new(text).with_runs(runs);
        let r = styled.request_layout(id, inspector, window, cx);
        self.styled = Some(styled);
        r
    }
    fn prepaint(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>, bounds: Bounds<Pixels>, state: &mut (), window: &mut Window, cx: &mut App) {
        if let Some(s) = self.styled.as_mut() { s.prepaint(id, inspector, bounds, state, window, cx); }
    }
    fn paint(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>, bounds: Bounds<Pixels>, rl: &mut (), pre: &mut (), window: &mut Window, cx: &mut App) {
        let Some(styled) = self.styled.as_mut() else { return };
        let layout = styled.layout().clone();
        // the selection first, under the glyphs
        let range = { let r = self.reg.borrow(); r.sel.within(self.ord, r.blocks[self.ord].text.len()) };
        if let Some(range) = range {
            let lh = layout.line_height();
            let tint = { let mut a = theme::color(self.light, "accent"); a.a = 0.28; a };
            if let (Some(pa), Some(pb)) = (layout.position_for_index(range.start), layout.position_for_index(range.end)) {
                let (left, right) = (bounds.left(), bounds.right());
                if pa.y == pb.y { window.paint_quad(fill(Bounds::from_corners(pa, point(pb.x, pb.y + lh)), tint)); }
                else {
                    window.paint_quad(fill(Bounds::from_corners(pa, point(right, pa.y + lh)), tint));
                    let mut y = pa.y + lh;
                    while y < pb.y { window.paint_quad(fill(Bounds::from_corners(point(left, y), point(right, y + lh)), tint)); y += lh; }
                    window.paint_quad(fill(Bounds::from_corners(point(left, pb.y), point(pb.x, pb.y + lh)), tint));
                }
            }
        }
        styled.paint(id, inspector, bounds, rl, pre, window, cx);
        if let Some(b) = self.reg.borrow_mut().blocks.get_mut(self.ord) { b.layout = Some(layout); }
    }
}

fn text(reg: &Reg, light: bool, spans: Vec<Span>, kind: Kind) -> AnyElement { SelText::new(reg, spans, kind, light).into_any_element() }
fn plain_spans(s: &str) -> Vec<Span> { vec![Span { text: s.to_string(), style: Default::default() }] }

/// A user's message text.
pub fn user_text(reg: &Reg, light: bool, s: &str) -> AnyElement { text(reg, light, plain_spans(s), Kind::Body) }

fn block_el(cx: &mut Context<Backplane>, reg: &Reg, light: bool, b: &Block, key: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    match b {
        Block::Para(s) => text(reg, light, s.clone(), Kind::Body),
        Block::Heading(l, s) => div().text_size(px(match l { 1 => 20., 2 => 17., 3 => 15., _ => 14. })).mt_1().child(text(reg, light, s.clone(), Kind::Head)).into_any_element(),
        Block::Quote(s) => div().border_l_2().border_color(c("line")).pl_3().child(text(reg, light, s.clone(), Kind::Quote)).into_any_element(),
        Block::Rule => div().h(px(1.)).w_full().bg(c("line")).my_1().into_any_element(),
        Block::List { ordered, start, items } => div().flex().flex_col().gap_1().pl(px(8.)).children(items.iter().enumerate().map(|(i, it)| {
            let mark = if *ordered { format!("{}.", start + i as u64) } else { "•".to_string() };
            div().flex().gap_2().child(div().min_w(px(18.)).flex_shrink_0().text_color(c("dim")).child(mark)).child(div().flex_1().min_w_0().child(text(reg, light, it.clone(), Kind::Body))).into_any_element()
        }).collect::<Vec<_>>()).into_any_element(),
        Block::Code(t) => {
            let copy = t.clone();
            div().w_full().bg(c("raise")).border_1().border_color(c("line")).flex().flex_col()
                .child(div().flex().justify_end().px_2().pt_1().child(div().id(SharedString::from(format!("copy-{key}"))).px_2().text_size(px(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                    .child("Copy").on_click(cx.listener(move |_, _, _, cx| { cx.write_to_clipboard(ClipboardItem::new_string(copy.clone())); }))))
                .child(div().id(SharedString::from(format!("code-{key}"))).overflow_x_scroll().px_2().pb_2().text_size(px(12.)).whitespace_nowrap().child(text(reg, light, plain_spans(t), Kind::Code)))
                .into_any_element()
        }
        Block::Table { head, rows } => {
            let cols = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0)).max(1);
            let row = |cells: &Vec<Vec<Span>>, bold: bool| div().flex().border_b_1().border_color(c("line")).children((0..cols).map(|i| {
                let spans = cells.get(i).cloned().unwrap_or_default();
                let spans = if bold { spans.into_iter().map(|mut s| { s.style.bold = true; s }).collect() } else { spans };
                div().flex_1().min_w(px(60.)).px_2().py_1().child(text(reg, light, spans, Kind::Body)).into_any_element()
            }).collect::<Vec<_>>());
            let mut t = div().id(SharedString::from(format!("tbl-{key}"))).overflow_x_scroll().border_1().border_color(c("line")).flex().flex_col().text_size(px(12.5));
            if !head.is_empty() { t = t.child(row(head, true).bg(c("raise"))); }
            for r in rows.iter().take(500) { t = t.child(row(r, false)); }
            t.into_any_element()
        }
    }
}

/// An assistant message's blocks, or its plain text when the hub sent none.
pub fn blocks(cx: &mut Context<Backplane>, reg: &Reg, light: bool, nodes: &[Value], key: &str) -> Vec<AnyElement> {
    md::parse(nodes).iter().enumerate().map(|(i, b)| block_el(cx, reg, light, b, &format!("{key}-{i}"))).collect()
}

/// Images a message names: each as its picture when it has arrived, a placeholder while it loads or when it cannot.
pub fn images(light: bool, list: &Value, key: &str, target: Option<&super::attach::Target>, imgs: &super::images::Images) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let items = list.as_array().filter(|a| !a.is_empty())?;
    let mut row = div().flex().flex_wrap().gap_2();
    for (i, it) in items.iter().take(12).enumerate() {
        let url = it["url"].as_str().unwrap_or("");
        let name = it["path"].as_str().unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
        let el = match imgs.get(target, url) {
            Slot::Ready(img) => gpui::img(ImageSource::Image(img)).max_w(px(480.)).max_h(px(360.)).object_fit(ObjectFit::Contain).into_any_element(),
            Slot::Loading => div().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("dim")).child(format!("Loading {name}…")).into_any_element(),
            Slot::Failed => div().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("bad")).child(format!("Could not load {name}")).into_any_element(),
        };
        row = row.child(div().id(SharedString::from(format!("img-{key}-{i}"))).child(el));
    }
    Some(row.into_any_element())
}

/// Repaint a few times a second while an image is still arriving, then stop (no standing repaint loop).
pub fn poll(cx: &mut Context<Backplane>) {
    cx.spawn(async move |weak, cx| loop {
        cx.background_executor().timer(std::time::Duration::from_millis(150)).await;
        let Ok(more) = weak.update(cx, |this, cx| { cx.notify(); let more = this.screens.images.loading(); if !more { this.screens.img_poll = false; } more }) else { return };
        if !more { return; }
    }).detach();
}

impl Backplane {
    fn timeline_pos(&self, p: Point<Pixels>) -> Option<Pos> { self.screens.reg.borrow().hit(p) }
    pub(crate) fn timeline_down(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let Some(pos) = self.timeline_pos(e.position) else { return };
        if e.button == MouseButton::Right {
            let link = self.screens.reg.borrow().link_at(pos).cloned();
            if let Some(l) = link { cx.write_to_clipboard(ClipboardItem::new_string(l.href)); }
            return;
        }
        let mut r = self.screens.reg.borrow_mut();
        let texts = r.texts();
        match e.click_count {
            2 => { let w = select::word_at(&texts[pos.block].text, pos.off); r.sel = Selection { anchor: Pos { block: pos.block, off: w.start }, head: Pos { block: pos.block, off: w.end } }; }
            n if n >= 3 => { if let Some(s) = select::block_range(&texts, pos.block) { r.sel = s; } }
            _ => { r.sel = if e.modifiers.shift { Selection { anchor: r.sel.anchor, head: pos } } else { Selection { anchor: pos, head: pos } }; r.dragging = true; }
        }
        drop(r); cx.notify();
    }
    pub(crate) fn timeline_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.screens.reg.borrow().dragging { return; }
        if let Some(pos) = self.timeline_pos(e.position) { self.screens.reg.borrow_mut().sel.head = pos; cx.notify(); }
    }
    pub(crate) fn timeline_up(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        let was = std::mem::replace(&mut self.screens.reg.borrow_mut().dragging, false);
        if !was { return; }
        // a click without a drag on a link opens it
        let (collapsed, pos) = { let r = self.screens.reg.borrow(); (r.sel.collapsed(), self.timeline_pos(e.position)) };
        let link = pos.filter(|_| collapsed).and_then(|p| self.screens.reg.borrow().link_at(p).cloned());
        if let Some(l) = link {
            if l.file { let _ = self.tx.try_send(Command::Action("file-open".into(), l.href)); }
            else if l.href.starts_with("https://") || l.href.starts_with("http://") { cx.open_url(&l.href); }
        }
        cx.notify();
    }
    /// Ctrl-C with the composer's selection empty: copy what is selected in the timeline.
    pub(crate) fn timeline_copy(&mut self, cx: &mut Context<Self>) {
        let s = { let r = self.screens.reg.borrow(); select::extract(&r.texts(), &r.sel) };
        if !s.is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(s)); }
    }
}

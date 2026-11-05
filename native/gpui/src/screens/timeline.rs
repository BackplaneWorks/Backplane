//! The timeline's text: Markdown blocks drawn as styled, wrapped, selectable text (selection runs across blocks and
//! messages), links, code blocks with Copy and light highlighting, tables, and images. What to draw comes from the hub's
//! nodes (screens::md); this file only lays them out and answers the pointer. The pure rules are in md.rs and select.rs.
use super::{images::Slot, md::{self, Block, Link, Span, Tok}, select::{self, Pos, Selection, Text}};
use crate::{art, hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{cell::RefCell, ops::Range, rc::Rc};

/// Everything the pointer needs about the blocks drawn this frame; rebuilt on every render, the selection survives.
pub struct Registry {
    pub blocks: Vec<Info>, pub dragging: bool, pub entry: usize,
    /// The entry being drawn and how many blocks it has had so far: a block's identity is (entry id, index in entry).
    pub entry_id: String, idx: usize, last_id: String,
    /// The selection as the person made it, by identity; `sel` is that resolved to this frame's ordinals.
    keyed: Option<(KPos, KPos)>, pub sel: Selection,
    /// Where each code block's Copy button was painted this frame (for the check harness), by index.
    pub copies: Vec<(usize, Bounds<Pixels>)>,
    pub copy_seen: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct KPos { pub entry: String, pub idx: usize, pub off: usize }
pub struct Info { pub text: String, pub entry: usize, pub id: String, pub idx: usize, pub links: Vec<(Range<usize>, Link)>, pub layout: Option<TextLayout> }
pub type Reg = Rc<RefCell<Registry>>;

impl Default for Registry {
    fn default() -> Self { Self { blocks: Vec::new(), dragging: false, entry: 0, entry_id: String::new(), idx: 0, last_id: String::new(), keyed: None, sel: Selection::default(), copies: Vec::new(), copy_seen: 0 } }
}

impl Registry {
    /// A new frame: forget the blocks, keep the selection (by identity).
    pub fn begin(&mut self) { self.blocks.clear(); self.copies.clear(); self.copy_seen = 0; self.idx = 0; self.last_id.clear(); }
    fn next_idx(&mut self) -> usize { if self.last_id != self.entry_id { self.last_id = self.entry_id.clone(); self.idx = 0; } let i = self.idx; self.idx += 1; i }
    fn key(&self, p: Pos) -> Option<KPos> { let b = self.blocks.get(p.block)?; Some(KPos { entry: b.id.clone(), idx: b.idx, off: p.off.min(b.text.len()) }) }
    fn ord(&self, k: &KPos) -> Option<Pos> { self.blocks.iter().position(|b| b.id == k.entry && b.idx == k.idx).map(|block| Pos { block, off: k.off }) }
    /// Make `s` (ordinals of this frame) the selection.
    pub fn select(&mut self, s: Selection) {
        self.keyed = self.key(s.anchor).zip(self.key(s.head));
        self.sel = s;
    }
    /// After the frame's blocks are known: the selection by identity as ordinals. One whose message is gone clears.
    pub fn resolve(&mut self) {
        let Some((a, h)) = self.keyed.clone() else { self.sel = Selection::default(); return };
        match self.ord(&a).zip(self.ord(&h)) { Some((anchor, head)) => self.sel = Selection { anchor, head }, None => { self.keyed = None; self.sel = Selection::default(); } }
    }
    pub fn anchor(&self) -> Selection { self.sel }
    pub fn texts(&self) -> Vec<Text> { self.blocks.iter().map(|b| Text { text: b.text.clone(), entry: b.entry }).collect() }
    /// The position under a window point: inside a block its character; elsewhere the nearest block, above it its start, below its end.
    pub fn hit(&self, p: Point<Pixels>) -> Option<Pos> {
        // the block under the point first (a table's cells share a row, so distance alone would pick the first), else the nearest
        let inside = self.blocks.iter().position(|b| b.layout.as_ref().map_or(false, |l| l.bounds().contains(&p)));
        let i = match inside {
            Some(i) => i,
            None => {
                let mut best: Option<(Pixels, Pixels, usize)> = None;
                for (i, b) in self.blocks.iter().enumerate() {
                    let Some(l) = &b.layout else { continue };
                    let r = l.bounds();
                    let dy = if p.y < r.top() { r.top() - p.y } else if p.y > r.bottom() { p.y - r.bottom() } else { px(0.) };
                    let dx = if p.x < r.left() { r.left() - p.x } else if p.x > r.right() { p.x - r.right() } else { px(0.) };
                    if best.map_or(true, |(by, bx, _)| (dy, dx) < (by, bx)) { best = Some((dy, dx, i)); }
                }
                best?.2
            }
        };
        let l = self.blocks[i].layout.as_ref()?;
        let r = l.bounds(); let len = self.blocks[i].text.len();
        let off = if p.y < r.top() { 0 } else if p.y > r.bottom() { len } else { match l.index_for_position(p) { Ok(o) | Err(o) => o.min(len) } };
        Some(Pos { block: i, off })
    }
    pub fn link_at(&self, pos: Pos) -> Option<&Link> { self.blocks.get(pos.block)?.links.iter().find(|(r, _)| r.contains(&pos.off)).map(|(_, l)| l) }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind { Body, Quote, Code, Head }

/// A block of text that paints its own selection and reports its layout to the registry.
pub struct SelText { spans: Vec<Span>, kind: Kind, light: bool, reg: Reg, ord: usize, styled: Option<StyledText>, tokens: Vec<(Range<usize>, Tok)> }

impl SelText {
    pub fn new(reg: &Reg, spans: Vec<Span>, kind: Kind, light: bool) -> Self {
        let mut r = reg.borrow_mut();
        let mut at = 0usize; let mut links = Vec::new();
        for s in &spans { if let Some(l) = &s.style.link { links.push((at..at + s.text.len(), l.clone())); } at += s.text.len(); }
        let entry = r.entry; let (id, idx) = (r.entry_id.clone(), r.next_idx());
        r.blocks.push(Info { text: md::plain(&spans), entry, id, idx, links, layout: None });
        let ord = r.blocks.len() - 1;
        drop(r);
        Self { spans, kind, light, reg: reg.clone(), ord, styled: None, tokens: Vec::new() }
    }
    fn runs(&self, base: &TextStyle) -> Vec<TextRun> {
        let c = |n: &str| theme::color(self.light, n);
        let mono = art::mono_font();
        let mut runs = Vec::new();
        if self.kind == Kind::Code {
            // light highlighting over the whole block (the chat carries no language), tokens computed when it was parsed
            for (r, tok) in &self.tokens {
                let color = match *tok { Tok::Keyword => c("accent"), Tok::Str => c("ok"), Tok::Comment => c("faint"), Tok::Number => c("dim"), Tok::Plain => c("text") };
                runs.push(TextRun { len: r.len(), font: mono.clone(), color: color.into(), background_color: None, underline: None, strikethrough: None });
            }
            if runs.is_empty() { runs.push(TextRun { len: 0, font: mono, color: c("text").into(), background_color: None, underline: None, strikethrough: None }); }
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
    type PrepaintState = Vec<(Hitbox, CursorStyle)>;
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
    fn prepaint(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Vec<(Hitbox, CursorStyle)> {
        let mut unit = ();
        if let Some(s) = self.styled.as_mut() { s.prepaint(id, inspector, bounds, &mut unit, window, cx); }
        // the OS I-beam over selectable text, the hand over a link (the later request wins where they overlap)
        let mut out = vec![(window.insert_hitbox(bounds, HitboxBehavior::Normal), CursorStyle::IBeam)];
        let Some(styled) = self.styled.as_ref() else { return out };
        let layout = styled.layout().clone(); let lh = layout.line_height();
        let links: Vec<Range<usize>> = self.reg.borrow().blocks.get(self.ord).map(|b| b.links.iter().map(|(r, _)| r.clone()).collect()).unwrap_or_default();
        for r in links {
            if let (Some(pa), Some(pb)) = (layout.position_for_index(r.start), layout.position_for_index(r.end)) {
                let rect = if pa.y == pb.y { Bounds::from_corners(pa, point(pb.x, pb.y + lh)) } else { Bounds::from_corners(point(bounds.left(), pa.y), point(bounds.right(), pb.y + lh)) };
                out.push((window.insert_hitbox(rect, HitboxBehavior::Normal), CursorStyle::PointingHand));
            }
        }
        out
    }
    fn paint(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>, bounds: Bounds<Pixels>, rl: &mut (), pre: &mut Vec<(Hitbox, CursorStyle)>, window: &mut Window, cx: &mut App) {
        let Some(styled) = self.styled.as_mut() else { return };
        let layout = styled.layout().clone();
        // the selection first, under the glyphs
        let range = { let r = self.reg.borrow(); r.blocks.get(self.ord).and_then(|b| r.sel.within(self.ord, b.text.len())) };
        if let Some(range) = range {
            let lh = layout.line_height();
            let tint = select::highlight(self.light);
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
        styled.paint(id, inspector, bounds, rl, &mut (), window, cx);
        for (h, style) in pre.iter() { window.set_cursor_style(*style, h); }
        if let Some(b) = self.reg.borrow_mut().blocks.get_mut(self.ord) { b.layout = Some(layout); }
    }
}

fn text(reg: &Reg, light: bool, spans: Vec<Span>, kind: Kind) -> AnyElement { SelText::new(reg, spans, kind, light).into_any_element() }
fn plain_spans(s: &str) -> Vec<Span> { vec![Span { text: s.to_string(), style: Default::default() }] }

/// A person's message whose text may wrap gets the bubble's full width (640 px at most) instead of shrinking to its text: a shrink-to-fit
/// parent let the layout measure the wrapping text at one width and paint it at another, so a long message was drawn over what follows it.
/// A message of short lines never wraps and keeps its snug bubble.
pub fn bubble_full_width(text: &str) -> bool { text.lines().any(|l| l.chars().count() > 60) }

/// A user's message text.
pub fn user_text(reg: &Reg, light: bool, s: &str) -> AnyElement { text(reg, light, md::linkify(s), Kind::Body) }

/// An invisible overlay that records where its parent was painted, for the check harness's control rectangles.
fn copy_probe(reg: &Reg, ix: usize) -> impl IntoElement {
    let reg = reg.clone();
    canvas(|b, _, _| b, move |_, b, _, _| { reg.borrow_mut().copies.push((ix, b)); }).absolute().size_full()
}

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
        Block::Code(t, tokens) => {
            let copy = t.clone();
            let copy_ix = { let mut r = reg.borrow_mut(); let n = r.copy_seen; r.copy_seen += 1; n };
            div().w_full().bg(c("raise")).border_1().border_color(c("line")).flex().flex_col()
                .child(div().flex().justify_end().px_2().pt_1().child(div().id(SharedString::from(format!("copy-{key}"))).relative().px_2().text_size(px(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                    .child("Copy").child(copy_probe(reg, copy_ix)).on_click(cx.listener(move |_, _, _, cx| { cx.write_to_clipboard(ClipboardItem::new_string(copy.clone())); }))))
                .child(div().id(SharedString::from(format!("code-{key}"))).overflow_x_scroll().px_2().pb_2().text_size(px(12.)).whitespace_nowrap().child({ let mut s = SelText::new(reg, plain_spans(t), Kind::Code, light); s.tokens = tokens.clone(); s.into_any_element() }))
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
            if rows.len() > 500 { t = t.child(div().px_2().py_1().text_color(c("dim")).child(format!("… {} more rows", rows.len() - 500))); }
            t.into_any_element()
        }
    }
}

/// Parsed blocks by entry id, valid while the entry's text and node count are the same: a streaming reply is parsed once per change, not once per frame.
#[derive(Default)]
pub struct ParseCache { items: std::collections::HashMap<String, (u64, Rc<Vec<Block>>)> }
const PARSE_KEEP: usize = 400;
fn fingerprint(text: &str, nodes: &[Value]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new(); text.hash(&mut h); nodes.len().hash(&mut h); h.finish()
}
impl ParseCache {
    pub fn get(&mut self, id: &str, text: &str, nodes: &[Value]) -> Rc<Vec<Block>> {
        let f = fingerprint(text, nodes);
        if let Some((g, b)) = self.items.get(id) { if *g == f { return b.clone(); } }
        if self.items.len() >= PARSE_KEEP { self.items.clear(); }
        let b = Rc::new(md::parse(nodes));
        self.items.insert(id.to_string(), (f, b.clone()));
        b
    }
}

/// An assistant message's blocks.
pub fn blocks(cx: &mut Context<Backplane>, reg: &Reg, light: bool, parsed: &[Block], key: &str) -> Vec<AnyElement> {
    parsed.iter().enumerate().map(|(i, b)| block_el(cx, reg, light, b, &format!("{key}-{i}"))).collect()
}

const MAX_IMAGES: usize = 12;
/// Images a message names: each as its picture when it has arrived, a placeholder while it loads or when it cannot.
pub fn images(light: bool, list: &Value, key: &str, target: Option<&super::attach::Target>, imgs: &super::images::Images) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let items = list.as_array().filter(|a| !a.is_empty())?;
    let mut row = div().flex().flex_wrap().gap_2();
    for (i, it) in items.iter().take(MAX_IMAGES).enumerate() {
        let url = it["url"].as_str().unwrap_or("");
        let name = it["path"].as_str().unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
        let el = match imgs.get(target, url) {
            Slot::Ready(img) => gpui::img(ImageSource::Image(img)).max_w(px(480.)).max_h(px(360.)).object_fit(ObjectFit::Contain).into_any_element(),
            Slot::Loading => div().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("dim")).child(format!("Loading {name}…")).into_any_element(),
            Slot::Failed => div().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("bad")).child(format!("Could not load {name}")).into_any_element(),
        };
        row = row.child(div().id(SharedString::from(format!("img-{key}-{i}"))).child(el));
    }
    if items.len() > MAX_IMAGES { row = row.child(div().px_2().py_1().text_size(px(12.)).text_color(c("dim")).child(format!("+{} more images", items.len() - MAX_IMAGES))); }
    Some(row.into_any_element())
}

/// A sent message's attachments (the hub's chips: label, path, image, url): pictures drawn like a reply's images, other files as a chip.
pub fn split_attachments(list: &Value) -> (Vec<Value>, Vec<String>) {
    let mut pics = Vec::new(); let mut files = Vec::new();
    for a in list.as_array().into_iter().flatten() {
        if a["image"].as_bool().unwrap_or(false) && a["url"].as_str().map_or(false, |u| !u.is_empty()) { pics.push(a.clone()); }
        else { let l = a["label"].as_str().filter(|l| !l.is_empty()).or_else(|| a["path"].as_str()).unwrap_or("").rsplit('/').next().unwrap_or("").to_string(); if !l.is_empty() { files.push(l); } }
    }
    (pics, files)
}
pub fn attachments(light: bool, list: &Value, key: &str, target: Option<&super::attach::Target>, imgs: &super::images::Images) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let (pics, files) = split_attachments(list);
    let mut out = Vec::new();
    if let Some(p) = images(light, &Value::Array(pics), key, target, imgs) { out.push(p); }
    if !files.is_empty() { out.push(div().flex().flex_wrap().gap_1().children(files.into_iter().map(|f| div().px_2().py_1().border_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim")).child(f))).into_any_element()); }
    out
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
    /// Every link and every code block's Copy button as painted in the last frame: ("link:<n>", url, bounds) and
    /// ("copy:<n>", "", bounds), logical pixels, for the check harness (G6).
    pub(crate) fn link_and_copy_rects(&self) -> Vec<(String, String, Bounds<Pixels>)> {
        let r = self.screens.reg.borrow();
        let mut out = Vec::new(); let mut n = 0usize;
        for b in &r.blocks {
            let Some(l) = &b.layout else { continue };
            let lh = l.line_height();
            for (range, link) in &b.links {
                let (Some(a), Some(z)) = (l.position_for_index(range.start), l.position_for_index(range.end)) else { continue };
                // a link that wraps is reported by its first line, to the edge of the text
                let right = if a.y == z.y { z.x } else { l.bounds().right() };
                out.push((format!("link:{n}"), link.href.clone(), Bounds::from_corners(a, point(right, a.y + lh)))); n += 1;
            }
        }
        for (i, bounds) in &r.copies { out.push((format!("copy:{i}"), String::new(), *bounds)); }
        out
    }
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
            2 => { let w = select::word_at(&texts[pos.block].text, pos.off); r.select(Selection { anchor: Pos { block: pos.block, off: w.start }, head: Pos { block: pos.block, off: w.end } }); }
            n if n >= 3 => { if let Some(s) = select::block_range(&texts, pos.block) { r.select(s); } }
            _ => { let s = if e.modifiers.shift { Selection { anchor: r.sel.anchor, head: pos } } else { Selection { anchor: pos, head: pos } }; r.select(s); r.dragging = true; }
        }
        drop(r); cx.notify();
    }
    pub(crate) fn timeline_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.screens.reg.borrow().dragging { return; }
        if let Some(pos) = self.timeline_pos(e.position) { let mut r = self.screens.reg.borrow_mut(); let mut s = r.sel; s.head = pos; r.select(s); drop(r); cx.notify(); }
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
        // a finished selection is the X11 PRIMARY selection too (middle-click paste in other apps)
        #[cfg(target_os = "linux")]
        { let s = { let r = self.screens.reg.borrow(); select::extract(&r.texts(), &r.sel) }; if !s.is_empty() { cx.write_to_primary(ClipboardItem::new_string(s)); } }
        cx.notify();
    }
    /// Ctrl-C with the composer's selection empty: copy what is selected in the timeline.
    pub(crate) fn timeline_copy(&mut self, cx: &mut Context<Self>) {
        let s = { let r = self.screens.reg.borrow(); select::extract(&r.texts(), &r.sel) };
        if !s.is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(s)); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_sent_messages_attachments_split_into_pictures_and_file_chips() {
        let l: Value = serde_json::from_str(r#"[{"label":"a.png","path":"/h/attachments/t1/k-a.png","image":true,"url":"/img?p=1"},{"label":"b.pdf","path":"/h/attachments/t1/k-b.pdf","image":false,"url":""}]"#).unwrap();
        let (p, f) = split_attachments(&l); assert_eq!(p.len(), 1); assert_eq!(f, vec!["b.pdf".to_string()]); assert_eq!(split_attachments(&Value::Null), (vec![], vec![]));
    }
    #[test] fn a_message_that_may_wrap_takes_the_bubbles_full_width() {
        assert!(bubble_full_width(&"word ".repeat(40))); assert!(bubble_full_width(&format!("short\n{}", "x".repeat(61))));
        assert!(!bubble_full_width("hello from the test")); assert!(!bubble_full_width("a\nb\nc")); assert!(!bubble_full_width(&"y".repeat(60)));
    }
    fn info(id: &str, idx: usize, text: &str) -> Info { Info { text: text.into(), entry: id[1..].parse().unwrap_or(0), id: id.into(), idx, links: vec![], layout: None } }
    fn frame(r: &mut Registry, blocks: &[(&str, usize, &str)]) { r.begin(); for (id, idx, t) in blocks { r.blocks.push(info(id, *idx, t)); } r.resolve(); }
    #[test] fn a_selection_follows_its_text_when_blocks_are_added_above() {
        let mut r = Registry::default();
        frame(&mut r, &[("e2", 0, "second"), ("e3", 0, "third message")]);
        r.select(Selection { anchor: Pos { block: 0, off: 2 }, head: Pos { block: 1, off: 5 } }); r.resolve();
        assert_eq!(select::extract(&r.texts(), &r.sel), "cond\n\nthird");
        // older history loads above: every ordinal shifts by two
        frame(&mut r, &[("e0", 0, "old one"), ("e1", 0, "old two"), ("e2", 0, "second"), ("e3", 0, "third message")]);
        assert_eq!(select::extract(&r.texts(), &r.sel), "cond\n\nthird", "same text after the shift");
        assert_eq!((r.sel.anchor.block, r.sel.head.block), (2, 3));
    }
    #[test] fn a_selection_whose_message_is_gone_clears() {
        let mut r = Registry::default();
        frame(&mut r, &[("e2", 0, "second"), ("e3", 0, "third")]);
        r.select(Selection { anchor: Pos { block: 0, off: 0 }, head: Pos { block: 1, off: 3 } }); r.resolve();
        frame(&mut r, &[("e3", 0, "third")]);
        assert!(r.sel.collapsed()); assert_eq!(select::extract(&r.texts(), &r.sel), ""); frame(&mut r, &[("e2", 0, "second"), ("e3", 0, "third")]);
        assert!(r.sel.collapsed(), "it does not come back");
    }
    #[test] fn blocks_in_one_message_are_numbered_from_zero() {
        let mut r = Registry::default(); r.begin();
        for id in ["a", "a", "b", "b", "b"] { r.entry_id = id.into(); let i = r.next_idx(); r.blocks.push(info(id, i, "x")); }
        assert_eq!(r.blocks.iter().map(|b| b.idx).collect::<Vec<_>>(), [0, 1, 0, 1, 2]);
    }
    #[test] fn the_parse_cache_reparses_only_when_the_text_changes() {
        let mut c = ParseCache::default();
        let n: Vec<Value> = serde_json::from_str(r#"[{"tag":"p","kids":[{"text":"hi"}]}]"#).unwrap();
        let a = c.get("e1", "hi", &n); let b = c.get("e1", "hi", &n);
        assert!(Rc::ptr_eq(&a, &b), "unchanged text is not parsed again");
        let d = c.get("e1", "hi there", &n); assert!(!Rc::ptr_eq(&a, &d));
    }
}

//! The timeline's text: Markdown blocks drawn as styled, wrapped, selectable text (selection runs across blocks and
//! messages), links, code blocks with Copy and light highlighting, tables, and images. What to draw comes from the hub's
//! nodes (screens::md); this file only lays them out and answers the pointer. The pure rules are in md.rs and select.rs.
use super::{images::Slot, md::{self, Align, Block, Dim, Img, Inline, Link, Span, Tok}, select::{self, Pos, Selection, Text}};
use crate::{art, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{cell::RefCell, ops::Range, rc::Rc};

/// Everything the pointer needs about the blocks drawn this frame; rebuilt on every render, the selection survives.
pub struct Registry {
    pub blocks: Vec<Info>, pub dragging: bool, pub entry: usize,
    /// The entry being drawn and how many blocks it has had so far: a block's identity is (entry id, index in entry).
    entry_id: String, idx: usize,
    /// The selection as the person made it, by identity; `sel` is that resolved to this frame's ordinals.
    keyed: Option<(KPos, KPos)>, pub sel: Selection,
    /// Where each code block's Copy button was painted this frame (for the check harness), by index.
    pub copies: Vec<(usize, Bounds<Pixels>)>,
    pub copy_seen: usize,
    /// The target of the link under the pointer, shown at the corner of the timeline.
    pub hover: String,
    /// The thread's design names: its parts, pins and nets in a message are links (screens/xprobe.rs).
    pub design: Rc<super::xprobe::Design>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct KPos { pub entry: String, pub idx: usize, pub off: usize }
pub struct Info { pub text: String, pub entry: usize, pub id: String, pub idx: usize, pub links: Vec<(Range<usize>, Link)>, pub layout: Option<TextLayout> }
pub type Reg = Rc<RefCell<Registry>>;
/// The viewer page's code blocks' Copy places are numbered from here (`copy:1000`...), the thread's rows' from 0: each part may draw without the other.
pub const VIEWER_COPIES: usize = 1000;

impl Default for Registry {
    fn default() -> Self { Self { blocks: Vec::new(), dragging: false, entry: 0, entry_id: String::new(), idx: 0, keyed: None, sel: Selection::default(), copies: Vec::new(), copy_seen: 0, hover: String::new(), design: Default::default() } }
}

impl Registry {
    /// A new frame: forget the blocks, keep the selection (by identity).
    pub fn begin(&mut self) { self.blocks.clear(); self.copies.clear(); self.copy_seen = 0; self.idx = 0; self.entry = 0; self.entry_id.clear(); }
    /// The viewer's page draws (or is gone): forget the blocks drawn outside the thread's rows (a review or file page, no entry id) and its
    /// code blocks' Copy places (numbered from VIEWER_COPIES). The rows' stay: their part (part.rs) may reuse its last frame.
    pub fn begin_outside(&mut self) { self.blocks.retain(|b| !b.id.is_empty()); self.copies.retain(|c| c.0 < VIEWER_COPIES); self.copy_seen = VIEWER_COPIES; self.idx = 0; self.entry = 0; self.entry_id.clear(); }
    /// The rows' part draws (or is gone): forget the rows' blocks and Copy places, keep the viewer's.
    pub fn begin_rows(&mut self) { self.blocks.retain(|b| b.id.is_empty()); self.copies.retain(|c| c.0 >= VIEWER_COPIES); self.copy_seen = 0; self.idx = 0; self.entry = 0; self.entry_id.clear(); }
    /// The list draws only the rows on screen, in its own order, and may draw one twice in a frame: the last drawing's blocks stand.
    pub fn enter(&mut self, row: usize, id: &str) { self.blocks.retain(|b| b.id != id); self.entry = row; self.entry_id = id.to_string(); self.idx = 0; }
    fn next_idx(&mut self) -> usize { let i = self.idx; self.idx += 1; i }
    /// The block with this identity, as drawn this frame.
    fn at(&self, id: &str, idx: usize) -> Option<usize> { self.blocks.iter().position(|b| b.id == id && b.idx == idx) }
    fn key(&self, p: Pos) -> Option<KPos> { let b = self.blocks.get(p.block)?; Some(KPos { entry: b.id.clone(), idx: b.idx, off: p.off.min(b.text.len()) }) }
    fn ord(&self, k: &KPos) -> Option<Pos> { self.blocks.iter().position(|b| b.id == k.entry && b.idx == k.idx).map(|block| Pos { block, off: k.off }) }
    /// Make `s` (ordinals of this frame) the selection.
    pub fn select(&mut self, s: Selection) {
        self.keyed = self.key(s.anchor).zip(self.key(s.head));
        self.sel = s;
    }
    /// Move the selection's moving end; its other end stays what the person chose, even scrolled away.
    pub fn extend(&mut self, head: Pos) {
        match (self.keyed.clone(), self.key(head)) { (Some((a, _)), Some(h)) => { self.keyed = Some((a, h)); self.sel.head = head; } _ => self.select(Selection { anchor: head, head }) }
    }
    #[cfg(test)] pub fn resolve(&mut self) { self.resolve_by(|_| None) }
    /// After the frame's blocks are known: in reading order, and the selection by identity as ordinals. An end scrolled off the rows drawn stands at the
    /// edge of them on its side, so what is on screen of the selection still shows. `row_of`: where a row is in the thread; one that is not there clears it.
    pub fn resolve_by(&mut self, row_of: impl Fn(&str) -> Option<usize>) {
        self.blocks.sort_by_key(|b| b.entry);
        let Some((a, h)) = self.keyed.clone() else { self.sel = Selection::default(); return };
        if let Some((anchor, head)) = self.ord(&a).zip(self.ord(&h)) { self.sel = Selection { anchor, head }; return; }
        let Some((ra, rh)) = row_of(&a.entry).zip(row_of(&h.entry)) else { self.keyed = None; self.sel = Selection::default(); return };
        self.sel = self.end(&a, ra).zip(self.end(&h, rh)).map_or(Selection::default(), |(anchor, head)| Selection { anchor, head });
    }
    fn end(&self, k: &KPos, row: usize) -> Option<Pos> {
        if let Some(p) = self.ord(k) { return Some(p); }
        let (first, last) = (self.blocks.first()?, self.blocks.last()?);
        if row < first.entry { Some(Pos { block: 0, off: 0 }) } else if row > last.entry { Some(Pos { block: self.blocks.len() - 1, off: last.text.len() }) } else { None }
    }
    /// The selected text. With an end scrolled away the rows between are read from the thread, whole where they are not drawn.
    pub fn selected(&self, rows: &[serde_json::Value]) -> String {
        let row_of = |id: &str| rows.iter().position(|e| e["id"].as_str() == Some(id));
        let Some((a, h)) = self.keyed.clone() else { return select::extract(&self.texts(), &self.sel) };
        if self.ord(&a).is_some() && self.ord(&h).is_some() { return select::extract(&self.texts(), &self.sel); }
        let Some((ra, rh)) = row_of(&a.entry).zip(row_of(&h.entry)) else { return String::new() };
        let (mut texts, mut anchor, mut head) = (Vec::new(), None, None);
        for row in ra.min(rh)..=ra.max(rh) {
            let id = rows[row]["id"].as_str().unwrap_or("");
            let drawn: Vec<&Info> = self.blocks.iter().filter(|b| b.id == id).collect();
            if drawn.is_empty() {
                let text = rows[row]["text"].as_str().unwrap_or("").to_string(); let len = text.len(); let block = texts.len();
                if row == ra { anchor = Some(Pos { block, off: if ra <= rh { 0 } else { len } }); }
                if row == rh { head = Some(Pos { block, off: if rh < ra { 0 } else { len } }); }
                texts.push(Text { text, entry: row });
            } else {
                for b in drawn {
                    if b.id == a.entry && b.idx == a.idx { anchor = Some(Pos { block: texts.len(), off: a.off }); }
                    if b.id == h.entry && b.idx == h.idx { head = Some(Pos { block: texts.len(), off: h.off }); }
                    texts.push(Text { text: b.text.clone(), entry: row });
                }
            }
        }
        anchor.zip(head).map_or(String::new(), |(anchor, head)| select::extract(&texts, &Selection { anchor, head }))
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
pub struct SelText { spans: Vec<Span>, kind: Kind, light: bool, reg: Reg, id: String, idx: usize, styled: Option<StyledText>, tokens: Vec<(Range<usize>, Tok)> }

impl SelText {
    pub fn new(reg: &Reg, spans: Vec<Span>, kind: Kind, light: bool) -> Self {
        let mut r = reg.borrow_mut();
        let spans = if kind == Kind::Code || r.design.is_empty() { spans } else { super::xprobe::links(spans, &r.design) };
        let mut at = 0usize; let mut links = Vec::new();
        for s in &spans { if let Some(l) = &s.style.link { links.push((at..at + s.text.len(), l.clone())); } at += s.text.len(); }
        let entry = r.entry; let (id, idx) = (r.entry_id.clone(), r.next_idx());
        r.blocks.push(Info { text: md::plain(&spans), entry, id: id.clone(), idx, links, layout: None });
        drop(r);
        Self { spans, kind, light, reg: reg.clone(), id, idx, styled: None, tokens: Vec::new() }
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
            let color = if s.style.link.is_some() { c("accent") } else if self.kind == Kind::Quote || s.style.small { c("dim") } else { c("text") };
            runs.push(TextRun {
                len: s.text.len(), font, color: color.into(),
                background_color: (s.style.code || s.style.kbd).then(|| c("raise").into()),
                underline: s.style.link.is_some().then(|| UnderlineStyle { color: Some(color.into()), thickness: px(1.), wavy: false }),
                strikethrough: s.style.strike.then(|| StrikethroughStyle { color: Some(color.into()), thickness: px(1.) }),
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
        let links: Vec<(Range<usize>, String)> = { let r = self.reg.borrow(); r.at(&self.id, self.idx).map(|o| r.blocks[o].links.iter().map(|(r, l)| (r.clone(), l.href.clone())).collect()).unwrap_or_default() };
        for (r, href) in links {
            if let (Some(pa), Some(pb)) = (layout.position_for_index(r.start), layout.position_for_index(r.end)) {
                let rect = if pa.y == pb.y { Bounds::from_corners(pa, point(pb.x, pb.y + lh)) } else { Bounds::from_corners(point(bounds.left(), pa.y), point(bounds.right(), pb.y + lh)) };
                // (check mode: where each link is, for the e2e drivers that click one)
                /* i18n-ok: an e2e rect name */ crate::art::report(&format!("link:{}", href.replace(['"', '\\'], "")), if pa.y == pb.y { rect } else { Bounds::from_corners(pa, point(bounds.right(), pa.y + lh)) }, window.scale_factor());
                out.push((window.insert_hitbox(rect, HitboxBehavior::Normal), CursorStyle::PointingHand));
            }
        }
        out
    }
    fn paint(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>, bounds: Bounds<Pixels>, rl: &mut (), pre: &mut Vec<(Hitbox, CursorStyle)>, window: &mut Window, cx: &mut App) {
        let Some(styled) = self.styled.as_mut() else { return };
        let layout = styled.layout().clone();
        // the selection first, under the glyphs
        let range = { let r = self.reg.borrow(); r.at(&self.id, self.idx).and_then(|o| r.sel.within(o, r.blocks[o].text.len())) };
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
        let mut r = self.reg.borrow_mut(); if let Some(o) = r.at(&self.id, self.idx) { r.blocks[o].layout = Some(layout); }
    }
}

fn text(reg: &Reg, light: bool, spans: Vec<Span>, kind: Kind) -> AnyElement { SelText::new(reg, spans, kind, light).into_any_element() }
fn plain_spans(s: &str) -> Vec<Span> { vec![Span { text: s.to_string(), style: Default::default() }] }

/// A person's message whose text may wrap gets the bubble's full width (640 px at most) instead of shrinking to its text: a shrink-to-fit
/// parent let the layout measure the wrapping text at one width and paint it at another, so a long message was drawn over what follows it.
/// A message of short lines never wraps and keeps its snug bubble.
/// A message with pictures is laid out in blocks too: in the snug flex bubble the pictures' wrapping row was measured on one line and painted on
/// two, so the second picture spilled out of the bubble over the rows below and the composer.
pub fn bubble_full_width(text: &str, attachments: &Value) -> bool { text.lines().any(|l| l.chars().count() > 60) || !split_attachments(attachments).0.is_empty() }

/// A user's message text.
pub fn user_text(reg: &Reg, light: bool, s: &str, refs: &[String]) -> AnyElement { text(reg, light, md::chips(md::linkify(s), refs), Kind::Body) }

/// An invisible overlay that records where its parent was painted, for the check harness's control rectangles.
fn copy_probe(reg: &Reg, ix: usize) -> impl IntoElement {
    let reg = reg.clone();
    canvas(|b, _, _| b, move |_, b, _, _| { reg.borrow_mut().copies.push((ix, b)); }).absolute().size_full()
}

/// A list: a marker column and the item, its nested lists below it one step in. Bullets change with the level (• ◦ ▪), numbers keep their own count.
fn list_el(cx: &mut Context<Backplane>, reg: &Reg, light: bool, b: &Block, key: &str, level: usize, doc: Option<&Doc>) -> AnyElement {
    let Block::List { ordered, start, items } = b else { return block_el(cx, reg, light, b, key, doc) };
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().gap_1();
    if level == 0 { col = col.pl(px(8.)); }
    for (i, it) in items.iter().enumerate() {
        let mark = match it.checked { Some(true) => "☑".to_string(), Some(false) => "☐".to_string(), None if *ordered => format!("{}.", start + i as u64), None => ["•", "◦", "▪"][level % 3].to_string() };
        let mut body = div().flex_1().min_w_0().flex().flex_col().gap_1();
        if !it.spans.is_empty() || it.sub.is_empty() { body = body.child(text(reg, light, it.spans.clone(), Kind::Body)); }
        for (k, s) in it.sub.iter().enumerate() { body = body.child(list_el(cx, reg, light, s, &format!("{key}-{i}-{k}"), if matches!(s, Block::List { .. }) { level + 1 } else { level }, doc)); }
        col = col.child(div().flex().gap_2().child(div().min_w(px(18.)).flex_shrink_0().text_color(c(if it.checked == Some(true) { "ok" } else { "dim" })).child(mark)).child(body));
    }
    col.into_any_element()
}

fn block_el(cx: &mut Context<Backplane>, reg: &Reg, light: bool, b: &Block, key: &str, doc: Option<&Doc>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    match b {
        Block::Para(s) => text(reg, light, s.clone(), Kind::Body),
        Block::Heading(l, s) => {
            // a document's h1 and h2 are underlined, as GitHub draws them
            let h = div().text_size(px(if doc.is_some() { doc_heading_size(*l) } else { heading_size(*l) })).mt_1().child(text(reg, light, s.clone(), Kind::Head));
            if doc.is_some() && *l <= 2 { h.pb_1().border_b_1().border_color(c("line")).into_any_element() } else { h.into_any_element() }
        }
        Block::Flow(parts, level) => flow_el(cx, reg, light, parts, *level, key, doc, Align::Left),
        Block::Aligned(a, bs) => {
            let mut col = div().w_full().flex().flex_col().gap_2();
            col = match a { Align::Center => col.text_center(), Align::Right => col.text_right(), Align::Left => col };
            for (i, b) in bs.iter().enumerate() {
                let k = format!("{key}-{i}");
                col = col.child(match b { Block::Flow(p, l) => flow_el(cx, reg, light, p, *l, &k, doc, *a), _ => block_el(cx, reg, light, b, &k, doc) });
            }
            col.into_any_element()
        }
        Block::Callout(kind, bs) => {
            let (word, tone) = kind.as_deref().map(alert_of).map_or((None, "line"), |(w, t)| (Some(w), t));
            let mut col = div().border_l_2().border_color(c(tone)).pl_3().flex().flex_col().gap_2();
            if let Some(w) = word { col = col.child(div().font_weight(FontWeight::BOLD).text_color(c(tone)).child(crate::i18n::t(w))); }
            for (i, b) in bs.iter().enumerate() { col = col.child(block_el(cx, reg, light, b, &format!("{key}-{i}"), doc)); }
            col.into_any_element()
        }
        Block::Details { summary, open, blocks } => {
            let k = key.to_string();
            let shown = *open != DETAILS.with(|d| d.borrow().contains(&k));
            let head = div().id(SharedString::from(format!("details-{key}"))).flex().gap_2().items_center().cursor_pointer()
                .child(div().text_color(c("dim")).child(if shown { "▾" } else { "▸" })).child(div().flex_1().min_w_0().child(text(reg, light, summary.clone(), Kind::Body)))
                .on_click(cx.listener(move |_, _, _, cx| { DETAILS.with(|d| { let mut d = d.borrow_mut(); if !d.remove(&k) { d.insert(k.clone()); } }); cx.notify(); }));
            let mut col = div().flex().flex_col().gap_2().child(head);
            if shown { let mut inner = div().pl_4().flex().flex_col().gap_2(); for (i, b) in blocks.iter().enumerate() { inner = inner.child(block_el(cx, reg, light, b, &format!("{key}-{i}"), doc)); } col = col.child(inner); }
            col.into_any_element()
        }
        Block::Anchor(id) => { let id = id.clone(); canvas(|_, _, _| (), move |b, _, _, _| { ANCHORS.with(|a| { a.borrow_mut().insert(id.clone(), b); }); }).w_full().h(px(0.)).into_any_element() }
        Block::Quote(s) => div().border_l_2().border_color(c("line")).pl_3().child(text(reg, light, s.clone(), Kind::Quote)).into_any_element(),
        Block::Rule => div().h(px(1.)).w_full().bg(c("line")).my_1().into_any_element(),
        Block::List { .. } => list_el(cx, reg, light, b, key, 0, doc),
        Block::Code(t, tokens) => {
            let copy = t.clone();
            let copy_ix = { let mut r = reg.borrow_mut(); let n = r.copy_seen; r.copy_seen += 1; n };
            div().w_full().bg(c("raise")).border_1().border_color(c("line")).flex().flex_col()
                .child(div().flex().justify_end().px_2().pt_1().child(div().id(SharedString::from(format!("copy-{key}"))).relative().px_2().text_size(px(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                    .child(crate::i18n::trs("Copy")).child(copy_probe(reg, copy_ix)).on_click(cx.listener(move |_, _, _, cx| { cx.write_to_clipboard(ClipboardItem::new_string(copy.clone())); }))))
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
            if rows.len() > 500 { t = t.child(div().px_2().py_1().text_color(c("dim")).child(crate::i18n::tf("gpui.timeline.more.rows", &[("n", (rows.len() - 500).into())]))); }
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
    parsed.iter().enumerate().map(|(i, b)| block_el(cx, reg, light, b, &format!("{key}-{i}"), None)).collect()
}
/// What a document's pictures need (a README, a note in the file viewer): the image cache, the hub they come from, and the width the page
/// gives the document (a `width="50%"` is a share of it).
pub struct Doc<'a> { pub imgs: &'a super::images::Images, pub target: Option<&'a super::attach::Target>, pub width: f32 }
/// A document's blocks: as `blocks`, with its pictures drawn.
pub fn doc_blocks(cx: &mut Context<Backplane>, reg: &Reg, light: bool, parsed: &[Block], key: &str, doc: &Doc) -> Vec<AnyElement> {
    parsed.iter().enumerate().map(|(i, b)| block_el(cx, reg, light, b, &format!("{key}-{i}"), Some(doc))).collect()
}

/// Where a document's picture is fetched from on the hub: a web address through the hub's cache (`/webimg`), a file through `/img`.
pub fn doc_img_url(src: &str) -> String {
    let l = src.to_ascii_lowercase();
    if l.starts_with("https://") || l.starts_with("http://") { format!("/webimg?url={}", super::attach::enc(src)) } else { format!("/img?path={}", super::attach::enc(src)) }
}
/// The box a document's picture is drawn in: the size it asks for (a share of `avail` for a percentage, its own aspect for the side it leaves
/// out), else its own pixels; never wider than `avail`. None while neither the picture nor its attributes say.
pub fn doc_img_size(img: &Img, natural: Option<(f32, f32)>, avail: f32) -> Option<(f32, f32)> {
    let px_of = |d: Option<Dim>| d.map(|d| match d { Dim::Px(p) => p, Dim::Pct(p) => avail * p / 100. });
    let (w, h) = (px_of(img.width), px_of(img.height));
    let (w, h) = match (w, h, natural) {
        (Some(w), Some(h), _) => (w, h),
        (Some(w), None, Some((nw, nh))) => (w, w * nh / nw.max(1.)),
        (None, Some(h), Some((nw, nh))) => (h * nw / nh.max(1.), h),
        (None, None, Some(n)) => n,
        _ => return None,
    };
    let k = if w > avail && avail > 0. { avail / w } else { 1. };
    Some(((w * k).max(1.), (h * k).max(1.)))
}

thread_local! {
    /// A document's `<details>` the person opened or closed, by block key (each toggles its own default).
    static DETAILS: RefCell<std::collections::HashSet<String>> = RefCell::new(Default::default());
    /// Where each anchor (`#usage`) was painted last: the heading a page's `#` link scrolls to.
    pub static ANCHORS: RefCell<std::collections::HashMap<String, Bounds<Pixels>>> = RefCell::new(Default::default());
}

/// A document's picture: drawn when it has arrived (a link around it opens the link, else a click shows it large); while it loads a box of
/// its size, and only once it comes near the screen is it asked for (a long README's pictures load as it is read).
fn doc_img(cx: &mut Context<Backplane>, light: bool, img: &Img, key: &str, doc: &Doc) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let src = img.source(light).to_string();
    let url = doc_img_url(&src);
    let name = if img.alt.is_empty() { src.rsplit('/').next().unwrap_or("").to_string() } else { img.alt.clone() };
    let slot = doc.imgs.peek(&url);
    let ready = match &slot { Some(Slot::Ready(i)) => Some(i.clone()), _ => None };
    let natural = ready.as_ref().and_then(|i| super::images::dims(&i.bytes)).map(|(w, h)| (w as f32, h as f32));
    let size = doc_img_size(img, natural, doc.width);
    let id = SharedString::from(format!("dimg-{key}"));
    match (ready, slot) {
        (Some(pic), _) => {
            let (bw, bh) = size.unwrap_or((240., 120.));
            let mut frame = div().id(id).w(px(bw)).max_w_full().flex_none(); frame.style().aspect_ratio = Some(bw / bh.max(1.0));
            let (shown, n, link) = (pic.clone(), name.clone(), img.link.clone());
            let src_img = doc.imgs.decoded(&url).map_or(ImageSource::Image(pic), ImageSource::Render);
            frame.overflow_hidden().cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| { match &link { Some(l) => this.open_link(l, cx), None => this.screens.lightbox = Some((shown.clone(), n.clone())) } cx.notify(); }))
                .child(gpui::img(src_img).size_full().object_fit(ObjectFit::Contain)).into_any_element()
        }
        (None, Some(Slot::Failed)) => {
            // GitHub shows the words of a picture that cannot be drawn
            div().px_1().text_size(px(12.)).text_color(c("dim")).border_1().border_color(c("line")).child(name).into_any_element()
        }
        (None, slot) => {
            let (bw, bh) = size.unwrap_or((if img.alt.is_empty() { 120. } else { (img.alt.chars().count() as f32 * 7. + 16.).min(doc.width) }, 22.));
            let asked = slot.is_some();
            let (imgs, target, u) = (doc.imgs.clone(), doc.target.cloned(), url.clone());
            let mut b = div().id(id).relative().w(px(bw)).max_w_full().flex_none().border_1().border_color(c("line")).overflow_hidden().text_size(px(11.)).text_color(c("faint")).px_1().child(name);
            b.style().aspect_ratio = Some(bw / bh.max(1.0));
            b.when(!asked, |b| b.child(canvas(move |bounds, window, _| {
                // near the screen (a screen's height above or below): ask for it, and draw again so the window keeps polling until it arrives
                let view = window.viewport_size(); let margin = view.height;
                if bounds.bottom() > -margin && bounds.top() < view.height + margin { let _ = imgs.get(target.as_ref(), &u); window.refresh(); }
            }, |_, _, _, _| {}).absolute().size_full())).into_any_element()
        }
    }
}

/// A paragraph or heading with pictures: its text and pictures in a row that wraps, aligned as asked.
fn flow_el(cx: &mut Context<Backplane>, reg: &Reg, light: bool, parts: &[Inline], level: u8, key: &str, doc: Option<&Doc>, align: Align) -> AnyElement {
    let mut row = div().w_full().flex().flex_wrap().items_end().gap_1();
    row = match align { Align::Center => row.justify_center(), Align::Right => row.justify_end(), Align::Left => row };
    if level > 0 { row = row.text_size(px(if doc.is_some() { doc_heading_size(level) } else { heading_size(level) })).mt_1(); }
    for (i, p) in parts.iter().enumerate() {
        row = match p {
            Inline::Text(s) => row.child(div().max_w_full().child(text(reg, light, s.clone(), if level > 0 { Kind::Head } else { Kind::Body }))),
            Inline::Img(img) => match doc { Some(d) => row.child(doc_img(cx, light, img, &format!("{key}-{i}"), d)), None => row.child(div().text_color(theme::color(light, "dim")).child(img.alt.clone())) },
            Inline::Break => row.child(div().w_full().h(px(0.))),
        };
    }
    row.into_any_element()
}
fn heading_size(level: u8) -> f32 { match level { 1 => 20., 2 => 17., 3 => 15., _ => 14. } }
/// A document's headings, larger, as GitHub draws them (2em, 1.5em, 1.25em of its text).
fn doc_heading_size(level: u8) -> f32 { match level { 1 => 27., 2 => 21., 3 => 17.5, _ => 15. } }

/// An alert's word (its catalog key) and colour (GitHub's five).
fn alert_of(kind: &str) -> (&'static str, &'static str) {
    match kind { "tip" => ("gpui.md.alert.tip", "ok"), "important" => ("gpui.md.alert.important", "accent"), "warning" => ("gpui.md.alert.warning", "warn"), "caution" => ("gpui.md.alert.caution", "bad"), _ => ("gpui.md.alert.note", "accent") }
}

const MAX_IMAGES: usize = 12;
/// Images a message names: each as its picture when it has arrived, a placeholder while it loads or when it cannot.
pub fn images(cx: &mut Context<Backplane>, light: bool, list: &Value, key: &str, target: Option<&super::attach::Target>, imgs: &super::images::Images) -> Option<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let items = list.as_array().filter(|a| !a.is_empty())?;
    let mut row = div().flex().flex_wrap().gap_2();
    for (i, it) in items.iter().take(MAX_IMAGES).enumerate() {
        let url = it["url"].as_str().unwrap_or("");
        let name = it["path"].as_str().unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
        let el = match imgs.get(target, url) {
            Slot::Ready(img) => { let shown = img.clone(); let n = name.clone();
                let (bw, bh) = match super::images::dims(&img.bytes) { Some((w, h)) => super::images::fit(w, h, 480., 360.), None => (480., 120.) };
                // a box of the picture's own aspect: layout and paint agree, so the picture never spills over the rows below. In a column
                // narrower than the picture (a viewer open beside the thread) the box narrows with it and its height follows, so the picture
                // is drawn whole instead of being cut at the column's edge.
                let (copy, ipath) = (img.clone(), it["path"].as_str().unwrap_or("").to_string());
                let mut frame = div().w(px(bw)).max_w_full().flex_none(); frame.style().aspect_ratio = Some(bw / bh.max(1.0));
                frame.overflow_hidden().cursor_pointer().on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| { cx.stop_propagation(); this.screens.ctx_img = Some(copy.clone()); this.ctx_open(e.position, super::ctxmenu::image_items(&ipath), cx); }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| { this.screens.lightbox = Some((shown.clone(), n.clone())); cx.notify(); }))
                    .child(gpui::img(imgs.decoded(url).map_or(ImageSource::Image(img), ImageSource::Render)).size_full().object_fit(ObjectFit::Contain)).into_any_element() }
            Slot::Loading => div().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("dim")).child(crate::i18n::tf("gpui.timeline.loading", &[("name", name.as_str().into())])).into_any_element(),
            Slot::Failed => div().px_2().py_1().border_1().border_color(c("line")).text_size(px(12.)).text_color(c("bad")).child(crate::i18n::tf("gpui.timeline.could.not.load", &[("name", name.as_str().into())])).into_any_element(),
        };
        row = row.child(div().id(SharedString::from(format!("img-{key}-{i}"))).max_w_full().child(el));
    }
    if items.len() > MAX_IMAGES { row = row.child(div().px_2().py_1().text_size(px(12.)).text_color(c("dim")).child(crate::i18n::tf("gpui.timeline.more.images", &[("n", (items.len() - MAX_IMAGES).into())]))); }
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
pub fn attachments(cx: &mut Context<Backplane>, light: bool, list: &Value, key: &str, target: Option<&super::attach::Target>, imgs: &super::images::Images) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let (pics, files) = split_attachments(list);
    let mut out = Vec::new();
    if let Some(p) = images(cx, light, &Value::Array(pics), key, target, imgs) { out.push(p); }
    if !files.is_empty() { out.push(div().flex().flex_wrap().gap_1().children(files.into_iter().map(|f| div().px_2().py_1().border_1().border_color(c("line")).text_size(px(11.)).text_color(c("dim")).child(f))).into_any_element()); }
    out
}

/// Repaint a few times a second while an image is still arriving, then stop (no standing repaint loop).
pub fn poll(window: &mut Window, cx: &mut Context<Backplane>) {
    let mut seen = None;
    cx.spawn_in(window, async move |weak, cx| loop {
        cx.background_executor().timer(std::time::Duration::from_millis(150)).await;
        // redraw (every part of the window) only when a fetch finished since the last look, not every 150 ms while one is slow;
        // images the cache let go of leave GPUI's memory then too
        let Ok(more) = weak.update_in(cx, |this, window, cx| {
            let done = this.screens.images.done();
            if seen != Some(done) { seen = Some(done); this.screens.images.release(window, cx); cx.notify(); }
            let more = this.screens.images.loading(); if !more { this.screens.img_poll = false; } more
        }) else { return };
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
            if let Some(l) = link { let items = super::ctxmenu::link_items(&l.href, l.file); self.ctx_open(e.position, items, cx); }
            return;
        }
        let mut r = self.screens.reg.borrow_mut();
        let texts = r.texts();
        match e.click_count {
            2 => { let w = select::word_at(&texts[pos.block].text, pos.off); r.select(Selection { anchor: Pos { block: pos.block, off: w.start }, head: Pos { block: pos.block, off: w.end } }); }
            n if n >= 3 => { if let Some(s) = select::block_range(&texts, pos.block) { r.select(s); } }
            _ => { if e.modifiers.shift { r.extend(pos); } else { r.select(Selection { anchor: pos, head: pos }); } r.dragging = true; }
        }
        drop(r); cx.notify();
    }
    pub(crate) fn timeline_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.screens.reg.borrow().dragging {
            let under = self.timeline_pos(e.position).and_then(|p| self.screens.reg.borrow().link_at(p).map(|l| super::linkroute::target(&l.href))).unwrap_or_default();
            if under != self.screens.reg.borrow().hover { self.screens.reg.borrow_mut().hover = under; cx.notify(); }
            return;
        }
        if let Some(pos) = self.timeline_pos(e.position) { self.screens.reg.borrow_mut().extend(pos); cx.notify(); }
    }
    pub(crate) fn timeline_up(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        let was = std::mem::replace(&mut self.screens.reg.borrow_mut().dragging, false);
        if !was { return; }
        // a click without a drag on a link opens it
        let (collapsed, pos) = { let r = self.screens.reg.borrow(); (r.sel.collapsed(), self.timeline_pos(e.position)) };
        let link = pos.filter(|_| collapsed).and_then(|p| self.screens.reg.borrow().link_at(p).cloned());
        if let Some(l) = link {
            self.screens.reg.borrow_mut().hover.clear();
            self.open_link(&l, cx);
        }
        // a finished selection is the X11 PRIMARY selection too (middle-click paste in other apps)
        #[cfg(target_os = "linux")]
        { let s = self.timeline_selected(); if !s.is_empty() { cx.write_to_primary(ClipboardItem::new_string(s)); } }
        cx.notify();
    }
    /// Open a link of the timeline or a document: a file in the viewer, a web address in the browser, a document's `#anchor` scrolled to.
    pub(crate) fn open_link(&mut self, l: &Link, cx: &mut Context<Self>) {
        if let Some(id) = l.href.strip_prefix('#') { self.jump_to_anchor(id); cx.notify(); return; }
        match super::linkroute::route(&l.href, l.file) {
            super::linkroute::Route::File(p) => { self.note.clear(); self.link_file(&p, false, cx) }
            super::linkroute::Route::Web(u) => cx.open_url(&u),
            super::linkroute::Route::Hub(u) => { self.note.clear(); self.go("link-open", u, cx); }
            super::linkroute::Route::Note(n) => self.note = n.to_string(),
            super::linkroute::Route::Design(w) => super::xprobe::clicked(self, &w),
        }
    }
    /// Scroll the document on screen (the project's README, or a note in the file viewer) so the anchor `id` is at its top.
    fn jump_to_anchor(&mut self, id: &str) {
        let Some(at) = ANCHORS.with(|a| a.borrow().get(id).copied()) else { return };
        for h in [&self.screens.home_scroll, &self.screens.files.preview_scroll] {
            let view = h.bounds();
            if view.size.height <= px(0.) || at.left() < view.left() || at.left() > view.right() { continue }
            let off = h.offset();
            let y = (off.y - (at.top() - view.top()) + px(8.)).min(px(0.)).max(-h.max_offset().height);
            h.set_offset(point(off.x, y));
            return;
        }
    }
    /// Ctrl-C with the composer's selection empty: copy what is selected in the timeline.
    pub(crate) fn timeline_copy(&mut self, cx: &mut Context<Self>) {
        let s = self.timeline_selected();
        if !s.is_empty() { cx.write_to_clipboard(ClipboardItem::new_string(s)); }
    }
    fn timeline_selected(&self) -> String { self.screens.reg.borrow().selected(self.data["thread"]["entries"].as_array().map_or(&[], |r| r.as_slice())) }
}

/// How far ahead of the reader rows are measured and history is asked for: about two screens, the web's runway. The two are one number on purpose: the
/// list counts only rows it has measured, so a reader this near the top of the measured rows is this near the top of the rows held.
pub const RUNWAY: f32 = 1600.;
/// `y`: how far the list is scrolled from the top of the rows measured (past `max` at its end, where it is `max`); `max` its range.
pub fn near_top(y: f32, max: f32) -> bool { y.min(max) < RUNWAY }

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_document_picture_takes_the_size_it_asks_for_and_never_more_than_the_page() {
        let img = |w: Option<Dim>, h: Option<Dim>| Img { width: w, height: h, ..Default::default() };
        assert_eq!(doc_img_size(&img(None, None), Some((1200., 600.)), 800.), Some((800., 400.)));
        assert_eq!(doc_img_size(&img(Some(Dim::Px(300.)), None), Some((1200., 600.)), 800.), Some((300., 150.)));
        assert_eq!(doc_img_size(&img(Some(Dim::Pct(50.)), None), Some((100., 100.)), 800.), Some((400., 400.)));
        assert_eq!(doc_img_size(&img(Some(Dim::Px(90.)), Some(Dim::Px(20.))), None, 800.), Some((90., 20.)));
        assert_eq!(doc_img_size(&img(None, None), None, 800.), None);
        assert_eq!(doc_img_url("https://x.io/a b.svg"), format!("/webimg?url={}", super::super::attach::enc("https://x.io/a b.svg")));
        assert!(doc_img_url("/p/img/a.png").starts_with("/img?path="));
    }
    #[test] fn history_is_asked_for_within_the_runway_of_the_top() {
        assert!(near_top(0., 5000.)); assert!(near_top(RUNWAY - 1., 5000.)); assert!(!near_top(RUNWAY, 5000.));
        assert!(near_top(9000., 300.), "at the end of a short list, past its range");
    }
    #[test] fn a_sent_messages_attachments_split_into_pictures_and_file_chips() {
        let l: Value = serde_json::from_str(r#"[{"label":"a.png","path":"/h/attachments/t1/k-a.png","image":true,"url":"/img?p=1"},{"label":"b.pdf","path":"/h/attachments/t1/k-b.pdf","image":false,"url":""}]"#).unwrap();
        let (p, f) = split_attachments(&l); assert_eq!(p.len(), 1); assert_eq!(f, vec!["b.pdf".to_string()]); assert_eq!(split_attachments(&Value::Null), (vec![], vec![]));
    }
    #[test] fn a_message_that_may_wrap_takes_the_bubbles_full_width() {
        let none = Value::Null;
        assert!(bubble_full_width(&"word ".repeat(40), &none)); assert!(bubble_full_width(&format!("short\n{}", "x".repeat(61)), &none));
        assert!(!bubble_full_width("hello from the test", &none)); assert!(!bubble_full_width("a\nb\nc", &none)); assert!(!bubble_full_width(&"y".repeat(60), &none));
        // pictures: block layout, so a wrapped second picture is inside the bubble's height; a file chip alone keeps the snug bubble
        assert!(bubble_full_width("add these", &serde_json::json!([{"image": true, "url": "/a.png", "path": "a.png"}, {"image": true, "url": "/b.png", "path": "b.png"}])));
        assert!(!bubble_full_width("add these", &serde_json::json!([{"label": "notes.txt", "path": "notes.txt"}])));
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
    #[test] fn the_root_and_the_rows_part_forget_only_their_own_blocks() {
        // the rows' part may reuse its last frame while the root draws again (part.rs): the root's frame keeps the rows' blocks, and the other way round
        let outside = |t: &str| Info { text: t.into(), entry: 0, id: String::new(), idx: 0, links: vec![], layout: None };
        let mut r = Registry::default(); r.begin();
        r.blocks.push(outside("review")); r.blocks.push(info("e1", 0, "one")); r.blocks.push(info("e2", 0, "two"));
        r.begin_outside(); assert_eq!(r.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>(), ["one", "two"]);
        r.blocks.push(outside("review again")); r.begin_rows(); assert_eq!(r.blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>(), ["review again"]);
    }
    #[test] fn a_row_drawn_twice_in_a_frame_stands_once() {
        let mut r = Registry::default(); r.begin();
        for _ in 0..2 { r.enter(3, "e3"); let i = r.next_idx(); r.blocks.push(Info { idx: i, ..info("e3", i, "hello") }); }
        assert_eq!(r.blocks.len(), 1); assert_eq!(r.at("e3", 0), Some(0));
    }
    #[test] fn blocks_read_in_thread_order_however_the_list_drew_them() {
        let mut r = Registry::default(); r.begin();
        for (row, id) in [(7, "e7"), (2, "e2")] { r.enter(row, id); r.blocks.push(Info { entry: row, ..info(id, 0, id) }); }
        r.resolve(); assert_eq!(r.blocks[0].id, "e2");
    }
    #[test] fn a_selection_scrolled_off_keeps_what_is_on_screen_and_its_other_end() {
        let rows = |id: &str| ["e1", "e2", "e3"].iter().position(|x| *x == id);
        let mut r = Registry::default(); frame(&mut r, &[("e1", 0, "one"), ("e2", 0, "two")]);
        r.select(Selection { anchor: Pos { block: 0, off: 1 }, head: Pos { block: 1, off: 2 } });
        r.begin(); r.blocks.push(info("e2", 0, "two")); r.blocks.push(info("e3", 0, "three")); r.resolve_by(rows);
        assert_eq!(r.sel, Selection { anchor: Pos { block: 0, off: 0 }, head: Pos { block: 0, off: 2 } });
        r.extend(Pos { block: 1, off: 3 });
        assert_eq!(r.keyed.as_ref().map(|(a, _)| (a.entry.as_str(), a.off)), Some(("e1", 1)), "the end scrolled away is not moved");
    }
    #[test] fn copying_a_selection_scrolled_off_reads_the_rows_not_drawn() {
        let thread: Vec<Value> = ["one", "two", "three"].iter().enumerate().map(|(i, t)| serde_json::json!({"id": format!("e{}", i + 1), "text": t})).collect();
        let mut r = Registry::default(); frame(&mut r, &[("e1", 0, "one"), ("e3", 0, "three")]);
        r.select(Selection { anchor: Pos { block: 0, off: 1 }, head: Pos { block: 1, off: 2 } });
        r.begin(); r.blocks.push(info("e3", 0, "three")); r.resolve_by(|id| ["e1", "e2", "e3"].iter().position(|x| *x == id));
        let s = r.selected(&thread); assert!(s.starts_with("one") && s.contains("two") && s.ends_with("th"), "{s}");
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
        for (row, (id, n)) in [("a", 2), ("b", 3)].into_iter().enumerate() { r.enter(row, id); for _ in 0..n { let i = r.next_idx(); r.blocks.push(info(id, i, "x")); } }
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

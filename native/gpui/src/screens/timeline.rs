//! The timeline's text: Markdown blocks drawn as styled, wrapped, selectable text (selection runs across blocks and
//! messages), links, code blocks with Copy and light highlighting, tables, and images. What to draw comes from the hub's
//! nodes (screens::md); this file only lays them out and answers the pointer. The pure rules are in md.rs and select.rs.
use super::{images::Slot, md::{self, Align, Block, Dim, Img, Inline, Link, ModelChip, Span, Tok, UserSeg}, select::{self, Pos, Selection, Text}};
use crate::{art, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{cell::RefCell, ops::Range, rc::Rc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DragMode { #[default] Char, Word, Block }

/// Everything the pointer needs about the blocks drawn this frame; rebuilt on every render, the selection survives.
pub struct Registry {
    pub blocks: Vec<Info>, pub dragging: bool, pub entry: usize,
    /// Monotonic counter incremented on every mouse-down so a delayed single-click copy cancels when a second click arrives.
    pub click_seq: u64,
    /// Which surface started the active drag (`Some(true)` = timeline rows, `Some(false)` = doc/outside blocks).
    pub drag_surface: Option<bool>,
    drag_mode: DragMode,
    drag_origin: Option<(KPos, KPos)>,
    pub drag_pos: Option<Point<Pixels>>,
    pub drag_scrolling: bool,
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
    /// The window's app, for a command's Run in terminal (set each frame by the part that draws the rows).
    pub app: Option<WeakEntity<Backplane>>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct KPos { pub entry: String, pub idx: usize, pub off: usize }
pub struct Info { pub text: String, pub entry: usize, pub id: String, pub idx: usize, pub links: Vec<(Range<usize>, Link)>, pub copy_spans: Vec<(Range<usize>, String, SharedString)>, pub layout: Option<BlockLayout> }
pub type Reg = Rc<RefCell<Registry>>;
/// The viewer page's code blocks' Copy places are numbered from here (`copy:1000`...), the thread's rows' from 0: each part may draw without the other.
pub const VIEWER_COPIES: usize = 1000;

impl Default for Registry {
    fn default() -> Self {
        Self {
            blocks: Vec::new(), dragging: false, entry: 0, click_seq: 0,
            drag_surface: None, drag_mode: DragMode::Char, drag_origin: None, drag_pos: None, drag_scrolling: false,
            entry_id: String::new(), idx: 0, keyed: None, sel: Selection::default(), copies: Vec::new(), copy_seen: 0,
            hover: String::new(), design: Default::default(), app: None,
        }
    }
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
    /// Extend an active drag according to its mode (character, double-click word, or triple-click block).
    pub fn extend_drag(&mut self, pos: Pos) {
        let Some(b) = self.blocks.get(pos.block) else { return };
        let (target_start, target_end) = match self.drag_mode {
            DragMode::Char => { self.extend(pos); return; }
            DragMode::Word => {
                let w = select::word_at(&b.text, pos.off);
                (Pos { block: pos.block, off: w.start }, Pos { block: pos.block, off: w.end })
            }
            DragMode::Block => (Pos { block: pos.block, off: 0 }, Pos { block: pos.block, off: b.text.len() }),
        };
        let Some((oa, ob)) = self.drag_origin.clone() else { self.select(Selection { anchor: target_start, head: target_end }); return; };
        let before = self.ord(&oa).map_or_else(|| pos < self.sel.anchor, |a| pos < a);
        let (anchor_k, head_p) = if before { (ob, target_start) } else { (oa, target_end) };
        if let Some(head_k) = self.key(head_p) {
            let anchor_p = self.ord(&anchor_k).unwrap_or(self.sel.anchor);
            self.keyed = Some((anchor_k, head_k));
            self.sel = Selection { anchor: anchor_p, head: head_p };
        }
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
    pub fn hit(&self, p: Point<Pixels>) -> Option<Pos> { self.hit_scoped(p, None) }
    /// Like `hit`, optionally restricted to timeline row blocks (`Some(true)`) or doc/outside blocks (`Some(false)`).
    pub fn hit_scoped(&self, p: Point<Pixels>, rows_only: Option<bool>) -> Option<Pos> {
        let matches = |b: &Info| match rows_only {
            Some(true) => !b.id.is_empty() && !b.id.starts_with("home-readme"),
            Some(false) => b.id.is_empty() || b.id.starts_with("home-readme"),
            None => true,
        };
        // the block under the point first (a table's cells share a row, so distance alone would pick the first), else the nearest
        let inside = self.blocks.iter().position(|b| matches(b) && b.layout.as_ref().map_or(false, |l| l.bounds().contains(&p)));
        let i = match inside {
            Some(i) => i,
            None => {
                let mut best: Option<(Pixels, Pixels, usize)> = None;
                for (i, b) in self.blocks.iter().enumerate() {
                    if !matches(b) { continue; }
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
        let off = if p.y < r.top() { 0 } else if p.y > r.bottom() { len } else { match l.closest_index_for_position(p) { Ok(o) | Err(o) => o.min(len) } };
        Some(Pos { block: i, off })
    }
    pub fn link_at(&self, pos: Pos) -> Option<&Link> { self.blocks.get(pos.block)?.links.iter().find(|(r, _)| r.contains(&pos.off)).map(|(_, l)| l) }
    pub fn copy_span_at(&self, pos: Pos) -> Option<(String, SharedString)> { self.blocks.get(pos.block)?.copy_spans.iter().find(|(r, _, _)| r.contains(&pos.off)).map(|(_, t, id)| (t.clone(), id.clone())) }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind { Body, Quote, Code, Head }

/// A block of text that paints its own selection and reports its layout to the registry.
pub struct SelText { spans: Vec<Span>, kind: Kind, light: bool, reg: Reg, id: String, idx: usize, styled: Option<StyledText>, tokens: Vec<(Range<usize>, Tok)>,
    /// The code spans with a copy button (md::CopyAs): where they are in the text, what they copy, and their buttons while shown.
    chips: Vec<Chip>,
    /// The text ranges drawn on a shade (a code span, a key, a Settings link's button): painted under the selection, which is under the glyphs.
    shaded: Vec<Range<usize>> }
/// The ranges of `spans` drawn on a shade (`code`, `kbd`, a link into Settings or a plan, drawn as a button).
pub fn shaded(spans: &[Span]) -> Vec<Range<usize>> {
    let mut out = Vec::new(); let mut at = 0usize;
    for s in spans {
        let button = s.style.link.as_ref().is_some_and(|l| l.href.starts_with("backplane://settings/") || l.href.starts_with("backplane://plan/"));
        if !s.text.is_empty() && (s.style.code || s.style.kbd || button) { out.push(at..at + s.text.len()); }
        at += s.text.len();
    }
    out
}
/// The rectangles a text range covers, line by line, from `a` (its start's position) to `b` (its end's), lines `lh` tall in text `left`..`right`.
pub fn line_rects(a: Point<Pixels>, b: Point<Pixels>, lh: Pixels, left: Pixels, right: Pixels) -> Vec<Bounds<Pixels>> {
    if a.y == b.y { return vec![Bounds::from_corners(a, point(b.x, b.y + lh))] }
    let mut out = vec![Bounds::from_corners(a, point(right, a.y + lh))];
    let mut y = a.y + lh;
    while y < b.y { out.push(Bounds::from_corners(point(left, y), point(right, y + lh))); y += lh; }
    out.push(Bounds::from_corners(point(left, b.y), point(b.x, b.y + lh)));
    out
}
struct Chip { range: Range<usize>, text: SharedString, cmd: bool, id: SharedString, el: Option<AnyElement>, hot: Vec<Bounds<Pixels>>, shown: bool }

/// The room a code span's buttons take after its end (they show over what follows while the pointer is there): one icon, or two for a command.
fn chip_room(cmd: bool) -> f32 { if cmd { 52. } else { 28. } }
/// Where a span from `a` to `b` (the text positions of its ends, lines `lh` tall) is drawn, line by line, in text `left`..`right`, and the room
/// after its end where its buttons go: the pointer anywhere over these keeps them shown.
pub fn chip_hot(a: Point<Pixels>, b: Point<Pixels>, lh: Pixels, left: Pixels, right: Pixels, room: Pixels) -> Vec<Bounds<Pixels>> {
    let mut out = line_rects(a, b, lh, left, right);
    out.push(Bounds::from_corners(point(b.x, b.y - px(3.)), point(b.x + room, b.y + lh + px(3.))));
    out
}

impl SelText {
    pub fn new(reg: &Reg, spans: Vec<Span>, kind: Kind, light: bool) -> Self {
        let mut r = reg.borrow_mut();
        let spans = if kind == Kind::Code || r.design.is_empty() { spans } else { super::xprobe::links(spans, &r.design) };
        let mut at = 0usize; let mut links = Vec::new();
        for s in &spans { if let Some(l) = &s.style.link { links.push((at..at + s.text.len(), l.clone())); } at += s.text.len(); }
        let entry = r.entry; let (id, idx) = (r.entry_id.clone(), r.next_idx());
        let mut chips = Vec::new(); let mut copy_spans = Vec::new(); let mut at = 0usize;
        for s in &spans {
            // every code span and bare file-path link copies (the hub's `copy` says what it is; a command also gets Run in terminal), its button shown on hover
            let is_file = s.style.link.as_ref().is_some_and(|l| l.file);
            if (s.style.copy.is_some() || s.style.code || is_file) && !s.text.trim().is_empty() {
                let k = s.style.copy;
                let cid = SharedString::from(format!("ic-{id}-{idx}-{at}"));
                if s.style.link.is_none() && (s.style.copy.is_some() || s.style.code) {
                    copy_spans.push((at..at + s.text.len(), s.text.clone(), cid.clone()));
                }
                chips.push(Chip { range: at..at + s.text.len(), text: s.text.clone().into(), cmd: k == Some(md::CopyAs::Cmd), id: cid, el: None, hot: vec![], shown: false });
            }
            at += s.text.len();
        }
        r.blocks.push(Info { text: md::plain(&spans), entry, id: id.clone(), idx, links, copy_spans, layout: None });
        drop(r);
        let shaded = if kind == Kind::Code { Vec::new() } else { shaded(&spans) };
        Self { spans, kind, light, reg: reg.clone(), id, idx, styled: None, tokens: Vec::new(), chips, shaded }
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
            if runs.is_empty() {
                let len = self.spans.iter().map(|s| s.text.len()).sum();
                runs.push(TextRun { len, font: mono, color: c("text").into(), background_color: None, underline: None, strikethrough: None });
            }
            return runs;
        }
        for s in &self.spans {
            if s.text.is_empty() { continue; }
            let mut font = base.font();
            if s.style.bold || self.kind == Kind::Head { font.weight = FontWeight::BOLD; }
            if s.style.italic { font.style = FontStyle::Italic; }
            if s.style.code { font = mono.clone(); }
            // a link into Settings (an agent's "Open Settings › CAD"): drawn as a button, a filled label with no underline
            let button = s.style.link.as_ref().is_some_and(|l| l.href.starts_with("backplane://settings/") || l.href.starts_with("backplane://plan/"));
            if button { font.weight = FontWeight::BOLD; }
            let color = if s.style.link.is_some() { c("accent") } else if self.kind == Kind::Quote || s.style.small { c("dim") } else { c("text") };
            runs.push(TextRun {
                len: s.text.len(), font, color: color.into(),
                // a code span's shade is painted under the selection by `paint` (shaded), not by the text: a run's own background covered the highlight
                background_color: None,
                underline: (s.style.link.is_some() && !button).then(|| UnderlineStyle { color: Some(color.into()), thickness: px(1.), wavy: false }),
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
        // a code span's copy button (and Run in terminal for a command), after its end, while the pointer is over it or a button has the keyboard
        let mouse = window.mouse_position(); let app = self.reg.borrow().app.clone(); let light = self.light;
        for ch in self.chips.iter_mut() {
            let (Some(pa), Some(pb)) = (layout.position_for_index(ch.range.start), layout.position_for_index(ch.range.end)) else { ch.el = None; continue };
            ch.hot = chip_hot(pa, pb, lh, bounds.left(), bounds.right(), px(chip_room(ch.cmd)));
            let run_id = SharedString::from(format!("run:{}", ch.id));
            ch.shown = ch.hot.iter().any(|h| h.contains(&mouse)) || super::copyable::focused(&ch.id, window) || super::copyable::focused(&run_id, window) || super::copyable::just_copied(&ch.id)
                || crate::art::rects_on();   // check runs draw every button, so the window tests find them where the pointer has not been
            if !ch.shown { ch.el = None; continue }
            let c = |n: &str| theme::color(light, n);
            let mut bar = div().flex().items_center().gap_px().bg(c("bg")).border_1().border_color(c("line")).shadow_sm();
            if let (true, Some(app)) = (ch.cmd, app.clone()) { bar = bar.child(super::copyable::run_button(light, ch.id.clone(), ch.text.clone(), false, app, cx)); }
            bar = bar.child(super::copyable::button(light, ch.id.clone(), ch.text.clone(), super::copyable::Opts { word: false, always: true }, cx));
            let mut el = bar.into_any_element();
            let size = el.layout_as_root(size(AvailableSpace::MinContent, AvailableSpace::MinContent), window, cx);
            let vw = window.viewport_size().width;
            let x = (pb.x + px(2.)).min(vw - size.width - px(4.)).max(px(0.));
            el.prepaint_at(point(x, pb.y + (lh - size.height) / 2.), window, cx);
            ch.el = Some(el);
        }
        out
    }
    fn paint(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>, bounds: Bounds<Pixels>, rl: &mut (), pre: &mut Vec<(Hitbox, CursorStyle)>, window: &mut Window, cx: &mut App) {
        let Some(styled) = self.styled.as_mut() else { return };
        let layout = styled.layout().clone();
        // in order: a code span's shade, the selection over it, then the glyphs (a run's own background would cover the selection)
        let (lh, left, right) = (layout.line_height(), bounds.left(), bounds.right());
        let shade = theme::color(self.light, "raise");
        for r in &self.shaded {
            if let (Some(pa), Some(pb)) = (layout.position_for_index(r.start), layout.position_for_index(r.end)) { for q in line_rects(pa, pb, lh, left, right) { window.paint_quad(fill(q, shade)); } }
        }
        let range = { let r = self.reg.borrow(); r.at(&self.id, self.idx).and_then(|o| r.sel.within(o, r.blocks[o].text.len())) };
        if let Some(range) = range {
            let tint = select::highlight(self.light);
            if let (Some(pa), Some(pb)) = (layout.position_for_index(range.start), layout.position_for_index(range.end)) { for q in line_rects(pa, pb, lh, left, right) { window.paint_quad(fill(q, tint)); } }
        }
        styled.paint(id, inspector, bounds, rl, &mut (), window, cx);
        for (h, style) in pre.iter() { window.set_cursor_style(*style, h); }
        for ch in self.chips.iter_mut() { if let Some(el) = ch.el.as_mut() { el.paint(window, cx); } }
        // the pointer coming onto a code span or leaving it draws the frame again, which shows or hides its buttons
        let hot: Vec<(Vec<Bounds<Pixels>>, bool)> = self.chips.iter().filter(|c| !c.hot.is_empty()).map(|c| (c.hot.clone(), c.shown)).collect();
        if !hot.is_empty() && !crate::art::rects_on() {
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _| {
                if phase != DispatchPhase::Bubble { return }
                if hot.iter().any(|(h, shown)| h.iter().any(|b| b.contains(&e.position)) != *shown) { window.refresh(); }
            });
        }
        let mut r = self.reg.borrow_mut(); if let Some(o) = r.at(&self.id, self.idx) { r.blocks[o].layout = Some(BlockLayout::Text(layout)); }
    }
}

/// Stored layout of one painted text block (plain `StyledText` or inline-chip `FlowText`).
#[derive(Clone)]
pub enum BlockLayout {
    Text(TextLayout),
    Flow(FlowLayout),
}

impl BlockLayout {
    pub fn bounds(&self) -> Bounds<Pixels> {
        match self { Self::Text(l) => l.bounds(), Self::Flow(l) => l.bounds }
    }
    pub fn line_height(&self) -> Pixels {
        match self { Self::Text(l) => l.line_height(), Self::Flow(l) => l.line_height }
    }
    pub fn index_for_position(&self, p: Point<Pixels>) -> Result<usize, usize> {
        match self { Self::Text(l) => l.index_for_position(p), Self::Flow(l) => l.index_for_position(p) }
    }
    pub fn closest_index_for_position(&self, p: Point<Pixels>) -> Result<usize, usize> {
        match self { Self::Text(l) => l.closest_index_for_position(p), Self::Flow(l) => l.index_for_position(p) }
    }
    pub fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        match self { Self::Text(l) => l.position_for_index(index), Self::Flow(l) => l.position_for_index(index) }
    }
}

#[derive(Clone)]
pub enum FlowItem {
    Text { x: Pixels, text_y: Pixels, range: Range<usize>, shaped: ShapedLine },
    Chip { rect: Bounds<Pixels>, range: Range<usize>, amp: ShapedLine, label: ShapedLine, kind: ShapedLine, amp_x: Pixels, label_x: Pixels, kind_x: Pixels },
}

#[derive(Clone)]
pub struct FlowLine {
    pub top: Pixels,
    pub height: Pixels,
    pub text_y: Pixels,
    pub range: Range<usize>,
    pub has_chip: bool,
    pub items: Vec<FlowItem>,
}

#[derive(Clone)]
pub struct FlowLayout {
    pub bounds: Bounds<Pixels>,
    pub line_height: Pixels,
    pub text_len: usize,
    pub lines: Vec<FlowLine>,
}

impl FlowLayout {
    pub fn index_for_position(&self, p: Point<Pixels>) -> Result<usize, usize> {
        if self.lines.is_empty() || p.y < self.bounds.top() { return Err(0) }
        for vl in &self.lines {
            if p.y <= vl.top + vl.height {
                let Some(first) = vl.items.first() else { return Err(vl.range.start) };
                let first_x = match first { FlowItem::Text { x, .. } => *x, FlowItem::Chip { rect, .. } => rect.left() };
                if p.x < first_x { return Err(vl.range.start) }
                for it in &vl.items {
                    match it {
                        FlowItem::Text { x, range, shaped, .. } => {
                            if p.x <= *x + shaped.width {
                                let off = shaped.closest_index_for_x((p.x - *x).max(px(0.)));
                                return Ok((range.start + off).min(range.end));
                            }
                        }
                        FlowItem::Chip { rect, range, .. } => {
                            if p.x <= rect.right() {
                                let mid = rect.left() + rect.size.width / 2.;
                                return Ok(if p.x < mid { range.start } else { range.end });
                            }
                        }
                    }
                }
                return Err(vl.range.end);
            }
        }
        Err(self.text_len)
    }

    pub fn position_for_index(&self, index: usize) -> Option<Point<Pixels>> {
        if index > self.text_len { return None }
        for vl in &self.lines {
            if index < vl.range.start { return Some(point(self.bounds.left(), vl.top + vl.text_y)) }
            if index <= vl.range.end {
                for it in &vl.items {
                    match it {
                        FlowItem::Text { x, text_y, range, shaped } if index >= range.start && index <= range.end => {
                            return Some(point(*x + shaped.x_for_index(index - range.start), vl.top + *text_y));
                        }
                        FlowItem::Chip { rect, range, .. } if index >= range.start && index <= range.end => {
                            let x = if index == range.end { rect.right() } else { rect.left() };
                            return Some(point(x, vl.top + vl.text_y));
                        }
                        _ => {}
                    }
                }
                return Some(point(self.bounds.left(), vl.top + vl.text_y));
            }
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
struct WrapAtom {
    width: f32,
    is_space: bool,
    is_chip: bool,
    glue_prev: bool,
}

/// Groups wrappable atoms into visual lines `(atom_range, has_chip)` under optional `wrap_w`, keeping
/// non-space clusters (e.g. a chip and adjacent punctuation or brackets) on one line and leaving
/// trailing spaces on the line before a soft wrap.
fn wrap_flow_lines(atoms: &[WrapAtom], wrap_w: Option<f32>) -> Vec<(Range<usize>, bool)> {
    if atoms.is_empty() { return vec![(0..0, false)] }
    let Some(max_w) = wrap_w else {
        return vec![(0..atoms.len(), atoms.iter().any(|a| a.is_chip))];
    };
    struct Cluster { range: Range<usize>, width: f32, is_space: bool, has_chip: bool }
    let mut clusters: Vec<Cluster> = Vec::new();
    for (i, a) in atoms.iter().enumerate() {
        if a.glue_prev && clusters.last().is_some_and(|c| !c.is_space) && !a.is_space {
            let c = clusters.last_mut().unwrap();
            c.range.end = i + 1;
            c.width += a.width;
            c.has_chip |= a.is_chip;
        } else {
            clusters.push(Cluster { range: i..i + 1, width: a.width, is_space: a.is_space, has_chip: a.is_chip });
        }
    }
    let mut out = Vec::new();
    let (mut line_start, mut line_end, mut cur_w, mut has_chip) = (0usize, 0usize, 0.0f32, false);
    for cl in clusters {
        let empty = line_start == line_end;
        if empty {
            line_start = cl.range.start;
            line_end = cl.range.end;
            cur_w = cl.width;
            has_chip = cl.has_chip;
            continue;
        }
        if cl.is_space {
            line_end = cl.range.end;
            cur_w += cl.width;
            continue;
        }
        if cur_w + cl.width > max_w + 0.01 {
            out.push((line_start..line_end, has_chip));
            line_start = cl.range.start;
            line_end = cl.range.end;
            cur_w = cl.width;
            has_chip = cl.has_chip;
        } else {
            line_end = cl.range.end;
            cur_w += cl.width;
            has_chip |= cl.has_chip;
        }
    }
    out.push((line_start..line_end, has_chip));
    out
}

#[derive(Clone)]
enum RawPiece {
    Span { text: String, style: md::Style, range: Range<usize> },
    Chip { label: String, range: Range<usize> },
}

#[derive(Clone)]
struct HardLine {
    range: Range<usize>,
    pieces: Vec<RawPiece>,
}

#[derive(Clone)]
enum AtomKind {
    Text { text: String, style: md::Style, shaped: ShapedLine },
    Chip { amp: ShapedLine, label: ShapedLine, kind: ShapedLine, width: Pixels, height: Pixels, pad_x: Pixels, gap: Pixels },
}

#[derive(Clone)]
struct MeasuredAtom {
    range: Range<usize>,
    width: f32,
    is_space: bool,
    is_chip: bool,
    glue_prev: bool,
    kind: AtomKind,
}

#[derive(Clone)]
enum BuiltItem {
    Text { x: Pixels, range: Range<usize>, shaped: ShapedLine },
    Chip { x: Pixels, w: Pixels, h: Pixels, pad_x: Pixels, gap: Pixels, range: Range<usize>, amp: ShapedLine, label: ShapedLine, kind: ShapedLine },
}

#[derive(Clone)]
struct BuiltLine {
    top: Pixels,
    height: Pixels,
    text_y: Pixels,
    range: Range<usize>,
    has_chip: bool,
    items: Vec<BuiltItem>,
}

#[derive(Clone)]
struct BuiltFlow {
    wrap_w: Option<f32>,
    size: Size<Pixels>,
    lh: Pixels,
    lines: Vec<BuiltLine>,
}

fn span_run(style: &md::Style, base_font: &Font, light: bool) -> TextRun {
    let c = |n: &str| theme::color(light, n);
    let mut font = base_font.clone();
    if style.bold { font.weight = FontWeight::BOLD; }
    if style.italic { font.style = FontStyle::Italic; }
    if style.code { font = art::mono_font(); }
    let button = style.link.as_ref().is_some_and(|l| l.href.starts_with("backplane://settings/") || l.href.starts_with("backplane://plan/"));
    if button { font.weight = FontWeight::BOLD; }
    let color = if style.link.is_some() { c("accent") } else if style.small { c("dim") } else { c("text") };
    TextRun {
        len: 0,
        font,
        color: color.into(),
        background_color: None,
        underline: (style.link.is_some() && !button).then(|| UnderlineStyle { color: Some(color.into()), thickness: px(1.), wavy: false }),
        strikethrough: style.strike.then(|| StrikethroughStyle { color: Some(color.into()), thickness: px(1.) }),
    }
}

fn shape_text_piece(text: &str, style: &md::Style, base_font: &Font, font_size: Pixels, light: bool, window: &Window) -> ShapedLine {
    let mut run = span_run(style, base_font, light);
    run.len = text.len();
    window.text_system().shape_line(SharedString::from(text.to_string()), font_size, &[run], None)
}

fn shape_truncated_label(label: &str, max_w: Pixels, base_font: &Font, font_size: Pixels, color: Rgba, window: &Window) -> ShapedLine {
    let make = |s: &str| {
        let run = TextRun { len: s.len(), font: base_font.clone(), color: color.into(), background_color: None, underline: None, strikethrough: None };
        window.text_system().shape_line(SharedString::from(s.to_string()), font_size, &[run], None)
    };
    let full = make(label);
    if full.width <= max_w { return full; }
    let chars: Vec<char> = label.chars().collect();
    let mut lo = 1usize;
    let mut hi = chars.len();
    let mut best = make("…");
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let cand: String = chars[..mid].iter().collect::<String>() + "…";
        let shaped = make(&cand);
        if shaped.width <= max_w {
            best = shaped;
            lo = mid + 1;
        } else {
            if mid == 0 { break; }
            hi = mid - 1;
        }
    }
    best
}

fn split_hard_lines(segs: &[UserSeg]) -> Vec<HardLine> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    let mut byte_at = 0usize;
    let mut line_start = 0usize;
    for seg in segs {
        match seg {
            UserSeg::Model(m) => {
                let len = m.word.len();
                cur.push(RawPiece::Chip { label: m.label.clone(), range: byte_at..byte_at + len });
                byte_at += len;
            }
            UserSeg::Span(sp) => {
                for (i, part) in sp.text.split('\n').enumerate() {
                    if i > 0 {
                        out.push(HardLine { range: line_start..byte_at, pieces: std::mem::take(&mut cur) });
                        byte_at += 1;
                        line_start = byte_at;
                    }
                    if !part.is_empty() {
                        let len = part.len();
                        cur.push(RawPiece::Span { text: part.to_string(), style: sp.style.clone(), range: byte_at..byte_at + len });
                        byte_at += len;
                    }
                }
            }
        }
    }
    out.push(HardLine { range: line_start..byte_at, pieces: cur });
    out
}

fn flow_range_rects(lines: &[FlowLine], r: &Range<usize>, lh: Pixels) -> Vec<Bounds<Pixels>> {
    let mut out = Vec::new();
    for vl in lines {
        if r.end <= vl.range.start || r.start >= vl.range.end { continue }
        for it in &vl.items {
            match it {
                FlowItem::Text { x, text_y, range, shaped } => {
                    let a = r.start.max(range.start);
                    let b = r.end.min(range.end);
                    if a < b {
                        let x0 = *x + shaped.x_for_index(a - range.start);
                        let x1 = *x + shaped.x_for_index(b - range.start);
                        out.push(Bounds::from_corners(point(x0, vl.top + *text_y), point(x1, vl.top + *text_y + lh)));
                    }
                }
                FlowItem::Chip { rect, range, .. } => {
                    if r.start < range.end && r.end > range.start {
                        out.push(*rect);
                    }
                }
            }
        }
    }
    out
}

/// Selectable, wrapping paragraph element that renders resolved `&provider/model` references as
/// inline bordered chips while keeping raw text selection/copy (`&provider/model`) and link hits intact.
pub struct FlowText {
    raw_text: String,
    hard_lines: Vec<HardLine>,
    light: bool,
    reg: Reg,
    id: String,
    idx: usize,
    shaded: Vec<Range<usize>>,
    measured: Rc<RefCell<Option<BuiltFlow>>>,
    layout: Option<FlowLayout>,
}

impl FlowText {
    pub fn new(reg: &Reg, raw_text: &str, segs: Vec<UserSeg>, light: bool) -> Self {
        let mut r = reg.borrow_mut();
        let mut at = 0usize;
        let mut links = Vec::new();
        let mut shaded = Vec::new();
        for seg in &segs {
            match seg {
                UserSeg::Span(sp) => {
                    if let Some(l) = &sp.style.link { links.push((at..at + sp.text.len(), l.clone())); }
                    let button = sp.style.link.as_ref().is_some_and(|l| l.href.starts_with("backplane://settings/") || l.href.starts_with("backplane://plan/"));
                    if !sp.text.is_empty() && (sp.style.code || sp.style.kbd || button) { shaded.push(at..at + sp.text.len()); }
                    at += sp.text.len();
                }
                UserSeg::Model(m) => { at += m.word.len(); }
            }
        }
        let entry = r.entry;
        let (id, idx) = (r.entry_id.clone(), r.next_idx());
        r.blocks.push(Info { text: raw_text.to_string(), entry, id: id.clone(), idx, links, copy_spans: vec![], layout: None });
        drop(r);
        let hard_lines = split_hard_lines(&segs);
        Self { raw_text: raw_text.to_string(), hard_lines, light, reg: reg.clone(), id, idx, shaded, measured: Rc::new(RefCell::new(None)), layout: None }
    }

    fn build(hard_lines: &[HardLine], light: bool, wrap_w: Option<f32>, style: &TextStyle, window: &Window) -> BuiltFlow {
        let c = |n: &str| theme::color(light, n);
        let sc = crate::scale::s();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let lh = style.line_height.to_pixels(font_size.into(), window.rem_size());
        let base_font = style.font();
        let chip_fs = crate::scale::tpx(12.);
        let kind_fs = crate::scale::tpx(10.);
        let pad_x = px(6. * sc);
        let gap = px(5. * sc);
        let chip_h = lh.max(crate::scale::tpx(19.));
        let chip_line_h = lh + px(4. * sc);
        let max_label_w = wrap_w.map_or(px(220. * sc), |w| px((w - 72. * sc).max(40. * sc)).min(px(220. * sc)));
        let kind_word = crate::i18n::trs("model");

        let mut built_lines = Vec::new();
        let mut y = px(0.);
        let mut max_w = px(0.);

        for hl in hard_lines {
            if hl.pieces.is_empty() {
                built_lines.push(BuiltLine { top: y, height: lh, text_y: px(0.), range: hl.range.clone(), has_chip: false, items: Vec::new() });
                y += lh;
                continue;
            }
            let mut atoms: Vec<MeasuredAtom> = Vec::new();
            for p in &hl.pieces {
                match p {
                    RawPiece::Chip { label, range } => {
                        let mut amp_font = base_font.clone();
                        amp_font.weight = FontWeight::BOLD;
                        let amp = window.text_system().shape_line("&".into(), chip_fs, &[TextRun { len: 1, font: amp_font, color: c("ok").into(), background_color: None, underline: None, strikethrough: None }], None);
                        let masked_label = crate::streamer::shown(label);
                        let lbl = shape_truncated_label(&masked_label, max_label_w, &base_font, chip_fs, c("text"), window);
                        let kind = window.text_system().shape_line(SharedString::from(kind_word.clone()), kind_fs, &[TextRun { len: kind_word.len(), font: base_font.clone(), color: c("faint").into(), background_color: None, underline: None, strikethrough: None }], None);
                        let w = pad_x * 2. + amp.width + gap + lbl.width + gap + kind.width;
                        let glue_prev = atoms.last().is_some_and(|prev| !prev.is_space);
                        atoms.push(MeasuredAtom {
                            range: range.clone(),
                            width: f32::from(w),
                            is_space: false,
                            is_chip: true,
                            glue_prev,
                            kind: AtomKind::Chip { amp, label: lbl, kind, width: w, height: chip_h, pad_x, gap },
                        });
                    }
                    RawPiece::Span { text, style: sp_style, range } => {
                        let masked = crate::streamer::same_width(text, false);
                        let text: &str = masked.as_deref().unwrap_or(text.as_str());
                        let mut pos = 0usize;
                        while pos < text.len() {
                            let ws = text[pos..].chars().next().unwrap().is_whitespace();
                            let mut end = pos;
                            for ch in text[pos..].chars() {
                                if ch.is_whitespace() != ws { break }
                                end += ch.len_utf8();
                            }
                            let mut sub_start = pos;
                            let mut first_sub = true;
                            while sub_start < end {
                                let chunk = &text[sub_start..end];
                                let shaped = shape_text_piece(chunk, sp_style, &base_font, font_size, light, window);
                                let glue_prev = first_sub && !ws && atoms.last().is_some_and(|prev| !prev.is_space);
                                if !ws && let Some(mw) = wrap_w && f32::from(shaped.width) > mw && chunk.chars().count() > 1 {
                                    let mut cut = shaped.closest_index_for_x(px(mw));
                                    if cut == 0 { cut = chunk.chars().next().map_or(chunk.len(), char::len_utf8); }
                                    while !chunk.is_char_boundary(cut) { cut += 1; }
                                    let part = &chunk[..cut];
                                    let part_shaped = shape_text_piece(part, sp_style, &base_font, font_size, light, window);
                                    let r = (range.start + sub_start)..(range.start + sub_start + cut);
                                    atoms.push(MeasuredAtom {
                                        range: r,
                                        width: f32::from(part_shaped.width),
                                        is_space: false,
                                        is_chip: false,
                                        glue_prev,
                                        kind: AtomKind::Text { text: part.to_string(), style: sp_style.clone(), shaped: part_shaped },
                                    });
                                    sub_start += cut;
                                    first_sub = false;
                                } else {
                                    let r = (range.start + sub_start)..(range.start + end);
                                    atoms.push(MeasuredAtom {
                                        range: r,
                                        width: f32::from(shaped.width),
                                        is_space: ws,
                                        is_chip: false,
                                        glue_prev,
                                        kind: AtomKind::Text { text: chunk.to_string(), style: sp_style.clone(), shaped },
                                    });
                                    sub_start = end;
                                }
                            }
                            pos = end;
                        }
                    }
                }
            }

            let wrap_atoms: Vec<WrapAtom> = atoms.iter().map(|a| WrapAtom { width: a.width, is_space: a.is_space, is_chip: a.is_chip, glue_prev: a.glue_prev }).collect();
            for (atom_range, has_chip) in wrap_flow_lines(&wrap_atoms, wrap_w) {
                let line_h = if has_chip { chip_line_h } else { lh };
                let text_y = (line_h - lh) / 2.;
                let line_atoms = &atoms[atom_range];
                let line_byte_range = line_atoms.first().map(|a| a.range.start).unwrap_or(hl.range.start)..line_atoms.last().map(|a| a.range.end).unwrap_or(hl.range.end);

                // Merge adjacent text atoms with identical style so prose keeps natural kerning and single ShapedLine layout per span.
                let mut merged: Vec<BuiltItem> = Vec::new();
                let mut pending: Option<(String, md::Style, Range<usize>)> = None;
                let mut x = px(0.);
                let flush_pending = |pending: &mut Option<(String, md::Style, Range<usize>)>, x: &mut Pixels, merged: &mut Vec<BuiltItem>| {
                    if let Some((txt, st, rng)) = pending.take() {
                        let shaped = shape_text_piece(&txt, &st, &base_font, font_size, light, window);
                        let w = shaped.width;
                        merged.push(BuiltItem::Text { x: *x, range: rng, shaped });
                        *x += w;
                    }
                };
                for a in line_atoms {
                    match &a.kind {
                        AtomKind::Text { text, style: st, .. } => {
                            if let Some((ptxt, pst, prng)) = pending.as_mut() && *pst == *st && prng.end == a.range.start {
                                ptxt.push_str(text);
                                prng.end = a.range.end;
                            } else {
                                flush_pending(&mut pending, &mut x, &mut merged);
                                pending = Some((text.clone(), st.clone(), a.range.clone()));
                            }
                        }
                        AtomKind::Chip { amp, label, kind, width, height, pad_x, gap } => {
                            flush_pending(&mut pending, &mut x, &mut merged);
                            merged.push(BuiltItem::Chip { x, w: *width, h: *height, pad_x: *pad_x, gap: *gap, range: a.range.clone(), amp: amp.clone(), label: label.clone(), kind: kind.clone() });
                            x += *width;
                        }
                    }
                }
                flush_pending(&mut pending, &mut x, &mut merged);
                max_w = max_w.max(x.ceil());
                built_lines.push(BuiltLine { top: y, height: line_h, text_y, range: line_byte_range, has_chip, items: merged });
                y += line_h;
            }
        }

        let out_w = wrap_w.map_or(max_w, |mw| max_w.min(px(mw)));
        BuiltFlow { wrap_w, size: size(out_w, y.max(lh)), lh, lines: built_lines }
    }
}

impl IntoElement for FlowText { type Element = Self; fn into_element(self) -> Self { self } }

fn flow_needs_rebuild(cached_wrap_w: Option<f32>, cached_w: f32, cached_lines: usize, bounds_w: f32) -> bool {
    cached_w > bounds_w + 0.5 || cached_wrap_w.is_some_and(|w| bounds_w > w + 0.5 && cached_lines > 1)
}

impl Element for FlowText {
    type RequestLayoutState = ();
    type PrepaintState = Vec<(Hitbox, CursorStyle)>;
    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, _: &mut App) -> (LayoutId, ()) {
        let style = window.text_style();
        let hard_lines = self.hard_lines.clone();
        let light = self.light;
        let cache = self.measured.clone();
        let layout_id = window.request_measured_layout(Default::default(), move |known, avail, window, _| {
            let wrap_w = known.width.or(match avail.width { AvailableSpace::Definite(x) => Some(x), _ => None }).map(f32::from);
            let mut g = cache.borrow_mut();
            if g.as_ref().is_none_or(|b| b.wrap_w != wrap_w) {
                *g = Some(FlowText::build(&hard_lines, light, wrap_w, &style, window));
            }
            g.as_ref().unwrap().size
        });
        (layout_id, ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, _: &mut App) -> Vec<(Hitbox, CursorStyle)> {
        let style = window.text_style();
        let bounds_w = f32::from(bounds.size.width);
        let built = {
            let mut g = self.measured.borrow_mut();
            let need_rebuild = g.as_ref().is_none_or(|b| flow_needs_rebuild(b.wrap_w, f32::from(b.size.width), b.lines.len(), bounds_w));
            if need_rebuild {
                *g = Some(FlowText::build(&self.hard_lines, self.light, Some(bounds_w), &style, window));
            }
            g.as_ref().unwrap().clone()
        };
        let lines = built.lines.into_iter().map(|vl| {
            let top = bounds.top() + vl.top;
            let items = vl.items.into_iter().map(|it| match it {
                BuiltItem::Text { x, range, shaped } => FlowItem::Text { x: bounds.left() + x, text_y: vl.text_y, range, shaped },
                BuiltItem::Chip { x, w, h, pad_x, gap, range, amp, label, kind } => {
                    let chip_y = top + (vl.height - h) / 2.;
                    let rect = Bounds::new(point(bounds.left() + x, chip_y), size(w, h));
                    let amp_x = rect.left() + pad_x;
                    let label_x = amp_x + amp.width + gap;
                    let kind_x = label_x + label.width + gap;
                    FlowItem::Chip { rect, range, amp, label, kind, amp_x, label_x, kind_x }
                }
            }).collect();
            FlowLine { top, height: vl.height, text_y: vl.text_y, range: vl.range, has_chip: vl.has_chip, items }
        }).collect();
        let layout = FlowLayout { bounds, line_height: built.lh, text_len: self.raw_text.len(), lines };

        let mut out = vec![(window.insert_hitbox(bounds, HitboxBehavior::Normal), CursorStyle::IBeam)];
        let links: Vec<(Range<usize>, String)> = {
            let r = self.reg.borrow();
            r.at(&self.id, self.idx).map(|o| r.blocks[o].links.iter().map(|(r, l)| (r.clone(), l.href.clone())).collect()).unwrap_or_default()
        };
        for (r, href) in links {
            let rects = flow_range_rects(&layout.lines, &r, layout.line_height);
            if let Some(first) = rects.first() {
                /* i18n-ok: an e2e rect name */ crate::art::report(&format!("link:{}", href.replace(['"', '\\'], "")), *first, window.scale_factor());
            }
            for rect in rects {
                out.push((window.insert_hitbox(rect, HitboxBehavior::Normal), CursorStyle::PointingHand));
            }
        }
        self.layout = Some(layout);
        out
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), pre: &mut Vec<(Hitbox, CursorStyle)>, window: &mut Window, cx: &mut App) {
        let Some(layout) = self.layout.clone() else { return };
        let c = |n: &str| theme::color(self.light, n);
        let shade = c("raise");
        for r in &self.shaded {
            for q in flow_range_rects(&layout.lines, r, layout.line_height) { window.paint_quad(fill(q, shade)); }
        }
        let panel = c("panel");
        let line_c = c("line");
        for vl in &layout.lines {
            for it in &vl.items {
                if let FlowItem::Chip { rect, .. } = it {
                    window.paint_quad(gpui::quad(*rect, Corners::default(), panel, Edges::all(px(1.)), line_c, BorderStyle::Solid));
                }
            }
        }
        let sel_range = { let r = self.reg.borrow(); r.at(&self.id, self.idx).and_then(|o| r.sel.within(o, r.blocks[o].text.len())) };
        if let Some(range) = sel_range {
            let tint = select::highlight(self.light);
            for q in flow_range_rects(&layout.lines, &range, layout.line_height) { window.paint_quad(fill(q, tint)); }
        }
        for vl in &layout.lines {
            for it in &vl.items {
                match it {
                    FlowItem::Text { x, text_y, shaped, .. } => {
                        let _ = shaped.paint(point(*x, vl.top + *text_y), layout.line_height, window, cx);
                    }
                    FlowItem::Chip { rect, amp, label, kind, amp_x, label_x, kind_x, .. } => {
                        let h = rect.size.height;
                        let y = rect.top();
                        let _ = amp.paint(point(*amp_x, y), h, window, cx);
                        let _ = label.paint(point(*label_x, y), h, window, cx);
                        let _ = kind.paint(point(*kind_x, y), h, window, cx);
                    }
                }
            }
        }
        for (h, style) in pre.iter() { window.set_cursor_style(*style, h); }
        let mut r = self.reg.borrow_mut();
        if let Some(o) = r.at(&self.id, self.idx) { r.blocks[o].layout = Some(BlockLayout::Flow(layout)); }
    }
}

pub(crate) fn text(reg: &Reg, light: bool, spans: Vec<Span>, kind: Kind) -> AnyElement { SelText::new(reg, spans, kind, light).into_any_element() }
fn plain_spans(s: &str) -> Vec<Span> { vec![Span { text: s.to_string(), style: Default::default() }] }

/// A person's message whose text may wrap gets the bubble's full width (640 px at most) instead of shrinking to its text: a shrink-to-fit
/// parent let the layout measure the wrapping text at one width and paint it at another, so a long message was drawn over what follows it.
/// A message of short lines never wraps and keeps its snug bubble.
/// A message with pictures is laid out in blocks too: in the snug flex bubble the pictures' wrapping row was measured on one line and painted on
/// two, so the second picture spilled out of the bubble over the rows below and the composer.
pub fn bubble_full_width(text: &str, attachments: &Value) -> bool {
    text.lines().any(|l| l.chars().count() > 60) || !split_attachments(attachments).0.is_empty()
}

/// [`bubble_full_width`] with a lower threshold when inline model chips add extra width beyond their raw `@`/`&` slug length.
pub fn bubble_full_width_with_chips(text: &str, attachments: &Value, has_model_chips: bool) -> bool {
    bubble_full_width(text, attachments) || (has_model_chips && text.lines().any(|l| l.chars().count() > 40))
}

/// A user's message text.
pub fn user_text(reg: &Reg, light: bool, s: &str, refs: &[String]) -> AnyElement { text(reg, light, md::chips(md::linkify(s), refs), Kind::Body) }

/// A user's message text with resolved `&provider/model` references embedded inline as chips, plus the set of model ref words embedded inline.
pub fn user_text_with_models(reg: &Reg, light: bool, s: &str, other_refs: &[String], models: &[ModelChip]) -> (AnyElement, std::collections::HashSet<String>) {
    if models.is_empty() { return (user_text(reg, light, s, other_refs), Default::default()) }
    let segs = md::user_segments(s, other_refs, models);
    let inline_words: std::collections::HashSet<String> = segs.iter().filter_map(|seg| match seg {
        UserSeg::Model(m) => Some(m.word.clone()),
        _ => None,
    }).collect();
    if inline_words.is_empty() {
        return (user_text(reg, light, s, other_refs), inline_words);
    }
    (FlowText::new(reg, s, segs, light).into_any_element(), inline_words)
}

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

pub fn block_el(cx: &mut Context<Backplane>, reg: &Reg, light: bool, b: &Block, key: &str, doc: Option<&Doc>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    match b {
        Block::Para(s) => text(reg, light, s.clone(), Kind::Body),
        Block::Heading(l, s) => {
            // a document's h1 and h2 are underlined, as GitHub draws them
            let h = div().text_size(crate::scale::tpx(if doc.is_some() { doc_heading_size(*l) } else { heading_size(*l) })).mt_1().child(text(reg, light, s.clone(), Kind::Head));
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
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.screens.reg.borrow().sel.collapsed() { return; }
                    DETAILS.with(|d| { let mut d = d.borrow_mut(); if !d.remove(&k) { d.insert(k.clone()); } }); cx.notify();
                }));
            let mut col = div().flex().flex_col().gap_2().child(head);
            if shown { let mut inner = div().pl_4().flex().flex_col().gap_2(); for (i, b) in blocks.iter().enumerate() { inner = inner.child(block_el(cx, reg, light, b, &format!("{key}-{i}"), doc)); } col = col.child(inner); }
            col.into_any_element()
        }
        Block::Anchor(id) => { let id = id.clone(); canvas(|_, _, _| (), move |b, _, _, _| { ANCHORS.with(|a| { a.borrow_mut().insert(id.clone(), b); }); }).w_full().h(px(0.)).into_any_element() }
        Block::Quote(s) => div().border_l_2().border_color(c("line")).pl_3().child(text(reg, light, s.clone(), Kind::Quote)).into_any_element(),
        Block::Rule => div().h(px(1.)).w_full().bg(c("line")).my_1().into_any_element(),
        Block::List { .. } => list_el(cx, reg, light, b, key, 0, doc),
        Block::Code(t, tokens, cmd) => {
            let copy_ix = { let mut r = reg.borrow_mut(); let n = r.copy_seen; r.copy_seen += 1; n };
            let id = SharedString::from(format!("code-{key}"));
            let app = cx.entity().downgrade();
            div().w_full().bg(c("raise")).border_1().border_color(c("line")).flex().flex_col()
                .child(div().flex().justify_end().gap_1().px_2().pt_1()
                    .when(*cmd, |d| d.child(super::copyable::run_button(light, id.clone(), t.clone(), true, app, cx)))
                    .child(div().relative().child(super::copyable::button(light, id.clone(), t.clone(), super::copyable::Opts { word: true, always: true }, cx)).child(copy_probe(reg, copy_ix))))
                .child(div().id(SharedString::from(format!("code-{key}"))).overflow_x_scroll().px_2().pb_2().text_size(crate::scale::tpx(12.)).whitespace_nowrap().child({ let mut s = SelText::new(reg, plain_spans(t), Kind::Code, light); s.tokens = tokens.clone(); s.into_any_element() }))
                .into_any_element()
        }
        Block::Table { head, rows } => {
            let cols = head.len().max(rows.iter().map(Vec::len).max().unwrap_or(0)).max(1);
            let row = |cells: &Vec<Vec<Span>>, bold: bool| div().flex().border_b_1().border_color(c("line")).children((0..cols).map(|i| {
                let spans = cells.get(i).cloned().unwrap_or_default();
                let spans = if bold { spans.into_iter().map(|mut s| { s.style.bold = true; s }).collect() } else { spans };
                div().flex_1().min_w(px(60.)).px_2().py_1().child(text(reg, light, spans, Kind::Body)).into_any_element()
            }).collect::<Vec<_>>());
            let mut t = div().id(SharedString::from(format!("tbl-{key}"))).overflow_x_scroll().border_1().border_color(c("line")).flex().flex_col().text_size(crate::scale::tpx(12.5));
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
fn fingerprint(text: &str, nodes: &[Value], extra: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new(); text.hash(&mut h); nodes.len().hash(&mut h); extra.hash(&mut h); h.finish()
}
impl ParseCache {
    pub fn get(&mut self, id: &str, text: &str, nodes: &[Value]) -> Rc<Vec<Block>> {
        self.get_with(id, text, nodes, 0, |_| {})
    }
    pub fn get_with(&mut self, id: &str, text: &str, nodes: &[Value], extra: u64, post: impl FnOnce(&mut Vec<Block>)) -> Rc<Vec<Block>> {
        let f = fingerprint(text, nodes, extra);
        if let Some((g, b)) = self.items.get(id) { if *g == f { return b.clone(); } }
        if self.items.len() >= PARSE_KEEP { self.items.clear(); }
        let mut parsed = md::parse(nodes);
        post(&mut parsed);
        let b = Rc::new(parsed);
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
            // the same menu as a message's pictures; Open, Show in folder and Copy path only for a file named by its full path
            let file = if src.starts_with('/') || src.starts_with("~/") { src.clone() } else { String::new() };
            let file_name = src.rsplit('/').next().unwrap_or("").split(['?', '#']).next().unwrap_or("").to_string();
            let open = super::images::Shown { img: shown.clone(), name: if file_name.is_empty() { n.clone() } else { file_name }, file, src: doc.target.cloned().map(|t| (t, url.clone())), shrunk: doc.imgs.shrunk(&url) };
            let open2 = open.clone();
            frame.overflow_hidden().cursor_pointer()
                .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| { cx.stop_propagation(); let s = &open2; this.screens.ctx_img = Some(s.img.clone());
                    this.screens.ctx_save = Some(super::images::Save { img: s.img.clone(), name: s.name.clone(), src: s.src.clone(), shrunk: s.shrunk.is_some() }); this.ctx_open(e.position, super::ctxmenu::image_items(&s.file), cx); }))
                .on_click(cx.listener(move |this, _, window, cx| { match &link { Some(l) => this.open_link(l, cx), None => this.lightbox_open(open.clone(), window, cx) } cx.notify(); }))
                .child(gpui::img(src_img).id(SharedString::from(format!("dimg-pic-{key}"))).size_full().object_fit(ObjectFit::Contain)).into_any_element()
        }
        (None, Some(Slot::Failed)) => {
            // GitHub shows the words of a picture that cannot be drawn
            div().px_1().text_size(crate::scale::tpx(12.)).text_color(c("dim")).border_1().border_color(c("line")).child(name).into_any_element()
        }
        (None, slot) => {
            let sc = crate::scale::s(); let (bw, bh) = size.unwrap_or((if img.alt.is_empty() { 120. * sc } else { (img.alt.chars().count() as f32 * 7. * sc + 16.).min(doc.width) }, 22. * sc));
            let asked = slot.is_some();
            let (imgs, target, u) = (doc.imgs.clone(), doc.target.cloned(), url.clone());
            let mut b = div().id(id).relative().w(px(bw)).max_w_full().flex_none().border_1().border_color(c("line")).overflow_hidden().text_size(crate::scale::tpx(11.)).text_color(c("faint")).px_1().child(name);
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
    if level > 0 { row = row.text_size(crate::scale::tpx(if doc.is_some() { doc_heading_size(level) } else { heading_size(level) })).mt_1(); }
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
                let open = super::images::Shown { img: shown, name: n, file: ipath.clone(), src: target.cloned().map(|t| (t, url.to_string())), shrunk: imgs.shrunk(url) };
                let save = super::images::Save { img: copy.clone(), name: open.name.clone(), src: open.src.clone(), shrunk: open.shrunk.is_some() };
                frame.overflow_hidden().cursor_pointer().on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| { cx.stop_propagation(); this.screens.ctx_img = Some(copy.clone()); this.screens.ctx_save = Some(save.clone()); this.ctx_open(e.position, super::ctxmenu::image_items(&ipath), cx); }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| { this.lightbox_open(open.clone(), window, cx); }))
                    .child(gpui::img(imgs.decoded(url).map_or(ImageSource::Image(img), ImageSource::Render)).id(SharedString::from(format!("timg-{key}-{i}"))).size_full().object_fit(ObjectFit::Contain)).into_any_element() }
            Slot::Loading => div().px_2().py_1().border_1().border_color(c("line")).text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(crate::i18n::tf("gpui.timeline.loading", &[("name", name.as_str().into())])).into_any_element(),
            Slot::Failed => div().px_2().py_1().border_1().border_color(c("line")).text_size(crate::scale::tpx(12.)).text_color(c("bad")).child(crate::i18n::tf("gpui.timeline.could.not.load", &[("name", name.as_str().into())])).into_any_element(),
        };
        row = row.child(crate::ui::with_rect(div().id(SharedString::from(format!("img-{key}-{i}"))), || format!("img:{name}")).max_w_full().child(el));
    }
    if items.len() > MAX_IMAGES { row = row.child(div().px_2().py_1().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(crate::i18n::tf("gpui.timeline.more.images", &[("n", (items.len() - MAX_IMAGES).into())]))); }
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
    if !files.is_empty() { out.push(div().flex().flex_wrap().gap_1().children(files.into_iter().map(|f| div().px_2().py_1().border_1().border_color(c("line")).text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(f))).into_any_element()); }
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
    /// Whether a delayed single-click copy should fire after the double-click window passes.
    fn should_delayed_copy(seq_then: u64, seq_now: u64, collapsed: bool) -> bool { seq_then == seq_now && collapsed }
    fn scoped_pos(&self, p: Point<Pixels>, rows_only: bool) -> Option<Pos> { self.screens.reg.borrow().hit_scoped(p, Some(rows_only)) }
    fn notify_sel(&self, rows_only: bool, cx: &mut Context<Self>) {
        if rows_only {
            let bot_kind = self.data["bot"]["kind"].as_str().unwrap_or("");
            if bot_kind == "room" && self.data["bot"]["swarm"].is_object() {
                if !self.screens.parts.notify(Backplane::swarm_part, cx) { cx.notify(); }
            } else if matches!(bot_kind, "room" | "remote") {
                cx.notify();
            } else if !self.screens.parts.notify(Backplane::timeline_part, cx) {
                cx.notify();
            }
        } else if self.project_page.is_some() {
            cx.notify();
        } else if super::viewer::is_open(&self.data["thread"]) {
            if !self.screens.parts.notify(Backplane::viewer_part, cx) { cx.notify(); }
        } else {
            cx.notify();
        }
    }
    pub(crate) fn timeline_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) { self.sel_down(e, true, window, cx); }
    pub(crate) fn doc_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) { self.sel_down(e, false, window, cx); }
    fn sel_down(&mut self, e: &MouseDownEvent, rows_only: bool, window: &mut Window, cx: &mut Context<Self>) {
        { let mut r = self.screens.reg.borrow_mut(); r.click_seq = r.click_seq.wrapping_add(1); }
        let Some(pos) = self.scoped_pos(e.position, rows_only) else { return };
        if e.button == MouseButton::Right {
            // a selection's Copy and Quote first, then a link's own items
            let link = self.screens.reg.borrow().link_at(pos).cloned();
            let mut items = super::ctxmenu::selection_items(&self.timeline_selected());
            if let Some(l) = link { let dl = self.link_download(pos.block, &l); items.extend(super::ctxmenu::link_items(&l.href, l.file, dl)); }
            if !items.is_empty() { self.ctx_open(e.position, items, cx); }
            return;
        }
        let mut r = self.screens.reg.borrow_mut();
        let texts = r.texts();
        match e.click_count {
            2 => {
                let w = select::word_at(&texts[pos.block].text, pos.off);
                r.select(Selection { anchor: Pos { block: pos.block, off: w.start }, head: Pos { block: pos.block, off: w.end } });
                r.drag_mode = DragMode::Word; r.drag_origin = r.keyed.clone();
            }
            n if n >= 3 => {
                if let Some(s) = select::block_range(&texts, pos.block) { r.select(s); }
                r.drag_mode = DragMode::Block; r.drag_origin = r.keyed.clone();
            }
            _ => {
                if e.modifiers.shift { r.extend(pos); } else { r.select(Selection { anchor: pos, head: pos }); }
                r.drag_mode = DragMode::Char; r.drag_origin = None;
            }
        }
        r.dragging = true;
        r.drag_surface = Some(rows_only);
        r.drag_pos = Some(e.position);
        let picked = e.click_count >= 2;
        drop(r);
        if picked { self.timeline_selection_made(window, cx); }
        self.notify_sel(rows_only, cx);
    }
    pub(crate) fn timeline_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) { self.sel_hover(e, true, cx); }
    pub(crate) fn doc_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) { self.sel_hover(e, false, cx); }
    fn sel_hover(&mut self, e: &MouseMoveEvent, rows_only: bool, cx: &mut Context<Self>) {
        if self.screens.reg.borrow().dragging { return; }
        let under = self.scoped_pos(e.position, rows_only).and_then(|p| self.screens.reg.borrow().link_at(p).map(|l| super::linkroute::target(&l.href))).unwrap_or_default();
        if under != self.screens.reg.borrow().hover { self.screens.reg.borrow_mut().hover = under; self.notify_sel(rows_only, cx); }
    }
    fn active_drag_list(&self) -> Option<&ListState> {
        let bot_kind = self.data["bot"]["kind"].as_str().unwrap_or("");
        if bot_kind == "room" && self.data["bot"]["swarm"].is_object() {
            Some(&self.screens.swarm_list)
        } else if matches!(bot_kind, "room" | "remote") {
            None
        } else {
            Some(&self.timeline)
        }
    }
    fn drag_scroll_step(&self, p: Point<Pixels>) -> Pixels {
        let Some(list) = self.active_drag_list() else { return px(0.); };
        let vp = list.viewport_bounds();
        drag_scroll_delta(p.y, vp.top(), vp.bottom())
    }
    fn ensure_drag_scroll(&mut self, cx: &mut Context<Self>) {
        {
            let mut r = self.screens.reg.borrow_mut();
            if r.drag_scrolling { return; }
            r.drag_scrolling = true;
        }
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(32)).await;
                let keep = this.update(cx, |b, cx| {
                    let (dragging, rows_only, pos) = {
                        let r = b.screens.reg.borrow();
                        (r.dragging, r.drag_surface.unwrap_or(true), r.drag_pos)
                    };
                    if !dragging || !rows_only { b.screens.reg.borrow_mut().drag_scrolling = false; return false; }
                    let Some(p) = pos else { b.screens.reg.borrow_mut().drag_scrolling = false; return false; };
                    let dy = b.drag_scroll_step(p);
                    if dy == px(0.) { b.screens.reg.borrow_mut().drag_scrolling = false; return false; }
                    if let Some(list) = b.active_drag_list() { list.scroll_by(dy); }
                    if let Some(hit) = b.scoped_pos(p, true) { b.screens.reg.borrow_mut().extend_drag(hit); }
                    b.notify_sel(true, cx);
                    true
                }).unwrap_or(false);
                if !keep { break; }
            }
        }).detach();
    }
    pub(crate) fn timeline_drag_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let rows_only = {
            let mut r = self.screens.reg.borrow_mut();
            if !r.dragging { return; }
            r.drag_pos = Some(e.position);
            r.drag_surface.unwrap_or(true)
        };
        if let Some(pos) = self.scoped_pos(e.position, rows_only) {
            self.screens.reg.borrow_mut().extend_drag(pos);
        }
        if rows_only {
            let dy = self.drag_scroll_step(e.position);
            if dy != px(0.) {
                if let Some(list) = self.active_drag_list() { list.scroll_by(dy); }
                self.ensure_drag_scroll(cx);
            }
        }
        self.notify_sel(rows_only, cx);
    }
    pub(crate) fn timeline_up(&mut self, e: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (was, rows_only) = {
            let mut r = self.screens.reg.borrow_mut();
            let was = std::mem::replace(&mut r.dragging, false);
            let rows_only = r.drag_surface.take().unwrap_or(true);
            r.drag_pos = None; r.drag_scrolling = false; r.drag_origin = None;
            (was, rows_only)
        };
        if !was { return; }
        // a click without a drag on a link opens it
        let (collapsed, pos) = { let r = self.screens.reg.borrow(); (r.sel.collapsed(), self.scoped_pos(e.position, rows_only)) };
        let link = pos.filter(|_| collapsed).and_then(|p| self.screens.reg.borrow().link_at(p).cloned());
        if let Some(l) = link {
            self.screens.reg.borrow_mut().hover.clear();
            // the row the link was pressed in: its `files` decide (the same path may be a link in another row too)
            let entry = pos.and_then(|p| self.screens.reg.borrow().blocks.get(p.block).map(|b| b.id.clone())).unwrap_or_default();
            self.open_link_in(&l, &entry, cx);
        } else if collapsed && e.click_count == 1 {
            let (seq, span) = { let r = self.screens.reg.borrow(); (r.click_seq, pos.and_then(|p| r.copy_span_at(p))) };
            if let Some((text, cid)) = span {
                let w = window.window_handle();
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(240)).await;
                    let _ = w.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            let r = this.screens.reg.borrow();
                            if Self::should_delayed_copy(seq, r.click_seq, r.sel.collapsed()) {
                                drop(r);
                                super::copyable::copy(&cid, &text, window, cx);
                            }
                        });
                    });
                }).detach();
            }
        }
        self.timeline_selection_made(window, cx);
        self.notify_sel(rows_only, cx);
    }
    /// A selection was made in the timeline (a drag, a double or a triple click): it is the PRIMARY selection on Linux (middle-click paste in
    /// other programs), and the copy chord (Ctrl+C, Cmd+C on a Mac) copies it from now on, wherever the keyboard was (`copy_claim`).
    fn timeline_selection_made(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.timeline_selected();
        if s.is_empty() { return }
        #[cfg(target_os = "linux")]
        cx.write_to_primary(ClipboardItem::new_string(s));
        match copy_claim(self.input.focus_handle(cx).is_focused(window), self.screens.root_focus.is_focused(window)) {
            Claim::ComposerDrops => self.input.update(cx, |c, cx| c.drop_selection(cx)),
            Claim::Root => window.focus(&self.screens.root_focus),
            Claim::Stay => {}
        }
    }
    /// Open a link of the timeline or a document: a file in the viewer, a web address in the browser, a document's `#anchor` scrolled to.
    pub(crate) fn open_link(&mut self, l: &Link, cx: &mut Context<Self>) { self.open_link_in(l, "", cx) }
    /// `entry`: the thread row the link was pressed in ("" when unknown: the first row drawn with that link).
    pub(crate) fn open_link_in(&mut self, l: &Link, entry: &str, cx: &mut Context<Self>) {
        if let Some(id) = l.href.strip_prefix('#') { self.jump_to_anchor(id); cx.notify(); return; }
        match super::linkroute::route(&l.href, l.file) {
            super::linkroute::Route::File(p) => { self.note.clear();
                // on another computer's hub a file the hub lists as a download comes down to this one (docs/file-links.md)
                let files = if entry.is_empty() { self.link_files(l) } else { self.row_files(entry) };
                match super::linkroute::file_go(&p, &files, self.screens.elsewhere.is_some(), &self.thread_raw_id()) {
                    super::linkroute::FileGo::Download { url, name } => self.download(url, name, super::dlbar::From::Other, cx),
                    super::linkroute::FileGo::Open(p) => self.link_file(&p, false, cx),
                } }
            super::linkroute::Route::Share(u) => { self.note.clear(); let name = super::linkroute::share_name(&u).unwrap_or_default(); self.download(u, name, super::dlbar::From::Other, cx) }
            super::linkroute::Route::Web(u) => cx.open_url(&u),
            super::linkroute::Route::Settings(page) => {
                self.note.clear();
                if page == "accounts" { self.open_accounts(cx); } else { self.go("link-open", format!("backplane://settings/{page}"), cx); }
            }
            super::linkroute::Route::Hub(u) => { self.note.clear(); self.go("link-open", u, cx); }
            super::linkroute::Route::Note(n) => self.note = n.to_string(),
            super::linkroute::Route::Design(w) => super::xprobe::clicked(self, &w),
            // an agent's plan review: the board page, then (screens/mgmt.rs `host`) that board with its Plan panel open
            super::linkroute::Route::Plan(rel) => { self.note.clear(); self.screens.mgmt.plan_next = Some((rel, std::time::Instant::now(), false)); let _ = self.tx.try_send(super::viewer::view_kind("board")); cx.notify(); }
        }
    }
    /// The thread on show, by its raw id (`<link>~<id>` for one of another machine).
    pub(crate) fn thread_raw_id(&self) -> String { let t = self.data["thread"]["id"].as_str().unwrap_or(""); t.rsplit_once('|').map_or(t, |(_, i)| i).to_string() }
    /// The files the hub lists for the message a link of the timeline is in (its row's `files`); none for a link outside the thread's rows.
    fn link_files(&self, l: &Link) -> Value {
        let r = self.screens.reg.borrow();
        let id = r.blocks.iter().find(|b| b.links.iter().any(|(_, x)| x == l)).map(|b| b.id.clone()).unwrap_or_default();
        drop(r);
        self.row_files(&id)
    }
    pub(crate) fn active_rows(&self) -> &[Value] {
        if self.data["bot"]["kind"] == "room" && self.data["bot"]["swarm"].is_object() {
            self.data["bot"]["swarm"]["posts"].as_array().map_or(&[], |r| r.as_slice())
        } else {
            self.data["thread"]["entries"].as_array().map_or(&[], |r| r.as_slice())
        }
    }
    fn row_files(&self, entry: &str) -> Value {
        if entry.is_empty() { return Value::Null }
        let is_swarm = self.data["bot"]["kind"] == "room" && self.data["bot"]["swarm"].is_object();
        self.active_rows().iter().find(|e| e["id"].as_str() == Some(entry)).map_or(Value::Null, |e| {
            if is_swarm { e["linkFiles"].clone() } else { e["files"].clone() }
        })
    }
    /// The Download a link's menu offers (a share always; a file on another computer's hub), as (hub address, name).
    fn link_download(&self, block: usize, l: &Link) -> Option<(String, String)> {
        if let Some(n) = super::linkroute::share_name(&l.href) { return Some((l.href.trim().to_string(), n)) }
        if !matches!(super::linkroute::route(&l.href, l.file), super::linkroute::Route::File(_)) { return None }
        let id = self.screens.reg.borrow().blocks.get(block).map(|b| b.id.clone()).unwrap_or_default();
        super::linkroute::menu_download(&l.href, &self.row_files(&id), self.screens.elsewhere.is_some(), &self.thread_raw_id())
    }
    /// Scroll the document on screen (the project's README, or a note in the file viewer) so the anchor `id` is at its top.
    fn jump_to_anchor(&mut self, id: &str) {
        // a project's page: its list goes to the README block the anchor is in, drawn or not
        if !self.screens.home_list_key.is_empty() {
            if let Some(&i) = self.screens.home_anchors.get(id) { self.screens.home_list.scroll_to(gpui::ListOffset { item_ix: crate::screens::projects::items(0) - 1 + i, offset_in_item: px(0.) }); return }
        }
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
    fn timeline_selected(&self) -> String { self.screens.reg.borrow().selected(self.active_rows()) }
    /// Insert the selected text as a Markdown blockquote into the active composer and focus it.
    pub(crate) fn quote_into_composer(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let bot_kind = self.data["bot"]["kind"].as_str().unwrap_or("");
        let field_input = if self.project_page.is_none() {
            match bot_kind {
                "room" => self.screens.fields.get("rpost").map(|s| s.input.clone()),
                "remote" => self.screens.fields.get("rtext").map(|s| s.input.clone()),
                _ => None,
            }
        } else {
            None
        };
        let target = match field_input {
            Some(input) => input,
            None => {
                self.want_focus = true;
                self.input.clone()
            }
        };
        target.update(cx, |c, cx| c.insert_quote(text, cx));
        target.focus_handle(cx).focus(window);
        cx.notify();
    }
}

/// Auto-scroll step in pixels when dragging above (`y < top`) or below (`y > bottom`) a list viewport.
pub fn drag_scroll_delta(y: Pixels, top: Pixels, bottom: Pixels) -> Pixels {
    if bottom <= top { return px(0.); }
    if y < top { -((top - y).min(px(160.)) * 0.35 + px(6.)) }
    else if y > bottom { (y - bottom).min(px(160.)) * 0.35 + px(6.) }
    else { px(0.) }
}

/// What a selection made in the timeline does with the keyboard, so that the copy chord copies it (the chord goes where the keyboard is).
#[derive(Debug, PartialEq)]
pub enum Claim {
    /// The composer keeps the keyboard (the person goes on typing) and drops its own selection: its Copy, with nothing of its own selected,
    /// copies the timeline's (`ComposerEvent::CopyTimeline`). One selection in the window, the newest, as a web page has.
    ComposerDrops,
    /// Anywhere else (a terminal, a viewer, a search box, a copy button, nothing at all) the window's own handle takes it: its Copy is the timeline's.
    Root,
    /// The window's handle has it already.
    Stay,
}
pub fn copy_claim(composer_focused: bool, root_focused: bool) -> Claim {
    if composer_focused { Claim::ComposerDrops } else if root_focused { Claim::Stay } else { Claim::Root }
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
    #[test] fn a_selection_in_the_timeline_is_what_the_copy_chord_copies_wherever_the_keyboard_was() {
        // the composer's own Ctrl+C would copy its older selection (a draft copied before): it is dropped, the keyboard stays to type on
        assert_eq!(copy_claim(true, false), Claim::ComposerDrops);
        // a terminal (Ctrl+C is its ^C), a search box, a copy button or no focus at all (no keys reach the window): the window's handle takes it
        assert_eq!(copy_claim(false, false), Claim::Root);
        assert_eq!(copy_claim(false, true), Claim::Stay);
    }
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
    fn info(id: &str, idx: usize, text: &str) -> Info { Info { text: text.into(), entry: id[1..].parse().unwrap_or(0), id: id.into(), idx, links: vec![], copy_spans: vec![], layout: None } }
    fn frame(r: &mut Registry, blocks: &[(&str, usize, &str)]) { r.begin(); for (id, idx, t) in blocks { r.blocks.push(info(id, *idx, t)); } r.resolve(); }
    #[test] fn a_code_span_is_shaded_by_paint_under_the_selection_not_by_its_run() {
        // the person selected a list item over an inline command and saw no highlight on the command: its run's own background covered it
        let sp = |t: &str, code: bool, bold: bool| Span { text: t.into(), style: md::Style { code, bold, ..Default::default() } };
        let spans = vec![sp("Paste this in: ", false, false), sp("PROTON_LOG=1 %command%", true, false), sp(" and ", false, true), sp("x", true, false)];
        assert_eq!(shaded(&spans), vec![15..37, 42..43]);
        let reg: Reg = Default::default(); let st = SelText::new(&reg, spans, Kind::Body, false);
        assert!(st.runs(&TextStyle::default()).iter().all(|r| r.background_color.is_none()), "no run paints a background over the selection");
        assert_eq!(st.shaded, vec![15..37, 42..43]);
    }
    #[test] fn a_range_covers_its_lines() {
        let (lh, l, r) = (px(10.), px(0.), px(100.));
        assert_eq!(line_rects(point(px(5.), px(0.)), point(px(20.), px(0.)), lh, l, r), vec![Bounds::from_corners(point(px(5.), px(0.)), point(px(20.), px(10.)))]);
        let m = line_rects(point(px(50.), px(0.)), point(px(20.), px(20.)), lh, l, r);
        assert_eq!(m.len(), 3); assert_eq!(m[1], Bounds::from_corners(point(l, px(10.)), point(r, px(20.)))); assert_eq!(m[2].right(), px(20.));
    }
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
        let outside = |t: &str| Info { text: t.into(), entry: 0, id: String::new(), idx: 0, links: vec![], copy_spans: vec![], layout: None };
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

    #[test] fn wrap_flow_lines_keeps_chips_with_adjacent_punctuation_and_wraps_cleanly() {
        let a = |w: f32, space: bool, chip: bool, glue: bool| WrapAtom { width: w, is_space: space, is_chip: chip, glue_prev: glue };
        // "use " (28 + 4), chip (120), "," (4, glued), " ok" (4 + 20) at max_w = 154: chip+comma = 124, 32 + 124 = 156 > 154 -> wraps before chip!
        let atoms = vec![a(28., false, false, false), a(4., true, false, false), a(120., false, true, false), a(4., false, false, true), a(4., true, false, false), a(20., false, false, false)];
        assert_eq!(wrap_flow_lines(&atoms, Some(154.)), vec![(0..2, false), (2..6, true)]);
        // Empty line still produces one line without a chip.
        assert_eq!(wrap_flow_lines(&[], Some(200.)), vec![(0..0, false)]);
        // "(" + chip + ")" stays together as one indivisible cluster.
        let paren = vec![a(50., false, false, false), a(4., true, false, false), a(6., false, false, false), a(100., false, true, true), a(6., false, false, true)];
        assert_eq!(wrap_flow_lines(&paren, Some(150.)), vec![(0..2, false), (2..5, true)]);
    }

    #[test] fn flow_needs_rebuild_reuses_snug_single_line_and_rebuilds_on_shrink_or_rewrap() {
        assert!(!flow_needs_rebuild(None, 180., 1, 180.), "snug single-line chip keeps its layout width");
        assert!(!flow_needs_rebuild(Some(614.), 180., 1, 614.), "exact cached width never rebuilds");
        assert!(!flow_needs_rebuild(Some(300.), 180., 1, 500.), "single unwrapped line stays valid when wider");
        assert!(flow_needs_rebuild(Some(614.), 250., 1, 200.), "narrower bounds trigger wrap rebuild");
        assert!(flow_needs_rebuild(Some(300.), 290., 2, 420.), "wider bounds trigger unwrap rebuild for multi-line flow");
    }

    #[test] fn inline_code_and_file_links_get_in_place_copy_chips_while_only_non_link_code_copies_on_click() {
        let reg: Reg = Default::default();
        reg.borrow_mut().enter(0, "m1");
        let spans = vec![
            Span { text: "Check ".into(), style: md::Style::default() },
            Span { text: "EPC2306".into(), style: md::Style { code: true, copy: Some(md::CopyAs::Id), ..Default::default() } },
            Span { text: " and ".into(), style: md::Style::default() },
            Span { text: "docs/architecture.md".into(), style: md::Style { code: true, copy: Some(md::CopyAs::Path), link: Some(md::Link { href: "docs/architecture.md".into(), file: true }), ..Default::default() } },
            Span { text: " or ".into(), style: md::Style::default() },
            Span { text: "/etc/hosts".into(), style: md::Style { link: Some(md::Link { href: "/etc/hosts".into(), file: true }), ..Default::default() } },
            Span { text: " via ".into(), style: md::Style::default() },
            Span { text: "https://example.com".into(), style: md::Style { link: Some(md::Link { href: "https://example.com".into(), file: false }), ..Default::default() } },
        ];
        let st = SelText::new(&reg, spans, Kind::Body, false);
        assert_eq!(st.chips.iter().map(|c| c.text.as_ref()).collect::<Vec<_>>(), ["EPC2306", "docs/architecture.md", "/etc/hosts"]);
        let r = reg.borrow();
        let code_at = r.blocks[0].text.find("EPC2306").unwrap() + 1;
        let path_at = r.blocks[0].text.find("docs/architecture.md").unwrap() + 1;
        assert_eq!(r.copy_span_at(Pos { block: 0, off: code_at }).map(|(t, _)| t), Some("EPC2306".to_string()));
        assert_eq!(r.copy_span_at(Pos { block: 0, off: path_at }), None, "file links open on click and copy via their hover button");
    }

    #[test] fn parse_cache_get_with_caches_post_transform_and_invalidates_on_extra_change() {
        let mut c = ParseCache::default();
        let n: Vec<Value> = serde_json::from_str(r#"[{"tag":"p","kids":[{"text":"@alice"}]}]"#).unwrap();
        let mut runs = 0usize;
        let a = c.get_with("post-1", "@alice", &n, 11, |_| runs += 1);
        let b = c.get_with("post-1", "@alice", &n, 11, |_| runs += 1);
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(runs, 1);
        let d = c.get_with("post-1", "@alice", &n, 22, |_| runs += 1);
        assert!(!Rc::ptr_eq(&a, &d));
        assert_eq!(runs, 2);
    }

    #[test] fn delayed_copy_cancels_when_click_seq_or_selection_changes() {
        assert!(crate::Backplane::should_delayed_copy(5, 5, true));
        assert!(!crate::Backplane::should_delayed_copy(5, 6, true));
        assert!(!crate::Backplane::should_delayed_copy(5, 5, false));
    }

    #[test] fn double_and_triple_click_drag_extend_by_word_and_block() {
        let mut r = Registry::default();
        frame(&mut r, &[("e1", 0, "alpha beta gamma"), ("e2", 0, "delta epsilon")]);
        r.select(Selection { anchor: Pos { block: 0, off: 6 }, head: Pos { block: 0, off: 10 } });
        r.drag_mode = DragMode::Word; r.drag_origin = r.keyed.clone();
        r.extend_drag(Pos { block: 1, off: 2 });
        assert_eq!(select::extract(&r.texts(), &r.sel), "beta gamma\n\ndelta");
        r.extend_drag(Pos { block: 0, off: 1 });
        assert_eq!(select::extract(&r.texts(), &r.sel), "alpha beta");
        r.select(Selection { anchor: Pos { block: 1, off: 0 }, head: Pos { block: 1, off: 13 } });
        r.drag_mode = DragMode::Block; r.drag_origin = r.keyed.clone();
        r.extend_drag(Pos { block: 0, off: 3 });
        assert_eq!(select::extract(&r.texts(), &r.sel), "alpha beta gamma\n\ndelta epsilon");
    }

    #[test] fn drag_scroll_delta_clamps_above_below_and_inside_viewport() {
        assert_eq!(drag_scroll_delta(px(150.), px(100.), px(300.)), px(0.));
        assert_eq!(drag_scroll_delta(px(100.), px(100.), px(300.)), px(0.));
        assert_eq!(drag_scroll_delta(px(300.), px(100.), px(300.)), px(0.));
        assert_eq!(drag_scroll_delta(px(100.), px(200.), px(200.)), px(0.));
        assert_eq!(drag_scroll_delta(px(80.), px(100.), px(300.)), px(-(20. * 0.35 + 6.)));
        assert_eq!(drag_scroll_delta(px(-200.), px(100.), px(300.)), px(-(160. * 0.35 + 6.)));
        assert_eq!(drag_scroll_delta(px(320.), px(100.), px(300.)), px(20. * 0.35 + 6.));
        assert_eq!(drag_scroll_delta(px(800.), px(100.), px(300.)), px(160. * 0.35 + 6.));
    }

    #[test] fn code_seltext_with_no_tokens_uses_full_span_length() {
        let reg: Reg = Default::default();
        let st = SelText::new(&reg, plain_spans("hello\nworld"), Kind::Code, false);
        let runs = st.runs(&TextStyle::default());
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].len, "hello\nworld".len());
    }
}

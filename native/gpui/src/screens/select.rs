//! Selection across the timeline's text blocks. Every block of text drawn in a frame is numbered in paint order;
//! a position is (block, byte offset); a selection is two positions. Pure: the element that paints and hit-tests
//! the blocks lives in timeline.rs, this is the arithmetic, the word and block rules and the copied text.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Pos { pub block: usize, pub off: usize }

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Selection { pub anchor: Pos, pub head: Pos }

/// One drawn block's text and the message it belongs to (a blank line separates messages in a copy).
#[derive(Clone, Debug, PartialEq)]
pub struct Text { pub text: String, pub entry: usize }

impl Selection {
    pub fn collapsed(&self) -> bool { self.anchor == self.head }
    pub fn ordered(&self) -> (Pos, Pos) { if self.anchor <= self.head { (self.anchor, self.head) } else { (self.head, self.anchor) } }
    /// The part of block `block` (of `len` bytes) the selection covers.
    pub fn within(&self, block: usize, len: usize) -> Option<Range<usize>> {
        let (a, b) = self.ordered();
        if self.collapsed() || block < a.block || block > b.block { return None; }
        let (s, e) = (if block == a.block { a.off.min(len) } else { 0 }, if block == b.block { b.off.min(len) } else { len });
        (s < e).then_some(s..e)
    }
}

fn snap(text: &str, mut o: usize) -> usize { o = o.min(text.len()); while !text.is_char_boundary(o) { o -= 1; } o }

/// Double click: the word at `off`, or the whitespace/punctuation run it lands in.
pub fn word_at(text: &str, off: usize) -> Range<usize> {
    let off = snap(text, off);
    let mut last = 0..0;
    for (k, w) in text.split_word_bound_indices() {
        last = k..k + w.len();
        if off < k + w.len() { return last; }
    }
    last
}

/// Triple click selects the whole block; a position past the end of text is the end of its block.
pub fn block_range(texts: &[Text], block: usize) -> Option<Selection> {
    let t = texts.get(block)?;
    Some(Selection { anchor: Pos { block, off: 0 }, head: Pos { block, off: t.text.len() } })
}

/// The text a Ctrl-C copies: each covered block's part, blocks of one message on separate lines, messages by a blank line.
pub fn extract(texts: &[Text], sel: &Selection) -> String {
    if sel.collapsed() { return String::new(); }
    let (a, b) = sel.ordered();
    let mut out = String::new(); let mut prev: Option<&Text> = None;
    for (i, t) in texts.iter().enumerate().take(b.block + 1).skip(a.block) {
        let (s, e) = (if i == a.block { snap(&t.text, a.off) } else { 0 }, if i == b.block { snap(&t.text, b.off) } else { t.text.len() });
        if i > a.block { out.push_str(if prev.map_or(false, |p| p.entry != t.entry) { "\n\n" } else { "\n" }); }
        if s < e { out.push_str(&t.text[s..e]); }
        prev = Some(t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn t(s: &str, entry: usize) -> Text { Text { text: s.into(), entry } }
    fn p(block: usize, off: usize) -> Pos { Pos { block, off } }
    #[test] fn within_one_block() {
        let s = Selection { anchor: p(0, 2), head: p(0, 5) };
        assert_eq!(s.within(0, 10), Some(2..5)); assert_eq!(s.within(1, 10), None);
        let r = Selection { anchor: p(0, 5), head: p(0, 2) }; assert_eq!(r.within(0, 10), Some(2..5), "backwards selection");
        assert_eq!(Selection { anchor: p(0, 3), head: p(0, 3) }.within(0, 10), None);
    }
    #[test] fn across_blocks_covers_the_middle_whole() {
        let s = Selection { anchor: p(0, 4), head: p(2, 3) };
        assert_eq!(s.within(0, 10), Some(4..10)); assert_eq!(s.within(1, 7), Some(0..7)); assert_eq!(s.within(2, 9), Some(0..3)); assert_eq!(s.within(3, 9), None);
    }
    #[test] fn extract_joins_blocks_and_separates_messages() {
        let texts = [t("hello world", 0), t("second block", 0), t("other message", 1)];
        let s = Selection { anchor: p(0, 6), head: p(2, 5) };
        assert_eq!(extract(&texts, &s), "world\nsecond block\n\nother");
        assert_eq!(extract(&texts, &Selection { anchor: p(1, 3), head: p(1, 3) }), "");
        assert_eq!(extract(&texts, &Selection { anchor: p(2, 5), head: p(0, 6) }), "world\nsecond block\n\nother", "head before anchor");
    }
    #[test] fn extract_is_safe_on_multibyte_and_out_of_range() {
        let texts = [t("日本語のテキスト", 0)];
        let s = Selection { anchor: p(0, 1), head: p(0, 7) };   // inside characters
        let out = extract(&texts, &s); assert!(texts[0].text.contains(&out));
        assert_eq!(extract(&texts, &Selection { anchor: p(0, 0), head: p(5, 99) }), texts[0].text, "past the last block is the end");
    }
    #[test] fn word_and_block_selection() {
        assert_eq!(&"alpha beta"[word_at("alpha beta", 7)], "beta"); assert_eq!(&"alpha beta"[word_at("alpha beta", 0)], "alpha");
        assert_eq!(&"שלום עולם"[word_at("שלום עולם", 9)], "עולם");
        assert_eq!(word_at("", 3), 0..0);
        let texts = [t("one", 0), t("two words", 0)];
        assert_eq!(extract(&texts, &block_range(&texts, 1).unwrap()), "two words"); assert!(block_range(&texts, 9).is_none());
    }
}

/// The selection highlight, one colour for every text surface. GPUI on X11 exposes no system highlight colour, so it is the theme's
/// accent at a fixed alpha (readable under the glyphs in both themes); a platform colour would replace this one function.
pub fn highlight(light: bool) -> gpui::Rgba { let mut a = crate::theme::color(light, "accent"); a.a = if light { 0.30 } else { 0.28 }; a }

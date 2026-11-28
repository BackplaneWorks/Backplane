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
pub struct Text { pub text: String, pub entry: usize, pub more: More }
/// What a block adds to its copy: `whole`, all of a row drawn cut to one line (copied from where the selection starts to the row's real end when
/// the selection runs to the end of what is drawn); `cell`, the table row a cell is in (the cells of one row are joined by a tab, for a sheet);
/// `tight`, a one-line activity row (one newline to its neighbours rather than the blank line between messages).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct More { pub whole: Option<String>, pub cell: Option<u64>, pub tight: bool }
impl Text { pub fn new(text: impl Into<String>, entry: usize) -> Self { Self { text: text.into(), entry, more: More::default() } } }

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

/// Triple click in a block holding newlines (a code block, a file's text): the logical line at `pos`, its newline not included; a
/// position past the end is the last line, an empty line an empty range there. A block without a newline: the whole block.
pub fn line_range(texts: &[Text], pos: Pos) -> Option<Selection> {
    let l = line_at(&texts.get(pos.block)?.text, pos.off);
    Some(Selection { anchor: Pos { block: pos.block, off: l.start }, head: Pos { block: pos.block, off: l.end } })
}
/// The logical line of `text` at `off` (see `line_range`).
pub fn line_at(text: &str, off: usize) -> Range<usize> {
    let off = snap(text, off);
    text[..off].rfind('\n').map_or(0, |i| i + 1)..text[off..].find('\n').map_or(text.len(), |i| off + i)
}

/// The text a Ctrl-C copies: each covered block's part, blocks of one message on separate lines, messages by a blank line (an activity row by one
/// newline), the cells of a table row by a tab; a row drawn cut copies to its real end.
pub fn extract(texts: &[Text], sel: &Selection) -> String {
    if sel.collapsed() { return String::new(); }
    let (a, b) = sel.ordered();
    let mut out = String::new(); let mut prev: Option<&Text> = None;
    for (i, t) in texts.iter().enumerate().take(b.block + 1).skip(a.block) {
        let (s, e) = (if i == a.block { snap(&t.text, a.off) } else { 0 }, if i == b.block { snap(&t.text, b.off) } else { t.text.len() });
        if let (Some(p), true) = (prev, i > a.block) { out.push_str(between(p, t)); }
        if s < e {
            match &t.more.whole { Some(w) if e == t.text.len() => out.push_str(&w[s.min(common_prefix(&t.text, w))..]), _ => out.push_str(&t.text[s..e]) }
        }
        prev = Some(t);
    }
    out
}
/// What separates two blocks in a copy: a tab between cells of one table row, a newline within a message or next to an activity row, else a blank line.
fn between(p: &Text, t: &Text) -> &'static str {
    if p.more.cell.is_some() && p.more.cell == t.more.cell { "\t" } else if p.entry != t.entry && !p.more.tight && !t.more.tight { "\n\n" } else { "\n" }
}
/// How many leading bytes two texts share, on a character boundary (a cut row's drawn line and its whole text).
fn common_prefix(a: &str, b: &str) -> usize { a.char_indices().zip(b.chars()).find(|((_, x), y)| x != y).map_or(a.len().min(b.len()), |((i, _), _)| i) }

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn t(s: &str, entry: usize) -> Text { Text::new(s, entry) }
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
    // a sheet takes a pasted table as tab-separated rows; the rest of the copy is unchanged
    #[test] fn a_tables_cells_copy_tab_separated_and_rows_by_line() {
        let cell = |s: &str, row: u64| Text { more: More { cell: Some(row), ..More::default() }, ..t(s, 0) };
        let texts = [t("Parts:", 0), cell("Ref", 1), cell("Value", 1), cell("Note", 1), cell("R1", 2), cell("100k", 2), cell("anode load", 2), cell("R2", 3), cell("1k5", 3), cell("cathode bias", 3), t("Done.", 0)];
        assert_eq!(extract(&texts, &Selection { anchor: p(4, 0), head: p(9, 12) }), "R1\t100k\tanode load\nR2\t1k5\tcathode bias");
        assert_eq!(extract(&texts, &Selection { anchor: p(0, 0), head: p(10, 5) }), "Parts:\nRef\tValue\tNote\nR1\t100k\tanode load\nR2\t1k5\tcathode bias\nDone.");
        let empty = [cell("a", 7), cell("", 7), cell("c", 7)];
        assert_eq!(extract(&empty, &Selection { anchor: p(0, 0), head: p(2, 1) }), "a\t\tc", "an empty cell keeps its column");
    }
    #[test] fn activity_rows_copy_in_reading_order_one_line_each_and_whole() {
        let row = |s: &str, whole: Option<&str>, entry: usize| Text { more: More { whole: whole.map(str::to_string), tight: true, ..More::default() }, ..t(s, entry) };
        let texts = [t("I will look.", 0), row("grep -n go board.kicad_pcb # 1", None, 1), row("cat <<EOF \u{2026}", Some("cat <<EOF\nline\nEOF"), 2), t("Found it.", 3), t("Next.", 4)];
        assert_eq!(extract(&texts, &Selection { anchor: p(0, 7), head: p(3, 5) }), "look.\ngrep -n go board.kicad_pcb # 1\ncat <<EOF\nline\nEOF\nFound", "one newline next to a row, the cut one whole");
        assert_eq!(extract(&texts, &Selection { anchor: p(3, 0), head: p(4, 5) }), "Found it.\n\nNext.", "messages keep their blank line");
        assert_eq!(extract(&texts, &Selection { anchor: p(2, 4), head: p(2, 6) }), "<<", "a part of what is drawn is itself");
        assert_eq!(extract(&texts, &Selection { anchor: p(2, 4), head: p(2, 13) }), "<<EOF\nline\nEOF", "to the drawn end: on to the real end");
        assert_eq!(common_prefix("héllo …", "héllo\nx"), 6); assert_eq!(common_prefix("ab", "abc"), 2);
    }
    #[test] fn word_and_block_selection() {
        assert_eq!(&"alpha beta"[word_at("alpha beta", 7)], "beta"); assert_eq!(&"alpha beta"[word_at("alpha beta", 0)], "alpha");
        assert_eq!(&"שלום עולם"[word_at("שלום עולם", 9)], "עולם");
        assert_eq!(word_at("", 3), 0..0);
        let texts = [t("one", 0), t("two words", 0)];
        assert_eq!(extract(&texts, &block_range(&texts, 1).unwrap()), "two words"); assert!(block_range(&texts, 9).is_none());
    }
    #[test] fn a_triple_click_takes_the_line_not_the_block() {
        let texts = [t("a b\ncd ef\n\ngh", 0), t("two words", 0)];
        let line = |b: usize, o: usize| { let s = line_range(&texts, p(b, o)).unwrap(); (s.anchor.block == b && s.head.block == b).then_some(s.anchor.off..s.head.off) };
        assert_eq!(line(0, 1), Some(0..3)); assert_eq!(line(0, 3), Some(0..3)); assert_eq!(line(0, 4), Some(4..9)); assert_eq!(line(0, 9), Some(4..9));
        assert_eq!(line(0, 10), Some(10..10), "an empty line selects nothing more"); assert_eq!(line(0, 12), Some(11..13)); assert_eq!(line(0, 99), Some(11..13));
        assert_eq!(line(1, 4), Some(0..9), "no newline: the whole block"); assert_eq!(line_range(&texts, p(1, 4)), block_range(&texts, 1));
        assert!(line_range(&texts, p(9, 0)).is_none());
    }
}

/// The selection highlight, one colour for every text surface. GPUI on X11 exposes no system highlight colour, so it is the theme's
/// accent at a fixed alpha (readable under the glyphs in both themes); a platform colour would replace this one function.
pub fn highlight(light: bool) -> gpui::Rgba { let mut a = crate::theme::color(light, "accent"); a.a = if light { 0.30 } else { 0.28 }; a }

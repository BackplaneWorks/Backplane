//! The composer's text model: pure, no GPUI. A buffer with one selection (anchor + head, byte offsets on
//! grapheme boundaries), word and line motions, multi-line text and an undo history that merges a run of
//! typing. Mirrors src/core/edit's behaviour; the view only maps keys and mouse onto these calls.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq)]
struct Snap { text: String, anchor: usize, head: usize }

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { Type, Delete, Other }

#[derive(Debug, Clone)]
pub struct Editor {
    text: String, anchor: usize, head: usize,
    undo: Vec<Snap>, redo: Vec<Snap>, last: Kind,
    /// IME composition range, replaced by the next commit.
    pub marked: Option<Range<usize>>,
}

const HISTORY: usize = 200;

impl Default for Editor { fn default() -> Self { Self::new("") } }

impl Editor {
    pub fn new(text: &str) -> Self { let n = text.len(); Self { text: text.into(), anchor: n, head: n, undo: vec![], redo: vec![], last: Kind::Other, marked: None } }
    pub fn text(&self) -> &str { &self.text }
    pub fn head(&self) -> usize { self.head }
    pub fn selection(&self) -> Range<usize> { self.anchor.min(self.head)..self.anchor.max(self.head) }
    pub fn selected(&self) -> &str { &self.text[self.selection()] }
    pub fn is_empty(&self) -> bool { self.text.is_empty() }
    pub fn lines(&self) -> usize { self.text.split('\n').count() }

    /// Replace everything (a thread switch, a restored draft, a sent message). History is dropped.
    pub fn set(&mut self, text: &str) { *self = Self::new(text); }

    fn snap(&self) -> Snap { Snap { text: self.text.clone(), anchor: self.anchor, head: self.head } }
    fn record(&mut self, kind: Kind) {
        // a run of typing, or of deleting, is one undo step
        if !(kind != Kind::Other && self.last == kind && !self.undo.is_empty()) {
            self.undo.push(self.snap());
            if self.undo.len() > HISTORY { self.undo.remove(0); }
        }
        self.redo.clear();
        self.last = kind;
    }
    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(self.snap());
        self.text = s.text; self.anchor = s.anchor; self.head = s.head; self.last = Kind::Other; self.marked = None; true
    }
    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(self.snap());
        self.text = s.text; self.anchor = s.anchor; self.head = s.head; self.last = Kind::Other; self.marked = None; true
    }

    fn replace(&mut self, range: Range<usize>, with: &str, kind: Kind) {
        self.record(kind);
        self.marked = None;
        self.text.replace_range(range.clone(), with);
        self.head = range.start + with.len(); self.anchor = self.head;
    }
    /// Typing and paste. Line endings are normalised to \n.
    pub fn insert(&mut self, s: &str) {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        if s.is_empty() { return; }
        let r = self.marked.take().unwrap_or_else(|| self.selection());
        let kind = if s.len() <= 4 && !s.contains('\n') && r.is_empty() { Kind::Type } else { Kind::Other };
        self.replace(r, &s, kind);
    }
    pub fn newline(&mut self) { self.insert("\n"); }

    pub fn prev(&self, i: usize) -> usize { self.text[..i].grapheme_indices(true).next_back().map_or(0, |(k, _)| k) }
    pub fn next(&self, i: usize) -> usize { self.text[i..].graphemes(true).next().map_or(self.text.len(), |g| i + g.len()) }
    pub fn word_start(&self, i: usize) -> usize {
        let mut it = self.text[..i].split_word_bound_indices().rev().peekable();
        while let Some((k, w)) = it.next() { if w.chars().any(char::is_alphanumeric) { return k; } if it.peek().is_none() { return 0; } }
        0
    }
    pub fn word_end(&self, i: usize) -> usize {
        for (k, w) in self.text[i..].split_word_bound_indices() { if w.chars().any(char::is_alphanumeric) { return i + k + w.len(); } }
        self.text.len()
    }
    pub fn line_start(&self, i: usize) -> usize { self.text[..i].rfind('\n').map_or(0, |k| k + 1) }
    pub fn line_end(&self, i: usize) -> usize { self.text[i..].find('\n').map_or(self.text.len(), |k| i + k) }

    fn go(&mut self, to: usize, extend: bool) { self.head = to; if !extend { self.anchor = to; } self.last = Kind::Other; }
    pub fn left(&mut self, extend: bool, word: bool) {
        if !extend && self.anchor != self.head { self.go(self.selection().start, false); return; }
        let t = if word { self.word_start(self.head) } else { self.prev(self.head) };
        self.go(t, extend);
    }
    pub fn right(&mut self, extend: bool, word: bool) {
        if !extend && self.anchor != self.head { self.go(self.selection().end, false); return; }
        let t = if word { self.word_end(self.head) } else { self.next(self.head) };
        self.go(t, extend);
    }
    pub fn home(&mut self, extend: bool) { let t = self.line_start(self.head); self.go(t, extend); }
    pub fn end(&mut self, extend: bool) { let t = self.line_end(self.head); self.go(t, extend); }
    pub fn doc_start(&mut self, extend: bool) { self.go(0, extend); }
    pub fn doc_end(&mut self, extend: bool) { self.go(self.text.len(), extend); }
    /// Up/down by logical line, keeping the column in characters; the view uses wrapped rows when it can.
    pub fn up(&mut self, extend: bool) {
        let s = self.line_start(self.head);
        if s == 0 { return self.go(0, extend); }
        let col = self.text[s..self.head].chars().count();
        let ps = self.line_start(s - 1);
        self.go(self.at_col(ps, s - 1, col), extend);
    }
    pub fn down(&mut self, extend: bool) {
        let e = self.line_end(self.head);
        if e == self.text.len() { return self.go(e, extend); }
        let col = self.text[self.line_start(self.head)..self.head].chars().count();
        let ns = e + 1;
        self.go(self.at_col(ns, self.line_end(ns), col), extend);
    }
    fn at_col(&self, start: usize, end: usize, col: usize) -> usize {
        self.text[start..end].char_indices().nth(col).map_or(end, |(k, _)| start + k)
    }
    pub fn select_all(&mut self) { self.anchor = 0; self.head = self.text.len(); }
    pub fn place(&mut self, offset: usize, extend: bool) { let o = self.clamp(offset); self.go(o, extend); }
    fn clamp(&self, mut o: usize) -> usize { o = o.min(self.text.len()); while !self.text.is_char_boundary(o) { o -= 1; } o }
    /// Double click: the word under `offset`; triple click: its line.
    pub fn select_word_at(&mut self, offset: usize) {
        let o = self.clamp(offset);
        for (k, w) in self.text.split_word_bound_indices() { if o >= k && o <= k + w.len() && w.chars().any(char::is_alphanumeric) { self.anchor = k; self.head = k + w.len(); return; } }
        self.anchor = o; self.head = o;
    }
    pub fn select_line_at(&mut self, offset: usize) { let o = self.clamp(offset); self.anchor = self.line_start(o); self.head = self.line_end(o); }

    pub fn backspace(&mut self, word: bool) {
        let sel = self.selection();
        if !sel.is_empty() { return self.replace(sel, "", Kind::Other); }
        if self.head == 0 { return; }
        let from = if word { self.word_start(self.head) } else { self.prev(self.head) };
        self.replace(from..self.head, "", if word { Kind::Other } else { Kind::Delete });
    }
    pub fn delete(&mut self, word: bool) {
        let sel = self.selection();
        if !sel.is_empty() { return self.replace(sel, "", Kind::Other); }
        if self.head == self.text.len() { return; }
        let to = if word { self.word_end(self.head) } else { self.next(self.head) };
        self.replace(self.head..to, "", if word { Kind::Other } else { Kind::Delete });
    }
    /// An IME update: replace `range` (default: the composition, else the selection) and keep the new text
    /// marked while composing. `sel` is relative to the replaced text's start.
    pub fn ime(&mut self, range: Option<Range<usize>>, text: &str, mark: bool, sel: Option<Range<usize>>) {
        let r = range.or_else(|| self.marked.clone()).map(|r| { let (a, b) = (self.clamp(r.start), self.clamp(r.end)); a.min(b)..a.max(b) }).unwrap_or_else(|| self.selection());
        self.record(Kind::Other);
        self.text.replace_range(r.clone(), text);
        self.marked = if mark && !text.is_empty() { Some(r.start..r.start + text.len()) } else { None };
        let (a, h) = match sel { Some(s) => (r.start + s.start.min(text.len()), r.start + s.end.min(text.len())), None => (r.start + text.len(), r.start + text.len()) };
        self.anchor = self.clamp(a); self.head = self.clamp(h);
    }
    pub fn set_selection(&mut self, r: Range<usize>) { self.anchor = self.clamp(r.start); self.head = self.clamp(r.end); }
    pub fn cut(&mut self) -> String { let s = self.selected().to_string(); let r = self.selection(); if !r.is_empty() { self.replace(r, "", Kind::Other); } s }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn typed(s: &str) -> Editor { let mut e = Editor::default(); for c in s.chars() { e.insert(&c.to_string()); } e }
    #[test] fn typing_is_one_undo_step_and_redo_restores() {
        let mut e = typed("hello world");
        e.undo(); assert_eq!(e.text(), "");
        e.redo(); assert_eq!(e.text(), "hello world");
        assert!(!e.redo());
    }
    #[test] fn a_new_edit_clears_redo() {
        let mut e = typed("ab"); e.undo(); e.insert("x"); assert!(!e.redo()); assert_eq!(e.text(), "x");
    }
    #[test] fn paste_breaks_the_typing_run() {
        let mut e = typed("ab"); e.insert("PASTE"); e.insert("c");
        e.undo(); assert_eq!(e.text(), "abPASTE");
        e.undo(); assert_eq!(e.text(), "ab");
    }
    #[test] fn multi_line_with_crlf_and_vertical_motion() {
        let mut e = Editor::default(); e.insert("one\r\ntwo three\rfour");
        assert_eq!(e.text(), "one\ntwo three\nfour"); assert_eq!(e.lines(), 3);
        e.up(false); assert_eq!(e.head(), 8); // column 4 of "two three"
        e.up(false); assert_eq!(e.head(), 3); // clamps to the end of the shorter first line
        e.down(false); assert_eq!(e.head(), 7); e.down(false); assert_eq!(e.head(), 17); e.down(false); assert_eq!(e.head(), e.text().len());
    }
    #[test] fn word_motions_and_word_delete() {
        let mut e = typed("foo bar-baz");
        e.left(false, true); assert_eq!(&e.text()[e.head()..], "baz");
        e.left(false, true); assert_eq!(&e.text()[e.head()..], "bar-baz");
        e.right(false, true); assert_eq!(e.head(), "foo bar".len());
        e.backspace(true); assert_eq!(e.text(), "foo -baz");
    }
    #[test] fn double_click_selects_word_triple_selects_line() {
        let mut e = typed("alpha beta\ngamma");
        e.select_word_at(7); assert_eq!(e.selected(), "beta");
        e.select_line_at(7); assert_eq!(e.selected(), "alpha beta");
        e.select_line_at(13); assert_eq!(e.selected(), "gamma");
    }
    #[test] fn home_end_and_shift_extend_within_a_line() {
        let mut e = typed("ab\ncdef"); e.left(false, false); e.left(false, false);
        e.home(true); assert_eq!(e.selected(), "cd"); e.end(false); e.home(false); e.end(true); assert_eq!(e.selected(), "cdef");
    }
    #[test] fn emoji_and_combining_clusters_move_as_one() {
        let mut e = typed("a👨‍👩‍👧b"); e.left(false, false); e.left(false, false);
        assert_eq!(e.head(), 1); e.right(false, false); assert_eq!(e.head(), "a👨‍👩‍👧".len());
        e.backspace(false); assert_eq!(e.text(), "ab");
        let mut f = typed("e\u{301}x"); f.left(false, false); f.backspace(false); assert_eq!(f.text(), "x");
    }
    #[test] fn rtl_and_cjk_text_edit_by_cluster() {
        let mut e = typed("שלום 世界"); e.backspace(false); assert_eq!(e.text(), "שלום 世");
        e.select_all(); assert_eq!(e.selected(), "שלום 世");
    }
    #[test] fn ime_composition_is_replaced_by_the_commit() {
        let mut e = typed("x"); e.insert("に"); e.marked = Some(1..1 + "に".len());
        e.insert("日本"); assert_eq!(e.text(), "x日本"); assert!(e.marked.is_none());
    }
    #[test] fn ime_marks_then_commits_and_keeps_one_undo_step_per_update() {
        let mut e = typed("a");
        e.ime(None, "ni", true, None); assert_eq!(e.text(), "ani"); assert_eq!(e.marked, Some(1..3));
        e.ime(None, "你", true, None); assert_eq!(e.text(), "a你");
        e.ime(None, "你", false, None); assert!(e.marked.is_none()); assert_eq!(e.text(), "a你");
    }
    #[test] fn editing_during_composition_clears_the_mark_and_a_stale_range_is_clamped() {
        let mut e = typed("ab"); e.ime(None, "ni", true, None); assert!(e.marked.is_some());
        e.backspace(false); assert!(e.marked.is_none()); assert_eq!(e.text(), "abn");
        e.ime(None, "xyz", true, None); e.marked = Some(2..99);           // stale, past the end
        e.ime(None, "q", false, None); assert_eq!(e.text(), "abq");
        let mut f = typed("abc"); f.ime(None, "d", true, None); f.select_all(); f.cut(); assert!(f.marked.is_none());
        f.ime(Some(5..9), "!", false, None); assert_eq!(f.text(), "!");
    }
    #[test] fn cut_returns_selection_and_is_undoable() {
        let mut e = typed("hello"); e.select_all(); assert_eq!(e.cut(), "hello"); assert!(e.is_empty()); e.undo(); assert_eq!(e.text(), "hello");
    }
}

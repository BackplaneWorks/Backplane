//! The thread's header. Its name (and the ▾ of its menu) is in the window's top bar; its state, its to-do and subagent cells and its
//! tools (the hub's `thread.tools`) go beside the name in that bar when the bar has the room, and in a row of their own under it when it
//! has not. The room is measured, not guessed: the top bar's two groups and the state report the widths they drew at (none of them
//! shrinks, so that is their natural width wherever they sit), and the name and the tools' words are shaped from their text on the
//! frame they change. `one_row` adds them up against the window's width. None of the widths depends on the layout picked, so crossing
//! the line is a single switch, never a back-and-forth, and the name never moves.
//!
//! The tools are buttons of one shape (icon and word, one height, one padding) with no rules between them: their groups (the turn's
//! own Stop or Retry, Buy it, Pin/Settle/Archive, Fork/Back) are told apart by spacing alone. Buy it is the one framed chip, its estimate
//! a quieter second half of it. In a row of their own that is too narrow for the words (a viewer open beside a small window), Pin,
//! Settle, Archive and Fork keep their icons and give their words to their tooltips; nothing leaves the row.
use super::tools::{press, tools, Item};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{cell::{Cell, RefCell}, rc::Rc};

/// The second row's height, and the sidebar's first row's, so their bottom rules are one line across the window.
pub const ROW_H: f32 = 32.;
/// The top bar's own side padding (px_3).
pub const PAD: f32 = 12.;
/// The least room between two groups of the top bar (name, state, tools, the window's own controls).
pub const GAP: f32 = 24.;
/// The second row's padding after the tools.
pub const ROW_END: f32 = 6.;
/// A tool button: its height, text size, icon size, padding either side of it and the gap between icon and word.
pub const BTN_H: f32 = 24.;
pub const TEXT: f32 = 11.;
const ICON: f32 = 13.;
const PL: f32 = 6.;
const PR: f32 = 8.;
const ICON_GAP: f32 = 6.;
/// Room between tools of one group, and between groups.
const IN_GROUP: f32 = 2.;
const BETWEEN: f32 = 14.;

/// What the header measured and shaped, kept between frames.
#[derive(Default)]
pub struct Head {
    /// The top bar's left group (logo, crumbs, the name and its ▾), the name alone in it, and the right group (Jump, Improvements, Search, Settings).
    pub left: Rc<Cell<f32>>, pub title: Rc<Cell<f32>>, pub right: Rc<Cell<f32>>,
    /// The thread's state and cells; the second row's own width (the thread column's).
    pub state: Rc<Cell<f32>>, pub row: Rc<Cell<f32>>,
    /// The name's text and its shaped width; the tools' words and the strip's widths (with words, with icons only).
    shaped: RefCell<(String, f32)>, strip: RefCell<(String, (f32, f32))>,
    /// Each tool's width when tight (its icon alone, or its word when it has no icon), for `keep_when_tight`.
    tight_w: RefCell<Vec<f32>>,
    /// The tools the last layout did not draw (bit i = tool i; none unless tight): they go into the thread's menus (`dropped_items`).
    pub dropped: Cell<u64>,
}

/// How the header is laid out this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout { pub one: bool, pub icons: bool, pub tight: bool, /** tight: which tools still fit (bit i = tool i), the rest are in the thread's menu */ pub keep: u64 }

/// Whether the top bar holds the name, the state and the tools side by side with the window's controls: every width known and the
/// sum, with the bar's padding and the gaps between the four groups, within the window's width.
pub fn one_row(win_w: f32, left: f32, state: f32, tools: f32, right: f32) -> bool {
    left > 0. && state > 0. && right > 0. && 2. * PAD + left + GAP + state + GAP + tools + GAP + right <= win_w
}
/// In a row of their own: whether the tools give up their words (the row is measured and too narrow for the state and the worded tools).
pub fn icons_only(row_w: f32, state: f32, tools: f32) -> bool { row_w > 0. && state + GAP + tools + ROW_END > row_w }
/// Folded and still too narrow (the strip with its foldable tools as icons): every tool with an icon shows the icon alone, Stop and Buy it too.
pub fn tight(row_w: f32, state: f32, icons: f32) -> bool { row_w > 0. && state + GAP + icons + ROW_END > row_w }

/// Tight and still too narrow for every icon: which tools stay (bit i = tool i). The branching ones go first, then the keeping ones, then
/// Buy it, Stop/Retry last, each group from its end (they are all in the thread's menu, the ▾ by its name and a right-click on the header).
/// `tools` is each tool's group and tight width, in the strip's order; `room` the width the strip may take.
pub fn keep_when_tight(tools: &[(u8, f32)], room: f32) -> u64 {
    let mut keep: Vec<bool> = vec![true; tools.len()];
    let width = |keep: &[bool]| { let (mut w, mut prev) = (0., None); for (i, (g, tw)) in tools.iter().enumerate() { if keep[i] { w += space(prev, *g) + tw; prev = Some(*g); } } w };
    while width(&keep) > room {
        let Some(i) = (0..tools.len()).filter(|&i| keep[i]).max_by_key(|&i| ([3u8, 2, 1, 0].iter().position(|g| *g == tools[i].0).map_or(0, |p| 4 - p), i)) else { break };
        if keep.iter().filter(|k| **k).count() <= 1 { break }
        keep[i] = false;
    }
    keep.iter().enumerate().filter(|(i, k)| **k && *i < 64).fold(0u64, |m, (i, _)| m | 1 << i)
}

/// The left group's width with this frame's name in it: the measured group, less the name it was measured with, plus the new name's shaped width.
pub fn left_now(left: f32, title_was: f32, title_now: f32) -> f32 { if left <= 0. { 0. } else { left - title_was + title_now } }

/// Which group of the strip a tool is in: the turn's own, buying, keeping (pin, settle, archive) or branching (fork, back). Spacing tells them apart.
pub fn group(action: &str) -> u8 {
    match action { "interrupt" | "retry" => 0, "fab-buy" => 1, "pin" | "unpin" | "settle" | "unsettle" | "archive" | "unarchive" => 2, _ => 3 }
}
/// Whether a tool can show its icon alone (its word in its tooltip): the keeping and branching ones, when the hub gave them an icon.
pub fn can_fold(t: &Item) -> bool { group(&t.action) >= 2 && !t.icon.is_empty() }
/// Buy it's words as the chip shows them: the word, and its estimate (what follows the first " · ") as a quieter second half.
pub fn split_buy(label: &str) -> (&str, &str) { label.split_once(" · ").unwrap_or((label, "")) }
/// The room before a tool: none first, a little within a group, more between groups.
fn space(prev: Option<u8>, g: u8) -> f32 { match prev { None => 0., Some(p) if p == g => IN_GROUP, _ => BETWEEN } }

/// The strip's width as `button` and `strip` draw it, with words and with the foldable ones as icons, from each word's shaped width.
pub fn strip_width(items: &[Item], shape: &mut dyn FnMut(&str) -> f32) -> (f32, f32) {
    let (mut full, mut icons, mut prev) = (0., 0., None);
    for t in items {
        let g = group(&t.action); let sp = space(prev, g); prev = Some(g);
        let icon = if t.icon.is_empty() { 0. } else { ICON + ICON_GAP };
        let label = crate::i18n::trs(&t.label); // measured as drawn (button)
        let w = if t.action == "fab-buy" { let (word, est) = split_buy(&label); 2. + PL + icon + shape(word) + if est.is_empty() { 0. } else { ICON_GAP + shape(est) } + PR } else { PL + icon + shape(&label) + PR };
        full += sp + w;
        icons += sp + if can_fold(t) { PL + ICON + PL } else { w };
    }
    (full, icons)
}

/// The tools a tight header did not draw (`Head::dropped`), first in the thread's menus (the ▾ by its name, a right-click on the header): Buy it by its word.
pub fn dropped_items(thread: &Value, dropped: u64) -> Vec<super::ctxmenu::Item> {
    tools(thread).into_iter().enumerate().filter(|(i, _)| *i < 64 && dropped & (1 << i) != 0)
        .map(|(_, t)| super::ctxmenu::Item { label: if t.action == "fab-buy" { split_buy(&t.label).0.to_string() } else { t.label.clone() }, action: t.action, value: t.value, confirm: t.danger, note: String::new() }).collect()
}

/// Each tool's width when the strip is tight (`strip` with `Layout::tight`): an icon alone, or the word of a tool with no icon.
pub fn tight_widths(items: &[Item], shape: &mut dyn FnMut(&str) -> f32) -> Vec<f32> {
    items.iter().map(|t| if t.icon.is_empty() { PL + shape(&crate::i18n::trs(&t.label)) + PR } else { PL + ICON + PL + if t.action == "fab-buy" { 2. } else { 0. } }).collect()
}

impl Head {
    /// This frame's layout. `font` is the window's text font, `title_size` the name's text size.
    pub fn layout(&self, window: &mut Window, win_w: f32, title: &str, items: &[Item], font: &str, title_size: f32) -> Layout {
        let shape = |window: &mut Window, text: &str, size: f32, weight: FontWeight| -> f32 {
            if text.is_empty() { return 0. }
            let run = TextRun { len: text.len(), font: Font { weight, ..gpui::font(SharedString::from(font.to_string())) }, color: Hsla::default(), background_color: None, underline: None, strikethrough: None };
            f32::from(window.text_system().shape_line(SharedString::from(text.to_string()), px(size), &[run], None).width)
        };
        let now = { let mut s = self.shaped.borrow_mut(); if s.0 != title || (s.1 <= 0. && !title.is_empty()) { *s = (title.to_string(), shape(window, title, title_size, FontWeight::BOLD)); } s.1 };
        let key = items.iter().map(|t| format!("{}\u{1f}{}\u{1f}{}", t.action, t.icon, t.label)).collect::<Vec<_>>().join("\u{1e}") + font + crate::i18n::tag() + &crate::scale::s().to_string();
        let (full, folded) = { let mut s = self.strip.borrow_mut(); if s.0 != key { let w = strip_width(items, &mut |t| shape(window, t, TEXT * crate::scale::s(), FontWeight::NORMAL)); *self.tight_w.borrow_mut() = tight_widths(items, &mut |t| shape(window, t, TEXT * crate::scale::s(), FontWeight::NORMAL)); *s = (key, w); } s.1 };
        let one = one_row(win_w, left_now(self.left.get(), self.title.get(), now), self.state.get(), full, self.right.get());
        let is_tight = !one && tight(self.row.get(), self.state.get(), folded);
        let keep = if is_tight { let tw = self.tight_w.borrow(); keep_when_tight(&items.iter().zip(tw.iter()).map(|(t, w)| (group(&t.action), *w)).collect::<Vec<_>>(), self.row.get() - self.state.get() - GAP - ROW_END) } else { u64::MAX };
        self.dropped.set(!keep);
        Layout { one, icons: !one && icons_only(self.row.get(), self.state.get(), full), tight: is_tight, keep }
    }
}

/// An invisible layer that keeps `cell` at the width its parent drew at, and redraws the window when it changes (the parent must be relative).
pub fn measure(cx: &mut Context<Backplane>, cell: Rc<Cell<f32>>) -> impl IntoElement {
    let me = cx.entity().downgrade();
    canvas(move |b, _, cx| {
        let w = f32::from(b.size.width); if (cell.get() - w).abs() < 0.5 { return }
        cell.set(w); if let Some(this) = me.upgrade() { cx.defer(move |cx| this.update(cx, |_, cx| cx.notify())) }
    }, |_, _, _, _| {}).absolute().size_full()
}

/// The words a phase can show, the longest of which the state keeps room for, so a change of word moves nothing after it.
const PHASES: [&str; 9] = ["active", "blocked", "failed", "finished", "inactive", "settled", "stalled", "stopped", "waiting"];
/// `widest_phase` in the language in force: the widest of the words as drawn (a CJK character counts twice), so the room fits the word shown.
pub fn shown_room(shown: &str) -> String {
    let w = |s: &str| s.chars().map(|c| if (c as u32) >= 0x1100 { 2 } else { 1 }).sum::<usize>();
    PHASES.iter().map(|p| crate::i18n::trs(p)).chain(std::iter::once(shown.to_string())).max_by_key(|s| w(s)).unwrap_or_default()
}
pub fn widest_phase(now: &str) -> &str { PHASES.iter().copied().chain(std::iter::once(now)).max_by_key(|w| w.chars().count()).unwrap_or(now) }

/// The thread's state ("STATE ● waiting", its word's room kept) and its cells, measured.
pub fn state(cx: &mut Context<Backplane>, light: bool, thread: &Value, cells: Vec<AnyElement>, cell: Rc<Cell<f32>>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let word = string(thread, "phase").split(':').next().unwrap_or("").trim().to_lowercase();
    let shown = crate::i18n::trs(&word);
    let room = shown_room(&shown);
    div().id("thread-state").relative().flex_shrink_0().h_full().flex().items_center().child(measure(cx, cell))
        .child(div().px_3().flex().items_center().gap_2().text_size(crate::scale::tpx(TEXT)).child(div().text_color(c("faint")).child(crate::i18n::trs("STATE")))
            // the thread's dot in the sidebar rows' 0.14 colours
            .child(super::siderow::dot(light, &serde_json::json!({"status": word, "state": thread["state"]})))
            // two lines in one: the widest word sets the width (with no height), the word itself shows
            .child(div().flex().flex_col().child(div().h(px(0.)).overflow_hidden().child(room)).child(div().whitespace_nowrap().child(shown))))
        .children(cells).into_any_element()
}

fn button(cx: &mut Context<Backplane>, light: bool, i: usize, t: &Item, fold: bool, tip: (String, String)) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let (a, v) = match press(t) { Command::Action(a, v) => (a, v), _ => (String::new(), String::new()) };
    let rect = format!("tool:{a}"); // a header tool reports its rectangle (tool:<action>), so a run can press Buy it as a person would
    let buy = t.action == "fab-buy";
    let label = crate::i18n::trs(&t.label);
    let (word, est) = if buy { split_buy(&label) } else { (label.as_str(), "") };
    let (fg, ic) = if t.danger { ("bad", "bad") } else if buy { ("accent", "accent") } else if t.on { ("text", "accent") } else { ("dim", "dim") };
    let d = crate::ui::with_rect(div().id(SharedString::from(format!("tool-{i}"))), move || rect).h(crate::scale::tpx(BTN_H)).flex().items_center().gap(px(ICON_GAP)).pl(px(PL)).pr(px(if fold { PL } else { PR }))
        .text_size(crate::scale::tpx(TEXT)).whitespace_nowrap().cursor_pointer().text_color(c(fg)).hover(|s| s.bg(c("raise")).text_color(c(if t.danger { "bad" } else if buy { "accent" } else { "text" })))
        .when(t.on && !buy, |d| d.bg(c("raise")))
        // Buy it is the one framed chip; it alone gives way (its estimate ellipsised) when even the icons do not fit, and clips, never paints over Stop
        .when(buy, |d| d.min_w_0().overflow_hidden().border_1().border_color(c("line")).bg(c("panel"))).when(buy && !fold, |d| d.flex_shrink()).when(!buy || fold, |d| d.flex_shrink_0());
    // an icon the set lacks keeps its room (art::icon draws an empty square too), so the width is what strip_width counts
    let d = if t.icon.is_empty() { d } else { d.child(div().flex_shrink_0().size(px(ICON)).children(crate::art::icon_name(&t.icon).map(|n| crate::art::icon(n, ICON, c(ic))))) };
    let d = if fold { d } else { d.child(div().flex_shrink_0().child(word.to_string())) };
    let d = d.when(!fold && !est.is_empty(), |d| d.child(div().min_w_0().overflow_hidden().text_ellipsis().text_color(c("faint")).child(est.to_string())));
    d.tooltip(crate::keys::tip(&tip.0, &tip.1, light)).on_click(cx.listener(move |this, _, _, cx| { super::settings::flush_fields(this); if super::tools::makes_thread(&a) { this.arm_new(); } let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); }))
}

/// The tools: one shape each, the groups apart by spacing alone; with `icons`, the foldable ones as icons with their words in their tooltips;
/// `tight`, every tool that has an icon (Buy it without its estimate).
pub fn strip(cx: &mut Context<Backplane>, light: bool, thread: &Value, local: &crate::local::Local, l: Layout) -> AnyElement {
    let mut row = div().id("thread-tools").min_w_0().h_full().flex().items_center().overflow_hidden();
    let mut prev: Option<u8> = None;
    for (i, t) in tools(thread).into_iter().enumerate() {
        if l.tight && i < 64 && l.keep & (1 << i) == 0 { continue } // no room even for its icon: it is in the thread's menu
        let k = if t.tip.contains("Ctrl+") { String::new() } else { crate::keys::hint(local, crate::keys::id_for(&t.action, &t.value, "")) };
        let g = group(&t.action); let sp = space(prev, g); prev = Some(g);
        let fold = (l.icons && can_fold(&t)) || (l.tight && !t.icon.is_empty());
        // folded, the tooltip leads with the word the button no longer shows
        let (label, tipw) = (crate::i18n::trs(&t.label), crate::i18n::trs(&t.tip));
        /* i18n-ok: a label and its tip, both through trs */ let tip = if t.tip.is_empty() { label } else if fold { format!("{label}: {tipw}") } else { tipw };
        row = row.child(button(cx, light, i, &t, fold, (tip, k)).ml(px(sp)));
    }
    row.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn item(action: &str, icon: &str, label: &str) -> Item { Item { label: label.into(), icon: icon.into(), tip: String::new(), action: action.into(), value: String::new(), on: false, danger: false, options: vec![] } }
    #[test] fn one_row_needs_every_width_and_the_room_for_all_four_groups() {
        // 12 + 400 + 24 + 120 + 24 + 450 + 24 + 470 + 12 = 1536
        assert!(one_row(1536., 400., 120., 450., 470.));
        assert!(!one_row(1535., 400., 120., 450., 470.));
        assert!(!one_row(4000., 0., 120., 450., 470.), "not measured yet: two rows");
        assert!(!one_row(4000., 400., 0., 450., 470.));
        assert!(one_row(4000., 400., 120., 0., 470.), "a thread with no tools still fits");
    }
    #[test] fn the_choice_does_not_depend_on_the_layout_so_it_cannot_flip_back() {
        // the inputs are natural widths, the same in either layout: the answer only grows with the window
        let mut was = false;
        for w in (800..2400).step_by(7) { let now = one_row(w as f32, 380., 140., 520., 470.); assert!(!was || now, "flipped back at {w}"); was = now; }
        assert!(was);
        let mut was = true;
        for w in (300..1200).step_by(5) { let now = icons_only(w as f32, 140., 520.); assert!(was || !now, "words came and went at {w}"); was = now; }
        assert!(!was && !icons_only(0., 140., 520.), "an unmeasured row keeps its words");
    }
    #[test] fn a_new_title_is_counted_before_it_is_drawn() {
        assert_eq!(left_now(400., 150., 260.), 510.); assert_eq!(left_now(0., 0., 260.), 0.);
    }
    #[test] fn the_state_keeps_room_for_its_widest_word() {
        assert_eq!(shown_room("idle"), widest_phase("idle"), "in English the room is the widest phase");
        crate::i18n::with_lang("ja", || { let r = shown_room(&crate::i18n::trs("waiting")); assert!(PHASES.iter().any(|p| crate::i18n::trs(p) == r), "{r}"); assert!(!r.is_ascii(), "{r}"); });
        assert_eq!(widest_phase("idle").chars().count(), 8); assert_eq!(widest_phase("a-much-longer-word"), "a-much-longer-word");
    }
    #[test] fn tools_fall_in_four_groups_and_buy_it_splits_at_its_first_dot() {
        let g: Vec<u8> = ["interrupt", "fab-buy", "pin", "settle", "unarchive", "fork", "back"].iter().map(|a| group(a)).collect();
        assert_eq!(g, [0, 1, 2, 2, 2, 3, 3]);
        assert_eq!(split_buy("Buy it · last estimate: from $82.79"), ("Buy it", "last estimate: from $82.79"));
        assert_eq!(split_buy("Buy again"), ("Buy again", ""));
        assert_eq!(split_buy("Buy it · from $5 · more"), ("Buy it", "from $5 · more"));
    }
    #[test] fn the_strip_width_is_what_is_drawn_and_folding_keeps_stop_and_buy_it_whole() {
        let mut w = |s: &str| s.chars().count() as f32 * 7.;
        let ts = [item("fab-buy", "shopping-cart", "Buy it · from $5"), item("pin", "pin", "Pin"), item("settle", "circle-check", "Settle"), item("fork", "git-fork", "Fork thread")];
        let (full, icons) = strip_width(&ts, &mut w);
        // buy: 2 + 6 + 19 + 42 + 6 + 49 + 8 = 132; pin: 14 + 6 + 19 + 21 + 8 = 68; settle: 2 + 6 + 19 + 42 + 8 = 77; fork: 14 + 6 + 19 + 77 + 8 = 124
        assert_eq!(full, 132. + 68. + 77. + 124.);
        // folded: 6 + 13 + 6 = 25 each, buy unchanged
        assert_eq!(icons, 132. + (14. + 25.) + (2. + 25.) + (14. + 25.));
        let stop = [item("interrupt", "stop", "Stop")]; assert_eq!(strip_width(&stop, &mut w).0, strip_width(&stop, &mut w).1, "Stop never folds");
        assert!(!can_fold(&item("pin", "", "Pin")), "a tool with no icon keeps its word");
    }
    #[test] fn tight_drops_the_branching_then_keeping_tools_and_keeps_stop_last() {
        // Buy it, Stop, Pin, Settle, Fork as icons: 27 + 14 + 25 + 14 + 25 + 2 + 25 + 14 + 25 = 171
        let ts = [(1u8, 27.), (0, 25.), (2, 25.), (2, 25.), (3, 25.)];
        assert_eq!(keep_when_tight(&ts, 171.), 0b11111, "all fit");
        assert_eq!(keep_when_tight(&ts, 170.), 0b01111, "Fork goes first");
        assert_eq!(keep_when_tight(&ts, 120.), 0b00111, "then Settle (the keeping group from its end)");
        assert_eq!(keep_when_tight(&ts, 66.), 0b00011, "Buy it and Stop");
        assert_eq!(keep_when_tight(&ts, 40.), 0b00010, "Stop is the last to go");
        assert_eq!(keep_when_tight(&ts, 0.), 0b00010, "never none");
        assert_eq!(keep_when_tight(&[], 10.), 0);
    }
    #[test] fn the_tools_a_tight_header_drops_are_in_its_menu() {
        let t = serde_json::json!({"tools": [{"action":"fab-buy","icon":"shopping-cart","label":"Buy it · from $5"},{"action":"interrupt","icon":"square","label":"Stop"},{"action":"pin","icon":"pin","label":"Pin"},{"action":"fork","icon":"git-fork","label":"Fork thread"}]});
        assert_eq!(tools(&t).len(), 4);
        assert!(dropped_items(&t, 0).is_empty(), "nothing dropped: nothing added");
        let d = dropped_items(&t, !0b0011);
        assert_eq!(d.iter().map(|i| (i.label.as_str(), i.action.as_str())).collect::<Vec<_>>(), [("Pin", "pin"), ("Fork thread", "fork")]);
        assert_eq!(dropped_items(&t, 0b0001)[0].label, "Buy it", "Buy it by its word, without its estimate");
    }
    #[test] fn tight_only_when_even_the_icons_do_not_fit() {
        // state 140 + 24 + icons 200 + 6 = 370
        assert!(!tight(370., 140., 200.), "the icons fit: Stop and Buy it keep their words");
        assert!(tight(369., 140., 200.));
        assert!(!tight(0., 140., 200.), "an unmeasured row is never tight");
        // tight is past icons_only: the folded strip is never wider than the worded one
        let mut w = |s: &str| s.chars().count() as f32 * 7.;
        let ts = [item("interrupt", "stop", "Stop"), item("fab-buy", "shopping-cart", "Buy it · from $5"), item("pin", "pin", "Pin")];
        let (full, icons) = strip_width(&ts, &mut w);
        for r in (100..800).step_by(3) { let r = r as f32; assert!(!tight(r, 140., icons) || icons_only(r, 140., full), "tight but worded at {r}"); }
    }
}

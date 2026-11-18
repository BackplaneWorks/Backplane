//! A schematic's Sheets menu (the Schematic page's "Sheets (N)" and a Split's schematic pane): the design's sheets as the hub lists them
//! (`Event::Sheets`, the hierarchy in pre-order, indented by depth), the one on show marked. A design of more than `FIND_OVER` sheets (an
//! Altium project of fifty flat sheets, a deep KiCad hierarchy) gets a search field on top, focused on opening: typing keeps the sheets whose
//! name has every word (a name that starts with them first, then one whose words start with them), each under its parent path; Up and Down
//! move, Enter opens, Esc shuts (main.rs's ClosePopup asks `escape`). A press on a row shows that sheet (`view-sheet`, "" the top sheet).
use super::{composer::{Composer, ComposerEvent}, plot::Sheet};
use crate::{hub::Command, i18n::trs, theme, Backplane};
use gpui::{prelude::*, *};

/// The search field shows with more sheets than this (the phones' pickers search from the same count).
pub const FIND_OVER: usize = 10;

/// The menu's state, one per schematic pane (held on its camera, `PlotView::sheets`).
#[derive(Default)]
pub struct SheetsUi {
    pub find: Option<Entity<Composer>>,
    /// The highlighted row of those listed (None: the sheet on show, until a key moves it or the search changes).
    pub ix: Option<usize>,
    /// The search the rows were last listed for; the sheet on show as last drawn (an index into the sheets), for the keys.
    pub q: String, pub here: usize,
    pub scroll: ScrollHandle,
}

/// The row on show for a schematic source `key`: a sheet whose file the key ends with, else the top sheet (row 0).
pub fn current(sheets: &[Sheet], key: &str) -> usize {
    if key.ends_with('|') { return 0 }
    sheets.iter().enumerate().skip(1).find(|(_, s)| !s.file.is_empty() && key.ends_with(s.file.as_str())).map_or(0, |(i, _)| i)
}

/// Where the query's words fall in a name, case ignored: None when a word is not in it, else (every word starts a word of the name,
/// the name starts with the first word). A word of the name starts after a character that is no letter or digit, at a lower-to-upper
/// change (camelCase), and where letters and digits meet.
fn hits(name: &str, q: &str) -> Option<(bool, bool)> {
    let n: Vec<char> = name.chars().collect();
    let low: Vec<char> = n.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let starts = |p: usize| p == 0 || { let (a, b) = (n[p - 1], n[p]); !a.is_alphanumeric() || (a.is_lowercase() && b.is_uppercase()) || (a.is_alphabetic() != b.is_alphabetic()) };
    let (mut all_starts, mut lead) = (true, false);
    for (k, w) in q.split_whitespace().enumerate() {
        let w: Vec<char> = w.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect();
        let at: Vec<usize> = (0..=low.len().saturating_sub(w.len())).filter(|&p| low.len() >= w.len() && low[p..p + w.len()] == w[..]).collect();
        if at.is_empty() { return None }
        all_starts &= at.iter().any(|&p| starts(p));
        if k == 0 { lead = at.contains(&0) }
    }
    Some((all_starts, lead))
}
/// How well a name answers a search: 0 not at all, 1 every word is in it, 2 every word starts a word of it, 3 and it starts with the first.
pub fn rank(name: &str, q: &str) -> u8 {
    match hits(name, q) { None => 0, Some((true, true)) => 3, Some((true, false)) => 2, Some(_) => 1 }
}
/// The rows a search lists, as indices into `sheets`: all of them in the hub's order for an empty search, else the matching ones, the best
/// first and in the hub's order among equals.
pub fn sheet_matches(sheets: &[Sheet], q: &str) -> Vec<usize> {
    if q.trim().is_empty() { return (0..sheets.len()).collect() }
    let mut v: Vec<(usize, u8)> = sheets.iter().enumerate().map(|(i, s)| (i, rank(&s.name, q))).filter(|x| x.1 > 0).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.into_iter().map(|x| x.0).collect()
}
/// The sheets above row `i` (its parent's name, theirs...), outermost first, without the top sheet every row is under: "Power / Regulators".
pub fn path(sheets: &[Sheet], i: usize) -> String {
    let Some(mut d) = sheets.get(i).map(|s| s.depth) else { return String::new() };
    let mut up = vec![];
    for s in sheets[..i].iter().rev() {
        if s.depth < d { d = s.depth; if d == 0 { break } up.push(s.name.as_str()); }
    }
    up.reverse(); up.join(" / ")
}
/// The highlighted row after a move of `d` from `ix` (None: from the row on show) in `n` rows, wrapping round.
pub fn step(ix: Option<usize>, here_at: Option<usize>, d: i32, n: usize) -> usize {
    if n == 0 { return 0 }
    match ix.or(here_at) { Some(i) => (i as i32 + d).rem_euclid(n as i32) as usize, None => if d < 0 { n - 1 } else { 0 } }
}
/// The row highlighted as drawn: the one the keys chose, else the sheet on show when no search narrows the list, else the first.
pub fn shown_ix(ix: Option<usize>, rows: &[usize], here: usize) -> Option<usize> {
    if rows.is_empty() { return None }
    Some(ix.map_or_else(|| rows.iter().position(|&r| r == here).unwrap_or(0), |i| i.min(rows.len() - 1)))
}

fn ui(this: &mut Backplane, slot: usize) -> &mut SheetsUi { &mut this.screens.pv(slot).sheets }
fn query(ui: &SheetsUi, cx: &App) -> String { ui.find.as_ref().map(|f| f.read(cx).content.to_string()).unwrap_or_default() }
fn popover_id(slot: usize) -> &'static str { if slot == 0 { "sheets" } else { "split-sheets" } }

/// The button's press: open or shut the menu of pane `slot` (0 the Schematic page, 1 a Split's schematic).
pub fn toggle(this: &mut Backplane, slot: usize, window: &mut Window, cx: &mut Context<Backplane>) {
    let pv = this.screens.pv(slot);
    // the press that ends in this click shut the menu already (a press outside it, on this button): it stays shut
    if !pv.sheets_open && super::popover::just_shut(popover_id(slot)) { cx.notify(); return }
    let on = !this.screens.pv(slot).sheets_open;
    set_open(this, slot, on, Some(window), cx);
}
/// Open (an empty search, the sheet on show highlighted, the field focused when there is a window to focus it in) or shut the menu.
pub fn set_open(this: &mut Backplane, slot: usize, on: bool, window: Option<&mut Window>, cx: &mut Context<Backplane>) {
    this.screens.pv(slot).sheets_open = on;
    if !on { cx.notify(); return }
    if ui(this, slot).find.is_none() {
        let f = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        // (popup_open: Up and Down move the highlighted row instead of the caret)
        f.update(cx, |i, _| { i.set_placeholder(&trs("Find a sheet")); i.popup_open = true; });
        // (only a change of the search draws the menu again: the field's caret is its own)
        cx.observe(&f, move |this, f, cx| { let q = f.read(cx).content.to_string(); let u = ui(this, slot); if q != u.q { u.q = q; u.ix = None; u.scroll.set_offset(point(px(0.), px(0.))); cx.notify(); } }).detach();
        cx.subscribe(&f, move |this, _, ev: &ComposerEvent, cx| match ev {
            ComposerEvent::Submit | ComposerEvent::PopupPick => {
                let q = ui(this, slot).q.clone(); let rows = sheet_matches(&this.screens.plot.sheets, &q);
                let u = ui(this, slot);
                if let Some(i) = shown_ix(u.ix, &rows, u.here) { let s = rows[i]; pick(this, slot, s, cx); }
            }
            ComposerEvent::PopupMove(d) => {
                let q = ui(this, slot).q.clone(); let rows = sheet_matches(&this.screens.plot.sheets, &q);
                let u = ui(this, slot);
                let at = shown_ix(u.ix, &rows, u.here);
                u.ix = Some(step(at, None, *d, rows.len()));
                if let Some(i) = u.ix { u.scroll.scroll_to_item(i); }
                cx.notify();
            }
            _ => {}
        }).detach();
        ui(this, slot).find = Some(f);
    }
    let u = ui(this, slot); u.ix = None; u.q.clear(); u.scroll.set_offset(point(px(0.), px(0.)));
    if let Some(f) = u.find.clone() {
        f.update(cx, |i, cx| i.set(String::new(), cx));
        if let Some(w) = window { if this.screens.plot.sheets.len() > FIND_OVER { w.focus(&f.read(cx).focus_handle(cx)); } }
    }
    cx.notify();
}
/// Show sheet `i` in pane `slot` and shut the menu.
fn pick(this: &mut Backplane, slot: usize, i: usize, cx: &mut Context<Backplane>) {
    let file = if i == 0 { String::new() } else { this.screens.plot.sheets.get(i).map(|s| s.file.clone()).unwrap_or_default() };
    let _ = this.tx.try_send(Command::Action("view-sheet".into(), file));
    this.screens.pv(slot).sheets_open = false;
    cx.notify();
}
/// Escape while a Sheets menu is open shuts it. True when it took the key.
pub fn escape(this: &mut Backplane, cx: &mut Context<Backplane>) -> bool {
    let mut took = false;
    for slot in 0..2 { let pv = this.screens.pv(slot); if pv.sheets_open { pv.sheets_open = false; took = true; } }
    if took { cx.notify(); }
    took
}

/// The open menu of pane `slot` under its button: the search field (more than `FIND_OVER` sheets), then the rows, scrolling past its height.
/// Without a search the hierarchy is indented by depth; a search lists the matches flat, each after its parent path (dimmed), the words marked.
pub fn menu(cx: &mut Context<Backplane>, light: bool, slot: usize, sheets: &[Sheet], ui: &mut SheetsUi, here: usize) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    ui.here = here;
    let finding = sheets.len() > FIND_OVER;
    let q = if finding { query(ui, cx) } else { String::new() };
    let rows = sheet_matches(sheets, &q);
    let hi = shown_ix(ui.ix, &rows, here);
    let flat = !q.trim().is_empty();
    let id = popover_id(slot);
    // (its rows are its children in order, so the scroll handle's item n is row n)
    let mut list = div().id(SharedString::from(format!("{id}-rows"))).flex().flex_col().flex_1().min_h_0().overflow_y_scroll().track_scroll(&ui.scroll).p_1();
    for (n, &i) in rows.iter().enumerate() {
        let sh = &sheets[i];
        let on = i == here;
        let up = if flat { path(sheets, i) } else { String::new() };
        let name = match (flat, super::palette::mark(&sh.name, &q)) {
            (true, Some(r)) => StyledText::new(sh.name.clone()).with_highlights([(r, HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })]).into_any_element(),
            _ => sh.name.clone().into_any_element(),
        };
        list = list.child(div().id(SharedString::from(format!("{}-{i}", if slot == 0 { "sheet" } else { "split-sheet" }))).flex().items_center().min_w_0().pr_2().py(px(3.)).cursor_pointer()
            .pl(px(6. + if flat { 0. } else { 12. * sh.depth.min(8) as f32 }))
            .border_l_2().border_color(if on { c("accent") } else { gpui::transparent_black().into() })
            .when(hi == Some(n), |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
            .text_color(c(if on { "accent" } else { "text" }))
            .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().flex().items_center()
                .when(!up.is_empty(), |d| d.child(div().flex_shrink().min_w_0().overflow_hidden().text_ellipsis().text_color(c("dim")).child(format!("{up} / "))))
                .child(div().flex_shrink_0().child(name)))
            .on_click(cx.listener(move |this, _, _, cx| pick(this, slot, i, cx))));
    }
    if rows.is_empty() { list = list.child(div().px_2().py_1().text_color(c("dim")).child(trs("Nothing matches"))); }
    let field = ui.find.clone().filter(|_| finding).map(|f| div().id(SharedString::from(format!("{id}-find"))).m_1().px_2().py_1().bg(c("bg")).border_1().border_color(c("line")).child(f));
    div().id(SharedString::from(format!("{id}-list"))).mt(px(2.)).min_w(px(240.)).max_w(px(420.)).max_h(crate::scale::tpx(420.)).flex().flex_col()
        .text_size(crate::scale::tpx(12.)).bg(c("panel")).border_1().border_color(c("line")).shadow_lg().occlude()
        .on_mouse_down_out(cx.listener(move |this, _, _, cx| { this.screens.pv(slot).sheets_open = false; super::popover::shut(popover_id(slot)); cx.notify(); }))
        .children(field).child(list).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn rows(t: &str) -> Vec<Sheet> { super::super::plot::sheet_rows(t) }
    fn names(s: &[Sheet], ix: &[usize]) -> Vec<String> { ix.iter().map(|&i| s[i].name.clone()).collect() }
    #[test] fn a_search_keeps_names_with_every_word_word_starts_first() {
        assert_eq!((rank("Power", "pow"), rank("USB_Power", "pow"), rank("Superpower", "pow"), rank("Audio", "pow")), (3, 2, 1, 0));
        assert_eq!(rank("Power_Regulators", "reg pow"), 2, "every word at a word start, in any order");
        assert_eq!(rank("PowerRegulators", "pow reg"), 3, "camelCase starts a word");
        assert_eq!(rank("Sheet12", "12"), 2, "digits after letters start a word");
        assert_eq!(rank("USB hub", "USB HUB"), 3, "case is ignored");
        assert_eq!(rank("Power", "pow zz"), 0, "every word must be in it");
        assert_eq!(rank("Ünterseite", "ün"), 3);
    }
    #[test] fn fifty_flat_sheets_are_found_by_a_word_best_first_in_the_hubs_order() {
        let mut t = String::from("0\t0\tTop.SchDoc\t/p/Top.SchDoc");
        for i in 1..50 { t.push_str(&format!("\n1\t0\t{}\t/p/s{i}.SchDoc", match i { 7 => "MCU_Power".to_string(), 20 => "Power".into(), 33 => "Superpower".into(), _ => format!("Sheet{i}") })); }
        let s = rows(&t);
        assert_eq!(sheet_matches(&s, "  ").len(), 50, "an empty search lists every sheet");
        assert_eq!(names(&s, &sheet_matches(&s, "power")), ["Power", "MCU_Power", "Superpower"]);
        assert_eq!(names(&s, &sheet_matches(&s, "sheet4")), ["Sheet4", "Sheet40", "Sheet41", "Sheet42", "Sheet43", "Sheet44", "Sheet45", "Sheet46", "Sheet47", "Sheet48", "Sheet49"]);
        assert!(sheet_matches(&s, "nothing").is_empty());
    }
    #[test] fn a_match_is_listed_under_its_parent_path_without_the_top_sheet() {
        let s = rows("0\t0\troot.kicad_sch\t/p/root.kicad_sch\n1\t0\tPower\t/p/power.kicad_sch\n2\t0\tRegulators\t/p/reg.kicad_sch\n3\t0\tLDO\t/p/ldo.kicad_sch\n2\t0\tBattery\t/p/bat.kicad_sch\n1\t0\tMCU\t/p/mcu.kicad_sch");
        assert_eq!((path(&s, 0), path(&s, 1), path(&s, 2), path(&s, 3)), (String::new(), String::new(), "Power".to_string(), "Power / Regulators".to_string()));
        assert_eq!((path(&s, 4), path(&s, 5)), ("Power".to_string(), String::new()), "a sibling after a deeper row has its own parent");
        assert_eq!(names(&s, &sheet_matches(&s, "ldo")), ["LDO"]);
    }
    #[test] fn the_sheet_on_show_is_found_from_the_source_key() {
        let s = rows("0\t0\troot\t/p/top.kicad_sch\n1\t0\tPower\t/p/power.kicad_sch\n1\t0\tAmp\t/p/amp.kicad_sch");
        assert_eq!(current(&s, "1|/p|"), 0);
        assert_eq!(current(&s, "1|/p|/p/amp.kicad_sch"), 2);
        assert_eq!(current(&s, "1|/p|/p/top.kicad_sch"), 0);
        assert_eq!(current(&s, "1|/p|/elsewhere.kicad_sch"), 0);
    }
    #[test] fn the_keys_move_from_the_sheet_on_show_and_wrap_round() {
        assert_eq!(shown_ix(None, &[0, 1, 2, 3], 2), Some(2), "opened: the sheet on show is highlighted");
        assert_eq!(shown_ix(None, &[3, 1], 2), Some(0), "a search without it: the first");
        assert_eq!(shown_ix(Some(9), &[0, 1], 0), Some(1), "a list that shrank keeps the highlight on it");
        assert_eq!(shown_ix(None, &[], 0), None);
        assert_eq!((step(Some(2), None, 1, 4), step(Some(3), None, 1, 4), step(Some(0), None, -1, 4)), (3, 0, 3));
        assert_eq!((step(None, None, 1, 4), step(None, None, -1, 4), step(None, Some(1), 1, 4), step(Some(0), None, 1, 0)), (0, 3, 2, 0));
    }
}

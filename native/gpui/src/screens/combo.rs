//! The composer's one chip: the provider's mark, the model, the effort, the account and what its binding limit has left, in the hub's words (`thread.combo`,
//! host present/keys.rs `combo`), at the width there is. A click, the model key or the account key opens one menu with three parts: Model (the picker: agent,
//! model with search and stars, effort; the menu's whole width), Account (each login with what it has left, digits pick, Move all threads here) and Usage (the thread's account's windows with their resets and
//! its extra usage). The model key puts the keyboard in the search, the account key on the accounts (their digits); the other key moves it, the same key shuts it.
use super::accounts;
use crate::{string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;
use std::{cell::Cell, rc::Rc};

/// The width of one character of the chip's words (12 px mono) and what the chip spends besides its words (padding, border, the mark and its gap).
const CH: f32 = 7.3;
const FRAME: f32 = 52.;
/// What the composer's row spends on its other controls (the context chip, Attach, Dictate, Send and the gaps between).
pub const OTHERS: f32 = 300.;
/// The context chip's frame (padding, border, the gauge and its gap) and the part of OTHERS that is not the context chip ("ctx NN%" was counted in it).
const TOK_FRAME: f32 = 46.;
const FIXED: f32 = OTHERS - (TOK_FRAME + 7. * CH);
/// The row's split between the one chip and the context chip: the context chip says tokens, cost and context (`tok`) while the chip's short form
/// still fits beside it, else the context alone (`ctx`), so the chip never shows a word cut in half. Returns (the chip's px, the context chip's words).
pub fn split(bar_w: f32, combo: &Value, tok: &str, ctx: &str) -> (f32, String) {
    let words = |t: &str| TOK_FRAME + t.chars().count() as f32 * CH;
    if bar_w <= 0. { return (0., if tok.is_empty() { ctx.to_string() } else { tok.to_string() }) }
    let short = string(combo, "short");
    let need = FRAME + short.chars().count() as f32 * CH;
    let text = if !tok.is_empty() && (ctx.is_empty() || bar_w - FIXED - words(tok) >= need) { tok } else { ctx };
    (bar_w - FIXED - words(text), text.to_string())
}

/// Chip.words: the longest of the hub's three forms (full, mid, short) that fits `avail` px; the full one while the width is not known yet (0).
pub fn words(combo: &Value, avail: f32) -> String {
    let forms: Vec<String> = ["full", "mid", "short"].iter().map(|k| string(combo, k)).filter(|s| !s.is_empty()).collect();
    let Some(last) = forms.last().cloned() else { return String::new() };
    if avail <= 0. { return forms[0].clone() }
    forms.into_iter().find(|f| FRAME + f.chars().count() as f32 * CH <= avail).unwrap_or(last)
}
/// The colour token of the chip's words: the hub's tone ("warn", "bad"), else plain.
pub fn tone(combo: &Value) -> &'static str { match combo["tone"].as_str() { Some("bad") => "bad", Some("warn") => "warn", _ => "text" } }
/// The hover: the account's email and each window with its reset, as the hub words them, then the keys.
pub fn tip(combo: &Value, model_key: &str, account_key: &str) -> String {
    let mut t = string(combo, "tip").lines().map(crate::i18n::trs).collect::<Vec<_>>().join("\n");
    let keys = [(model_key, "gpui.combo.key.model"), (account_key, "gpui.combo.key.account")].iter().filter(|(k, _)| !k.is_empty()).map(|(k, w)| crate::i18n::tf(w, &[("key", (*k).into())])).collect::<Vec<_>>().join(" · ");
    if !keys.is_empty() { if !t.is_empty() { t.push('\n') } t.push_str(&keys) }
    t
}
/// The Usage part: one row a window ("Weekly", "36% left", "resets Wed 11 Nov 17:00", its tone, whether it binds this thread's model), then the extra usage and why there is nothing.
#[derive(Debug, PartialEq)]
pub struct UsageRow { pub label: String, pub left: String, pub resets: String, pub tone: &'static str, pub applies: bool }
pub fn usage_rows(combo: &Value) -> (Vec<UsageRow>, Vec<String>) {
    let rows = combo["windows"].as_array().into_iter().flatten().map(|w| UsageRow { label: crate::i18n::trs(&string(w, "label")), left: w["left"].as_u64().map(|l| crate::i18n::tf("gpui.combo.left", &[("pct", l.into())])).unwrap_or_default(),
        resets: Some(string(w, "resets")).filter(|r| !r.is_empty()).map(|r| crate::i18n::tf("gpui.combo.resets", &[("when", r.into())])).unwrap_or_default(), tone: tone(w), applies: w["applies"].as_bool().unwrap_or(true) }).collect();
    let notes = ["extra", "note"].iter().map(|k| crate::i18n::trs(&string(combo, k))).filter(|s| !s.is_empty()).collect();
    (rows, notes)
}

/// The chip. `avail`: the px the row leaves it (0: not measured yet). `menu_open`: its menu is up, so its hover stays away (it would sit over the menu's Account row).
pub fn chip(cx: &mut Context<Backplane>, light: bool, thread: &Value, avail: f32, keys: (&str, &str), this_chip: &Rc<Cell<Bounds<Pixels>>>, menu_open: bool) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let combo = &thread["combo"];
    let provider = super::picker::bare(thread["provider"].as_str().or(thread["picker"]["providers"].as_array().and_then(|p| p.iter().find(|x| x["on"] == true)).and_then(|x| x["value"].as_str())).unwrap_or("claude")).to_string();
    let text = words(combo, avail);
    let text = if text.is_empty() { "…".to_string() } else { text };
    let tipped = tip(combo, keys.0, keys.1);
    // where it was drawn: a press on it is the chip's own (it shuts the menu), not a press outside the menu
    let at = this_chip.clone();
    div().id("model-chip").relative().cursor_pointer().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().px_3().py_1().border_1().border_color(c("line")).bg(c("panel")).hover(|s| s.bg(c("raise")))
        .text_size(px(12.)).font(crate::art::mono_font()).flex().items_center().gap_2()
        .child(canvas(move |b, _, _| at.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()).child(crate::art::probe("modelchip"))
        .child(super::picker::logo(&provider)).child(div().min_w_0().overflow_hidden().text_ellipsis().text_color(c(tone(combo))).child(text))
        .when(!tipped.is_empty() && !menu_open, |d| d.tooltip(move |_, cx| cx.new(|_| Tip(tipped.clone(), light)).into()))
        .on_click(cx.listener(|this, _, window, cx| this.combo_click(window, cx)))
}
/// A hover of several lines.
pub(crate) struct Tip(pub String, pub bool);
impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let c = |n: &str| theme::color(self.1, n);
        div().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_color(c("text")).text_size(px(12.)).flex().flex_col().children(self.0.lines().map(|l| div().whitespace_nowrap().child(l.to_string())).collect::<Vec<_>>())
    }
}

/// The gap between the chip's row and the menu, and the least the menu keeps from the window's edges.
pub const GAP: f32 = 6.;
pub const MARGIN: f32 = 8.;
/// Where the menu floats: over the chip's row (as 0.14's popover) when the menu fits there or there is more room there than under it, else under it;
/// and the height it may take on that side. `row` is the row's top and bottom as last drawn (equal before it was drawn), `want` the menu's usual height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot { pub above: bool, pub room: f32 }
pub fn spot(row: (f32, f32), win_h: f32, want: f32) -> Spot {
    if row.1 <= row.0 { return Spot { above: true, room: (win_h * 0.6).max(160.) } }
    let (up, down) = (row.0 - GAP - MARGIN, win_h - row.1 - GAP - MARGIN);
    let above = up >= want || up >= down;
    Spot { above, room: if above { up } else { down }.max(160.) }
}
/// The height the open menu holds: [width, room, height]. A list that shrinks (a search that narrows it, another agent with fewer models, the usage
/// read again) leaves the menu as tall as it was, so its edge away from the chip stays put; it only grows, and starts again at another width or room.
pub fn hold(prev: [f32; 3], w: f32, room: f32, natural: f32) -> [f32; 3] {
    if prev[0] != w || prev[1] != room { [w, room, natural] } else { [w, room, prev[2].max(natural)] }
}
/// The least height the menu is drawn at this frame: what it held at this width and room, never more than the room.
pub fn held_h(held: [f32; 3], w: f32, room: f32) -> f32 { if held[0] == w && held[1] == room { held[2].min(room) } else { 0. } }

/// The menu: the picker over Account and Usage. It floats over the thread (a deferred overlay anchored to the chip's row), so opening it, moving in
/// it or shutting it never moves or resizes the composer or anything else in the window. `None` while shut: the caller puts it in the chip's row.
pub fn menu(cx: &mut Context<Backplane>, light: bool, thread: &Value, this: &Backplane, bar_w: f32, win: (f32, f32)) -> Option<AnyElement> {
    if !this.screens.picker_open { crate::art::forget("chipmenu"); return None }
    let c = |n: &str| theme::color(light, n);
    // the composer can be wider than what the window shows of it (the thread's own least width): the menu keeps to the window, less the sidebar, the rail and the margins
    let shown_w = win.0 - if this.sidebar_open { this.local.sidebar_px as f32 } else { 0. } - 100.;
    let s = (this.local.text_pct as f32 / 100.).clamp(0.6, 2.);
    let row = this.screens.pick.row.get();
    let at = spot((f32::from(row.top()), f32::from(row.bottom())), win.1, 420. * s);
    let f = super::picker::fit(super::picker::menu_width(bar_w, shown_w), at.room, this.local.text_pct);
    let narrow = !f.side_row;
    let z = |v: f32| px(v * f.s);
    let model = super::picker::view(cx, light, thread, true, &this.screens.pick, &this.local.favs, &f);
    let st = &this.screens.acct;
    let accounts_on = matches!(thread["provider"].as_str().map(super::picker::bare), Some("claude" | "codex")) || thread["combo"]["account"].as_str().map_or(false, |a| !a.is_empty());
    let head = |t: &str, hint: String, on: bool| div().px_3().pt_2().pb_1().flex().justify_between().text_size(z(11.)).text_color(c(if on { "accent" } else { "faint" })).child(crate::i18n::trs(t).to_uppercase()).child(hint);
    let mut side = div().flex().flex_col().min_w_0();
    if accounts_on {
        /* i18n-ok: drawn through trs in head */ let acct = div().flex_1().min_w_0().flex().flex_col().child(head("Account", if st.menu { "1–9".into() } else { this.key_hint("account") }, st.menu)).child(accounts::section(cx, light, st, f.s));
        let (rows, notes) = usage_rows(&thread["combo"]);
        let mut u = div().flex().flex_col().px_3().pb_2().gap(px(2.)).text_size(z(12.));
        for r in rows {
            u = u.child(div().flex().items_center().gap_2().when(!r.applies, |d| d.opacity(0.6))
                .child(div().w(z(96.)).flex_shrink_0().overflow_hidden().whitespace_nowrap().text_color(c("dim")).child(r.label)).child(div().w(z(66.)).flex_shrink_0().text_color(c(r.tone)).child(r.left))
                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(z(11.)).text_color(c("faint")).child(r.resets)));
        }
        for n in notes { u = u.child(div().text_size(z(11.)).text_color(c("dim")).child(n)) }
        /* i18n-ok: drawn through trs in head */ let usage = div().flex_1().min_w_0().flex().flex_col().child(head("Usage", String::new(), false)).child(u);
        // under the picker: Account beside Usage when the composer is wide, one over the other when it is narrow
        side = if narrow { side.child(acct).child(div().mx_2().my_1().h(px(1.)).bg(c("line"))).child(usage) } else { side.flex_row().items_start().child(acct).child(usage.border_l_1().border_color(c("line"))) };
    }
    // the picker (screens/picker.rs: agent, model and effort, each column with its own head and keys) takes the menu's whole width, Account and Usage under it
    // Account and Usage keep to their own room: they scroll inside it rather than push the picker's lists out of the window
    let side = div().id("combo-side").max_h(px((f.menu_h * 0.35).max(120.))).overflow_y_scroll().child(side);
    let model_col = div().w_full().min_w_0().flex().flex_col().children(model);
    let body = div().flex().flex_col().child(model_col).when(accounts_on, |d| d.child(div().h(px(1.)).bg(c("line"))).child(side));
    // its height while open: never less than it has been (hold), so neither edge moves as its lists change
    let held = this.screens.pick.held.clone();
    let floor = held_h(held.get(), f.w, f.menu_h);
    let (w, room) = (f.w, f.menu_h);
    // (the canvas fills the inside of the menu's 1 px border: the menu's own height is 2 px more)
    let measure = canvas(move |b, _, _| held.set(hold(held.get(), w, room, f32::from(b.size.height) + 2.)), |_, _, _, _| {}).absolute().top_0().left_0().size_full();
    let chip = this.screens.pick.chip.clone();
    let card = div().id("combo-menu").occlude().relative().w(px(f.w)).min_h(px(floor)).max_h(px(f.menu_h)).overflow_y_scroll().border_1().border_color(c("line")).bg(c("panel")).shadow_md()
        .child(crate::art::probe("chipmenu")).child(measure).child(body)
        // a press anywhere else shuts it (the chip's own press toggles it)
        .on_mouse_down_out(cx.listener(move |this, e: &MouseDownEvent, window, cx| { if !chip.get().contains(&e.position) { this.picker_dismiss(window, cx) } }));
    // anchored to the chip's row: its bottom edge just over the row (or its top just under it), its left edge on the row's, kept inside the window
    let pin = div().absolute().left_0().size_0().map(|d| if at.above { d.top(px(-GAP)) } else { d.bottom(px(-GAP)) })
        .child(deferred(anchored().anchor(if at.above { Corner::BottomLeft } else { Corner::TopLeft }).snap_to_window_with_margin(px(MARGIN)).child(card)).with_priority(1));
    Some(pin.into_any_element())
}

impl Backplane {
    pub(crate) fn key_hint(&self, action: &str) -> String { crate::keys::hint(&self.local, action) }
    /// The chip pressed: open on the model (search) part, or shut.
    pub(crate) fn combo_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screens.picker_open { self.picker_close(cx) } else { self.combo_model(window, cx) }
    }
    /// The model key: the menu with the keyboard in its search; pressed with the keyboard already there, it shuts.
    pub(crate) fn combo_model(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screens.picker_open && !self.screens.acct.menu { self.picker_close(cx); return }
        self.screens.acct.menu = false;
        self.picker_show(window, cx);
        accounts::list(self); accounts::refresh_usage(self);
    }
    /// Something drawn over the page (the palette, a picture, Settings, the tour, a 0.14 home coming across): the menu stays open but is not drawn over it.
    pub(crate) fn chip_menu_covered(&self) -> bool { self.screens.palette.open || self.screens.lightbox.is_some() || super::settings::is_open(self) || self.data["tour"].is_object() || super::migrate::is_up(&self.data) }
    /// A press outside the menu: it shuts. The keyboard goes back to the composer if it was in the menu; what was pressed may then take it (a field, the sidebar's search).
    pub(crate) fn picker_dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let inside = self.screens.pick.search.as_ref().map_or(false, |s| s.focus_handle(cx).contains_focused(window, cx)) || self.screens.acct.focus.as_ref().map_or(false, |f| f.contains_focused(window, cx));
        self.screens.picker_open = false; self.screens.acct.menu = false; self.screens.pick.cur = None;
        if inside { self.input.focus_handle(cx).focus(window); }
        cx.notify();
    }
    /// The account key: the menu with the keyboard on the accounts (their digits pick); pressed with the keyboard already there, it shuts.
    pub(crate) fn combo_account(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screens.picker_open && self.screens.acct.menu { self.picker_close(cx); return }
        if !self.screens.picker_open { self.picker_show(window, cx); }
        self.screens.acct.menu = true; self.screens.acct.notice.clear();
        accounts::list(self); accounts::refresh_usage(self);
        let h = self.screens.acct.focus.get_or_insert_with(|| cx.focus_handle()).clone(); window.focus(&h);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use serde_json::json;
    fn combo() -> Value { json!({"full":"Opus 5.5 · auto · kellyhlord · 36% left","mid":"Opus 5.5 · kellyhlord · 36%","short":"Opus 5.5 · 36%","tone":"","tip":"kellyhlord@gmail.com (saved account)\nWeekly  36% left · resets Wed 11 Nov 17:00",
        "windows":[{"label":"Weekly","left":36,"tone":"","applies":true,"resets":"Wed 11 Nov 17:00"},{"label":"Fable weekly","left":100,"tone":"","applies":false,"resets":""}],"extra":"Extra usage $0.00 of $50.00 · off (out of credits)","note":""}) }
    #[test] fn the_token_words_give_way_before_the_chip_cuts_a_word() {
        let tok = "885k tok · $0.0915 · ctx 44%";
        // a wide row: both say everything
        let (avail, text) = split(1232., &combo(), tok, "ctx 44%"); assert_eq!(text, tok); assert_eq!(words(&combo(), avail), string(&combo(), "full"));
        // the row beside an open viewer (532 px): the context alone, and the chip keeps a whole form
        let (avail, text) = split(532., &combo(), tok, "ctx 44%"); assert_eq!(text, "ctx 44%"); assert!(FRAME + string(&combo(), "short").chars().count() as f32 * CH <= avail);
        // nothing measured yet, or no context words: the hub's token words as they are
        assert_eq!(split(0., &combo(), tok, "ctx 44%"), (0., tok.to_string())); assert_eq!(split(532., &combo(), tok, "").1, tok);
    }
    #[test] fn the_chip_says_as_much_as_fits() {
        let c = combo();
        assert_eq!(words(&c, 0.), "Opus 5.5 · auto · kellyhlord · 36% left", "not measured yet: all of it");
        assert_eq!(words(&c, 600.), "Opus 5.5 · auto · kellyhlord · 36% left");
        assert_eq!(words(&c, 280.), "Opus 5.5 · kellyhlord · 36%");
        assert_eq!(words(&c, 170.), "Opus 5.5 · 36%");
        assert_eq!(words(&c, 40.), "Opus 5.5 · 36%", "the shortest when nothing fits");
        assert_eq!(words(&json!({"full":"grok-4 · low","mid":"grok-4","short":"grok-4"}), 120.), "grok-4");
        assert_eq!(words(&Value::Null, 300.), "");
    }
    #[test] fn the_menu_floats_over_the_row_when_it_fits_there_else_under_it() {
        // the composer at the window's foot (a 900 px window, the row at 856..888): over it, with the room up to the window's top margin
        assert_eq!(spot((856., 888.), 900., 420.), Spot { above: true, room: 856. - GAP - MARGIN });
        // a short window: still over the row, the room what there is (the lists shrink to it, screens/picker.rs fit)
        assert_eq!(spot((606., 638.), 650., 420.), Spot { above: true, room: 606. - GAP - MARGIN });
        // the row near the top: under it
        assert_eq!(spot((60., 92.), 900., 420.), Spot { above: false, room: 900. - 92. - GAP - MARGIN });
        // neither side fits the usual height: the side with more room
        let s = spot((200., 232.), 500., 420.); assert!(!s.above && s.room == 500. - 232. - GAP - MARGIN, "{s:?}");
        assert!(spot((300., 332.), 500., 420.).above);
        // not drawn yet: over it, a usable room; never less than the least the menu needs
        assert!(spot((0., 0.), 900., 420.).above && spot((0., 0.), 900., 420.).room >= 160.);
        assert_eq!(spot((20., 52.), 60., 420.).room, 160.);
    }
    #[test] fn the_open_menu_holds_its_height_as_its_lists_change() {
        let h = hold([0.; 3], 898., 842., 470.); assert_eq!(h, [898., 842., 470.]);
        let h = hold(h, 898., 842., 300.); assert_eq!(h[2], 470., "a search with fewer rows, another agent with fewer models: as tall as it was");
        let h = hold(h, 898., 842., 520.); assert_eq!(h[2], 520., "more (the usage read in): it grows");
        assert_eq!(held_h(h, 898., 842.), 520.);
        assert_eq!(held_h(h, 898., 400.), 0., "another room (the window resized): drawn afresh");
        assert_eq!(hold(h, 560., 842., 380.), [560., 842., 380.], "another width: held afresh");
        assert_eq!(held_h([898., 842., 900.], 898., 842.), 842., "never more than the room");
    }
    #[test] fn low_and_out_read_in_their_colours_and_the_hover_says_everything() {
        assert_eq!((tone(&combo()), tone(&json!({"tone":"warn"})), tone(&json!({"tone":"bad"}))), ("text", "warn", "bad"));
        assert_eq!(tip(&combo(), "Ctrl+M", "Ctrl+Shift+A"), "kellyhlord@gmail.com (saved account)\nWeekly  36% left · resets Wed 11 Nov 17:00\nCtrl+M model · Ctrl+Shift+A account");
        let (rows, notes) = usage_rows(&combo());
        assert_eq!(rows[0], UsageRow { label: "Weekly".into(), left: "36% left".into(), resets: "resets Wed 11 Nov 17:00".into(), tone: "text", applies: true });
        assert!(!rows[1].applies && rows[1].resets.is_empty()); assert_eq!(notes, ["Extra usage $0.00 of $50.00 · off (out of credits)"]);
    }
}

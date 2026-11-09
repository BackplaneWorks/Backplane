//! The composer's one chip: the provider's mark, the model, the effort, the account and what its binding limit has left, in the hub's words (`thread.combo`,
//! host present/keys.rs `combo`), at the width there is. A click, the model key or the account key opens one menu with three parts: Model (the picker: agent,
//! model with search and stars, effort; the menu's whole width), Account (each login with what it has left, digits pick, Move all threads here) and Usage (the thread's account's windows with their resets and
//! its extra usage). The model key puts the keyboard in the search, the account key on the accounts (their digits); the other key moves it, the same key shuts it.
use super::accounts;
use crate::{string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

/// The width of one character of the chip's words (12 px mono) and what the chip spends besides its words (padding, border, the mark and its gap).
const CH: f32 = 7.3;
const FRAME: f32 = 52.;
/// What the composer's row spends on its other controls (Draw, the context chip, Attach, Dictate, Send and the gaps between).
pub const OTHERS: f32 = 350.;

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
    let mut t = string(combo, "tip");
    let keys = [(model_key, "model"), (account_key, "account")].iter().filter(|(k, _)| !k.is_empty()).map(|(k, w)| format!("{k} {w}")).collect::<Vec<_>>().join(" · ");
    if !keys.is_empty() { if !t.is_empty() { t.push('\n') } t.push_str(&keys) }
    t
}
/// The Usage part: one row a window ("Weekly", "36% left", "resets Wed 11 Nov 17:00", its tone, whether it binds this thread's model), then the extra usage and why there is nothing.
#[derive(Debug, PartialEq)]
pub struct UsageRow { pub label: String, pub left: String, pub resets: String, pub tone: &'static str, pub applies: bool }
pub fn usage_rows(combo: &Value) -> (Vec<UsageRow>, Vec<String>) {
    let rows = combo["windows"].as_array().into_iter().flatten().map(|w| UsageRow { label: string(w, "label"), left: w["left"].as_u64().map(|l| format!("{l}% left")).unwrap_or_default(),
        resets: Some(string(w, "resets")).filter(|r| !r.is_empty()).map(|r| format!("resets {r}")).unwrap_or_default(), tone: tone(w), applies: w["applies"].as_bool().unwrap_or(true) }).collect();
    let notes = ["extra", "note"].iter().map(|k| string(combo, k)).filter(|s| !s.is_empty()).collect();
    (rows, notes)
}

/// The chip. `avail`: the px the row leaves it (0: not measured yet).
pub fn chip(cx: &mut Context<Backplane>, light: bool, thread: &Value, avail: f32, keys: (&str, &str)) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let combo = &thread["combo"];
    let provider = super::picker::bare(thread["provider"].as_str().or(thread["picker"]["providers"].as_array().and_then(|p| p.iter().find(|x| x["on"] == true)).and_then(|x| x["value"].as_str())).unwrap_or("claude")).to_string();
    let text = words(combo, avail);
    let text = if text.is_empty() { "…".to_string() } else { text };
    let tipped = tip(combo, keys.0, keys.1);
    div().id("model-chip").cursor_pointer().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().px_3().py_1().border_1().border_color(c("line")).bg(c("panel")).hover(|s| s.bg(c("raise")))
        .text_size(px(12.)).font(crate::art::mono_font()).flex().items_center().gap_2()
        .child(super::picker::logo(&provider)).child(div().min_w_0().overflow_hidden().text_color(c(tone(combo))).child(text))
        .when(!tipped.is_empty(), |d| d.tooltip(move |_, cx| cx.new(|_| Tip(tipped.clone(), light)).into()))
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

/// The menu: Model beside Account and Usage, or under them when the composer is narrow (`narrow`).
pub fn menu(cx: &mut Context<Backplane>, light: bool, thread: &Value, this: &Backplane, bar_w: f32, win: (f32, f32)) -> Option<AnyElement> {
    if !this.screens.picker_open { return None }
    let c = |n: &str| theme::color(light, n);
    // the composer can be wider than what the window shows of it (the thread's own least width): the menu keeps to the window, less the sidebar, the rail and the margins
    let shown_w = win.0 - if this.sidebar_open { this.local.sidebar_px as f32 } else { 0. } - 100.;
    let f = super::picker::fit(super::picker::menu_width(bar_w, shown_w), win.1, this.local.text_pct);
    let narrow = !f.side_row;
    let z = |v: f32| px(v * f.s);
    let model = super::picker::view(cx, light, thread, true, &this.screens.pick, &this.local.favs, &f);
    let st = &this.screens.acct;
    let accounts_on = matches!(thread["provider"].as_str().map(super::picker::bare), Some("claude" | "codex")) || thread["combo"]["account"].as_str().map_or(false, |a| !a.is_empty());
    let head = |t: &str, hint: String, on: bool| div().px_3().pt_2().pb_1().flex().justify_between().text_size(z(11.)).text_color(c(if on { "accent" } else { "faint" })).child(t.to_uppercase()).child(hint);
    let mut side = div().flex().flex_col().min_w_0();
    if accounts_on {
        let acct = div().flex_1().min_w_0().flex().flex_col().child(head("Account", if st.menu { "1–9".into() } else { this.key_hint("account") }, st.menu)).child(accounts::section(cx, light, st, f.s));
        let (rows, notes) = usage_rows(&thread["combo"]);
        let mut u = div().flex().flex_col().px_3().pb_2().gap(px(2.)).text_size(z(12.));
        for r in rows {
            u = u.child(div().flex().items_center().gap_2().when(!r.applies, |d| d.opacity(0.6))
                .child(div().w(z(96.)).flex_shrink_0().overflow_hidden().whitespace_nowrap().text_color(c("dim")).child(r.label)).child(div().w(z(66.)).flex_shrink_0().text_color(c(r.tone)).child(r.left))
                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(z(11.)).text_color(c("faint")).child(r.resets)));
        }
        for n in notes { u = u.child(div().text_size(z(11.)).text_color(c("dim")).child(n)) }
        let usage = div().flex_1().min_w_0().flex().flex_col().child(head("Usage", String::new(), false)).child(u);
        // under the picker: Account beside Usage when the composer is wide, one over the other when it is narrow
        side = if narrow { side.child(acct).child(div().mx_2().my_1().h(px(1.)).bg(c("line"))).child(usage) } else { side.flex_row().items_start().child(acct).child(usage.border_l_1().border_color(c("line"))) };
    }
    // the picker (screens/picker.rs: agent, model and effort, each column with its own head and keys) takes the menu's whole width, Account and Usage under it
    // Account and Usage keep to their own room: they scroll inside it rather than push the picker's lists out of the window
    let side = div().id("combo-side").max_h(px((f.menu_h * 0.35).max(120.))).overflow_y_scroll().child(side);
    let model_col = div().w_full().min_w_0().flex().flex_col().children(model);
    let body = div().flex().flex_col().child(model_col).when(accounts_on, |d| d.child(div().h(px(1.)).bg(c("line"))).child(side));
    Some(div().id("combo-menu").w_full().max_w(px(f.w)).max_h(px(f.menu_h)).overflow_y_scroll().border_1().border_color(c("line")).bg(c("panel")).child(body).into_any_element())
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
    #[test] fn low_and_out_read_in_their_colours_and_the_hover_says_everything() {
        assert_eq!((tone(&combo()), tone(&json!({"tone":"warn"})), tone(&json!({"tone":"bad"}))), ("text", "warn", "bad"));
        assert_eq!(tip(&combo(), "Ctrl+M", "Ctrl+Shift+A"), "kellyhlord@gmail.com (saved account)\nWeekly  36% left · resets Wed 11 Nov 17:00\nCtrl+M model · Ctrl+Shift+A account");
        let (rows, notes) = usage_rows(&combo());
        assert_eq!(rows[0], UsageRow { label: "Weekly".into(), left: "36% left".into(), resets: "resets Wed 11 Nov 17:00".into(), tone: "text", applies: true });
        assert!(!rows[1].applies && rows[1].resets.is_empty()); assert_eq!(notes, ["Extra usage $0.00 of $50.00 · off (out of credits)"]);
    }
}

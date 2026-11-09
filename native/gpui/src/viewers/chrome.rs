//! The browser's chrome, shared by both Browser pages (the embedded Chromium and the hub-frame fallback): a tab strip that is always there, a toolbar with icon buttons
//! (back, forward, reload or stop), an address field and "Hand back". Drawn only from the app's theme tokens, so light and dark match the rest of the window; the page decides
//! what each press does (`Act`).
use crate::{art, theme};
use gpui::{prelude::*, *};

pub struct TabView { pub id: String, pub title: String, pub url: String, pub active: bool }
#[derive(Clone, Debug, PartialEq)]
pub enum Act { Back, Forward, Reload, NewTab, Select(String), Close(String), FocusBar, HandBack }
pub struct In<'a> { pub light: bool, pub tabs: &'a [TabView], /** what the field shows: the address being typed while it has the focus, else the page's */ pub value: String,
    pub focused: bool, pub taken: bool, pub have: bool, pub loading: bool, pub can_back: bool, pub can_forward: bool, pub bar_focus: &'a FocusHandle }

/// The label of a tab: its title, else the address without its scheme, else "New tab".
pub fn tab_label(title: &str, url: &str) -> String {
    let t = title.trim(); if !t.is_empty() { return t.chars().take(24).collect() }
    let u = url.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');
    if u.is_empty() || u == "about:blank" { crate::i18n::trs("New tab") } else { u.chars().take(24).collect() }
}

fn icon_button(light: bool, id: &'static str, name: &'static str, on: bool) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    div().id(id).w(px(28.)).h(px(28.)).flex().items_center().justify_center().flex_shrink_0().border_1().border_color(c("bg"))
        .child(art::icon(name, 16., c(if on { "text" } else { "faint" })))
        .when(on, |d| d.cursor_pointer().hover(|s| s.bg(c("raise")).border_color(c("line")))).when(!on, |d| d.opacity(0.6))
}

/// (the tab strip, the toolbar).
pub fn render<T: 'static>(i: In, cx: &mut Context<T>, on: fn(&mut T, Act, &mut Window, &mut Context<T>), key: fn(&mut T, &KeyDownEvent, &mut Window, &mut Context<T>)) -> (AnyElement, AnyElement) {
    let light = i.light; let c = |n: &str| theme::color(light, n);
    let mut strip = div().h(px(32.)).flex().flex_row().items_end().bg(c("panel")).border_b_1().border_color(c("line")).text_size(px(12.)).px_1().gap_px().overflow_hidden();
    for (n, t) in i.tabs.iter().enumerate() {
        let (sel, close, active) = (t.id.clone(), t.id.clone(), t.active);
        strip = strip.child(div().h(px(28.)).min_w(px(96.)).max_w(px(200.)).flex().flex_row().items_center().gap_1().pl_3().pr_1()
            .border_1().border_b_0().border_color(c(if active { "line" } else { "panel" })).bg(c(if active { "bg" } else { "panel" })).text_color(c(if active { "text" } else { "dim" }))
            .child(div().id(("btab", n)).flex_1().h_full().flex().items_center().cursor_pointer().overflow_hidden().child(tab_label(&t.title, &t.url))
                .on_click(cx.listener(move |this, _, w, cx| on(this, Act::Select(sel.clone()), w, cx))))
            .child(div().id(("btabx", n)).w(px(20.)).h(px(20.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c("raise"))).child(art::icon("x", 12., c("dim")))
                .on_click(cx.listener(move |this, _, w, cx| on(this, Act::Close(close.clone()), w, cx)))));
    }
    strip = strip.child(div().id("br-new").w(px(28.)).h(px(28.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c("raise"))).child(art::icon("plus", 14., c("text")))
        .on_click(cx.listener(move |this, _, w, cx| on(this, Act::NewTab, w, cx))));
    let border = if i.focused { c("accent") } else { c("line") };
    let shown = if i.value.is_empty() { crate::i18n::trs("Search or enter address") } else { i.value.clone() };
    let bar = div().h(px(40.)).flex().flex_row().items_center().gap_1().px_2().bg(c("bg")).border_b_1().border_color(c("line"))
        .child(icon_button(light, "br-back", "chevron-left", i.have && i.can_back).on_click(cx.listener(move |this, _, w, cx| on(this, Act::Back, w, cx))))
        .child(icon_button(light, "br-fwd", "chevron-right", i.have && i.can_forward).on_click(cx.listener(move |this, _, w, cx| on(this, Act::Forward, w, cx))))
        .child(icon_button(light, "br-reload", if i.loading { "x" } else { "undo" }, i.have).on_click(cx.listener(move |this, _, w, cx| on(this, Act::Reload, w, cx))))
        .child(div().id("br-url").flex_1().h(px(28.)).px_2().flex().flex_row().items_center().gap_2().text_size(px(13.)).bg(c("panel")).border_1().border_color(border)
            .track_focus(i.bar_focus).key_context("BrowserBar").key_context("Viewer").cursor_text()
            .on_key_down(cx.listener(move |this, e, w, cx| key(this, e, w, cx)))
            .on_click(cx.listener(move |this, _, w, cx| on(this, Act::FocusBar, w, cx)))
            .child(art::icon("globe", 14., c("dim")))
            .child(div().flex_1().overflow_hidden().text_color(c(if i.value.is_empty() { "dim" } else { "text" })).child(format!("{}{}", shown, if i.focused { "▏" } else { "" }))))
        .when(i.taken, |d| d.child(div().id("br-hand").h(px(28.)).px_3().flex().items_center().cursor_pointer().bg(c("accent")).text_color(c("on-accent")).text_size(px(12.)).child(crate::i18n::trs("Hand back"))
            .on_click(cx.listener(move |this, _, w, cx| on(this, Act::HandBack, w, cx)))));
    (strip.into_any_element(), bar.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::tab_label;
    #[test] fn a_tab_is_named_by_its_title_then_its_address_then_new_tab() {
        assert_eq!(tab_label("Orders", "http://x/"), "Orders"); assert_eq!(tab_label("", "https://example.com/a/"), "example.com/a"); assert_eq!(tab_label("", "about:blank"), "New tab"); assert_eq!(tab_label(" ", ""), "New tab");
    }
}

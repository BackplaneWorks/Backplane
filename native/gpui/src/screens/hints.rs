//! The keys you can see (0.14's workspace-hints.js and its style.css "Keys you can see"): while Ctrl (Cmd on a Mac) is held where its shortcuts work, each of the
//! open project's first nine threads shows its digit, each rail view its bare key, the palette bar and the model chip their keys, and a short legend sits at the
//! sidebar's foot; while Alt is held each of the first nine projects shows its digit. Nothing is drawn otherwise, and what shows is laid over the window, so nothing
//! moves when it comes and goes (the tooltips still name every key). What is held and where: keys::hint_mode.
use crate::{theme, Backplane};
use gpui::{prelude::*, *};

/// A keycap: faint, or lit (the accent's edge) for the one that does what is on show.
pub fn cap(light: bool, text: &str, lit: bool) -> Div {
    let c = |n: &str| theme::color(light, n);
    div().flex_shrink_0().px(px(3.)).border_1().border_b_2().border_color(c(if lit { "accent" } else { "line" })).bg(c("bg")).text_size(px(9.)).line_height(px(11.))
        .font(crate::art::mono_font()).text_color(c(if lit { "accent" } else { "faint" })).child(text.to_string())
}
/// A row's digit (a thread's or a project's): a box with the accent's edge, as the old sidebar drew it.
pub fn digit(light: bool, n: usize) -> Div {
    let c = |n: &str| theme::color(light, n);
    div().flex_shrink_0().size(px(16.)).flex().items_center().justify_center().border_1().border_color(c("accent")).bg(c("panel")).text_size(px(11.)).line_height(px(14.))
        .font(crate::art::mono_font()).text_color(c("text")).child(format!("{n}"))
}
/// Laid over the end of a row's words (a parent that is `relative`), so the row keeps its layout while the hint shows.
pub fn over_end(e: Div) -> Div { div().absolute().right_0().top_0().bottom_0().flex().items_center().child(e) }
/// Where `id` is among the digits' targets: its digit (1-9).
pub fn digit_of(list: &[String], id: &str) -> Option<usize> { list.iter().position(|x| x == id).map(|i| i + 1) }

/// The legend at the sidebar's foot; its last row opens Commands, which lists every key.
pub fn legend(cx: &mut Context<Backplane>, light: bool, l: &crate::local::Local) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let rows = crate::keys::legend(l);
    let last = rows.len().saturating_sub(1);
    div().id("keys-legend").absolute().left_0().right_0().bottom_0().px_3().pt_2().pb(px(10.)).bg(c("panel")).border_t_1().border_color(c("line")).flex().flex_col().gap(px(4.))
        .text_size(px(11.)).text_color(c("faint"))
        .children(rows.into_iter().enumerate().map(|(i, (keys, what))| {
            let row = div().id(("legend", i)).flex().items_center().gap(px(10.)).child(div().w(px(104.)).flex_shrink_0().flex().gap(px(3.)).children(keys.iter().map(|k| cap(light, k, false))))
                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(what));
            if i == last { row.cursor_pointer().hover(|s| s.text_color(c("text"))).on_click(cx.listener(|this, _, _, cx| this.palette_open(cx))).into_any_element() } else { row.into_any_element() }
        }).collect::<Vec<_>>())
        .into_any_element()
}

impl Backplane {
    /// What the hints show this frame, and the rows their digits go to (the same lists the keys use, so a digit on show is where its key goes).
    pub(crate) fn hints_frame(&mut self, window: &Window, cx: &App) {
        let held = self.hint_mode(window, cx);
        self.screens.held = held;
        self.screens.hold_threads = if held == crate::keys::Held::Threads { crate::keys::here_threads(&self.data, &self.selected, self.project_page.as_deref()) } else { vec![] };
        self.screens.hold_projects = if held == crate::keys::Held::Projects { crate::keys::digit_projects(&self.data) } else { vec![] };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn a_digit_is_its_place_among_the_targets() {
        let l = vec!["h|a".to_string(), "h|b".to_string()];
        assert_eq!(digit_of(&l, "h|b"), Some(2)); assert_eq!(digit_of(&l, "h|c"), None); assert_eq!(digit_of(&[], "x"), None);
    }
}

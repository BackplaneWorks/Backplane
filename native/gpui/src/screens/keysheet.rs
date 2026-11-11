//! The keyboard shortcuts sheet: every key of `keys::TABLE`, with this device's choices, under its place. Opened from the palette ("Keyboard
//! shortcuts") and with `?` or F1 when no text box has the keyboard; Esc, a click outside or the same key shuts it. Settings, This device changes them.
//! Setup lists no keys (docs/ALWAYS-ON.md, D3): this sheet is where they are.
use crate::{i18n::trs, keys, local::Local, theme, Backplane};
use gpui::{prelude::*, *};

/// The table's rows under their places, in the table's order: (place, [(what it does, its key)]).
pub fn groups(l: &Local) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    let mut out: Vec<(&'static str, Vec<(&'static str, String)>)> = vec![];
    for x in keys::TABLE {
        let k = keys::hint(l, x.id);
        if k.is_empty() { continue }
        match out.iter_mut().find(|(s, _)| *s == x.scope) { Some((_, v)) => v.push((x.label, k)), None => out.push((x.scope, vec![(x.label, k)])) }
    }
    out
}

impl Backplane {
    pub(crate) fn keysheet_toggle(&mut self, cx: &mut Context<Self>) { self.screens.keysheet = !self.screens.keysheet; cx.notify(); }
    /// Esc: shut it if it is open.
    pub(crate) fn keysheet_close(&mut self, cx: &mut Context<Self>) -> bool { let was = self.screens.keysheet; if was { self.screens.keysheet = false; cx.notify(); } was }
}

pub fn view(cx: &mut Context<Backplane>, light: bool, open: bool, l: &Local) -> Option<AnyElement> {
    if !open { return None }
    let c = |n: &str| theme::color(light, n);
    let mut list = div().flex().flex_col().gap_1();
    for (place, rows) in groups(l) {
        list = list.child(div().pt_3().pb_1().text_size(px(11.)).text_color(c("faint")).child(trs(place)));
        for (what, key) in rows {
            list = list.child(div().flex().justify_between().gap_4().py(px(2.)).text_size(px(13.))
                .child(div().flex_1().min_w_0().text_color(c("dim")).child(trs(what)))
                .child(div().flex_shrink_0().px_2().bg(c("raise")).text_color(c("text")).child(key)));
        }
    }
    /* i18n-ok: drawn through trs */ let title = trs("Keyboard shortcuts");
    let note = crate::i18n::t("gpui.keysheet.note");
    let sheet = div().id("keysheet").relative().w(px(640.)).max_h(relative(0.85)).flex().flex_col().bg(c("panel")).border_1().border_color(c("line")).text_color(c("text"))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        // where it is, for the window's checks (BACKPLANE_GPUI_RECTS=1: test/onboarding/run.sh finds it)
        .child(canvas(|_, _, _| (), |b, _, w, _| crate::art::report("keysheet", b, w.scale_factor())).absolute().size_full())
        .child(div().px_6().pt_5().pb_2().flex().flex_col().gap_1().child(div().text_size(px(18.)).font_weight(FontWeight::BOLD).child(title)).child(div().text_size(px(12.)).text_color(c("faint")).child(note)))
        .child(div().id("keysheet-list").flex_1().min_h_0().overflow_y_scroll().px_6().pb_5().child(list));
    Some(div().id("keysheet-back").absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(hsla(0., 0., 0., 0.45)).occlude()
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.keysheet_close(cx); }))
        .child(sheet).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn every_key_with_a_hint_is_listed_once_under_its_place() {
        let l = Local::default();
        let g = groups(&l);
        let n: usize = g.iter().map(|(_, r)| r.len()).sum();
        assert_eq!(n, keys::TABLE.iter().filter(|x| !keys::hint(&l, x.id).is_empty()).count());
        let places: Vec<&str> = g.iter().map(|(p, _)| *p).collect(); let mut d = places.clone(); d.dedup(); assert_eq!(places, d, "each place once");
        assert!(g.iter().any(|(_, r)| r.iter().any(|(w, _)| w.contains("Commands"))), "the palette is in it");
        assert!(g.iter().any(|(_, r)| r.iter().any(|(w, _)| w.starts_with("Keyboard shortcuts"))), "and the sheet's own key");
    }
}

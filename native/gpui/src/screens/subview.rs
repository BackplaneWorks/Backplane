//! A subagent's own view over its thread: what it was asked, what it said, each tool call with its result and how it ended, live while it works.
//! Opened from its row in a tool fold (`sub` on an Agent call) or in the Subagents panel (`key`); Back (or Escape) returns to the thread. The hub
//! sends it as the thread's `subView` (present/panels.rs `sub_view`) while this window's session names it (`sub`); the window draws it as it comes.
use crate::{art, hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

/// The view the window asked for, by the thread it opened over and its key: drawn while the hub's answer is on its way, and gone with another thread.
pub fn wanted(p: &super::panels::Panels, thread: &Value) -> Option<String> {
    p.sub_view.as_ref().filter(|(t, _)| *t == string(thread, "id")).map(|(_, k)| k.clone())
}
/// The view to draw: the hub's, when it is the one asked for (a late answer for a view left is not drawn).
pub fn shown<'a>(p: &super::panels::Panels, thread: &'a Value) -> Option<&'a Value> {
    let k = wanted(p, thread)?; let v = &thread["subView"]; (v.is_object() && v["key"].as_str() == Some(k.as_str())).then_some(v)
}
/// The words over a tool call's mark: running, failed, or done.
pub fn mark(state: &str) -> &'static str { match state { "running" => "\u{25b8}", "failed" => "\u{2717}", _ => "\u{2713}" } }
/// A result's lines shown shut (the rest open with a click), and whether there is more.
pub fn result_lines(text: &str, open: bool) -> (String, bool) {
    let ls: Vec<&str> = text.lines().collect(); if open || ls.len() <= 3 { return (text.to_string(), false) }
    (ls[..3].join("\n"), true)
}

impl Backplane {
    /// Open a subagent's view (its call's id) over the thread on screen.
    pub(crate) fn sub_open(&mut self, key: String, cx: &mut Context<Self>) {
        let t = string(&self.data["thread"], "id"); if key.is_empty() || t.is_empty() { return }
        self.screens.panels.sub_view = Some((t, key.clone())); self.screens.panels.sub_seen = 0;
        let _ = self.tx.try_send(Command::Sub(key)); self.ui_epoch += 1; cx.notify();
    }
    /// Back to the thread.
    pub(crate) fn sub_back(&mut self, cx: &mut Context<Self>) {
        if self.screens.panels.sub_view.take().is_none() { return }
        let _ = self.tx.try_send(Command::Sub(String::new())); self.ui_epoch += 1; cx.notify();
    }
    /// The view in place of the thread's messages.
    pub(crate) fn sub_page(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light(); let c = |n: &str| theme::color(light, n);
        let thread = &self.data["thread"];
        let v = shown(&self.screens.panels, thread).cloned().unwrap_or(Value::Null);
        let back = crate::ui::with_rect(div().id("sub-back"), || "ui:Back".into()).flex_shrink_0().flex().items_center().gap_1().px_2().py_1().border_1().border_color(c("line")).cursor_pointer().hover(|s| s.bg(c("raise")))
            .text_size(crate::scale::tpx(12.)).text_color(c("text")).child(art::icon("chevron-left", 13., c("dim"))).child({ let t = string(thread, "title"); if t.is_empty() { crate::i18n::trs("Back to the thread") } else { crate::i18n::tf("gpui.subview.back", &[("title", t.into())]) } })
            .on_click(cx.listener(|this, _, _, cx| this.sub_back(cx)));
        let state = string(&v, "state"); let live = v["live"] == true;
        let took = string(&v, "took");
        let mut head = div().w_full().flex().items_center().gap_3().child(back)
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("text")).child(if v.is_null() { crate::i18n::trs("Subagent") } else { string(&v, "title") }))
            .child(div().flex_shrink_0().text_size(crate::scale::tpx(12.)).text_color(if live { c("accent") } else { c("faint") }).child(if took.is_empty() { state.clone() } else { format!("{} · {took}", crate::i18n::trs(&state)) /* i18n-ok: a join of translated parts */ }));
        if !string(&v, "act").is_empty() {
            let (act, value) = (string(&v, "act"), string(&v, "value"));
            /* i18n-ok: a control id, and a join of translated parts */ head = head.child(crate::ui::with_rect(div().id("sub-stop"), || "ui:Stop subagent".into()).flex_shrink_0().px_2().cursor_pointer().text_size(crate::scale::tpx(12.)).text_color(c("accent")).child(string(&v, "label"))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(act.clone(), value.clone())); cx.notify(); })));
        }
        let mut top = div().w_full().max_w(px(930.)).px_4().pt_3().pb_2().flex().flex_col().gap_1().child(head);
        for l in [string(&v, "meta"), string(&v, "where")].into_iter().filter(|l| !l.is_empty()) { top = top.child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(l)); }
        let mut col = div().w_full().max_w(px(930.)).px_4().pb_4().flex().flex_col().gap_1();
        if v.is_null() { col = col.child(div().py_2().text_color(c("dim")).child(crate::i18n::trs("Loading…"))); }
        let more = v["more"].as_u64().unwrap_or(0);
        if more > 0 { col = col.child(div().py_1().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(crate::i18n::tf("gpui.subview.more", &[("more", more.into())]))); }
        let es: Vec<Value> = v["entries"].as_array().cloned().unwrap_or_default();
        if !v.is_null() && es.is_empty() { col = col.child(div().py_2().text_color(c("dim")).child(crate::i18n::trs("No steps reported yet."))); }
        for e in &es {
            let id = string(e, "id"); let text = string(e, "text");
            let el: AnyElement = match e["kind"].as_str().unwrap_or("") {
                "user" => div().w_full().py_2().child(div().px_3().py_2().bg(c("raise")).border_1().border_color(c("line")).flex().flex_col().gap_1()
                    .child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(crate::i18n::trs("Asked"))).child(div().text_color(c("text")).child(text))).into_any_element(),
                "assistant" => div().w_full().py_2().text_color(c("text")).child(text).into_any_element(),
                "tool" => {
                    let t = &e["tool"]; let st = string(t, "state"); let res = string(t, "result");
                    let open = self.screens.panels.sub_results.contains(&id);
                    let (shown_res, cut) = result_lines(&res, open);
                    let tone = match st.as_str() { "running" => c("accent"), "failed" => c("bad"), _ => c("faint") };
                    let mut row = div().id(SharedString::from(format!("sub-e-{id}"))).w_full().py_1().flex().flex_col().font(art::mono_font()).text_size(crate::scale::tpx(12.))
                        .child(div().flex().gap_2().child(div().w(px(12.)).flex_shrink_0().text_color(tone).child(mark(&st))).child(div().flex_shrink_0().text_color(if st == "running" { c("accent") } else { c("faint") }).child(string(t, "name")))
                            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("dim")).child(string(t, "summary"))));
                    if !shown_res.is_empty() {
                        row = row.child(div().pl(px(20.)).text_color(if st == "failed" { c("bad") } else { c("faint") }).child(if cut { format!("{shown_res}\n…") } else { shown_res }));
                        if cut || open { let k = id.clone(); row = row.cursor_pointer().hover(|s| s.bg(c("raise"))).on_click(cx.listener(move |this, _, _, cx| { let o = &mut this.screens.panels.sub_results; if !o.remove(&k) { o.insert(k.clone()); } cx.notify(); })); }
                    }
                    row.into_any_element()
                }
                _ => { let tone = string(e, "tone");
                    div().w_full().py_1().flex().gap_2().text_size(crate::scale::tpx(12.)).child(div().flex_shrink_0().text_color(c("faint")).child(string(e, "label")))
                        .child(div().flex_1().min_w_0().text_color(if tone.contains("error") { c("bad") } else { c("dim") }).child(text)).into_any_element() }
            };
            col = col.child(el);
        }
        // live: a new step scrolls to it when the view was at its end (or was just opened)
        let n = es.len(); let h = self.screens.panels.sub_scroll.clone();
        let at_end = { let (o, m) = (h.offset().y, h.max_offset().height); -o >= m - px(40.) };
        if n != self.screens.panels.sub_seen { if self.screens.panels.sub_seen == 0 || at_end { h.scroll_to_bottom(); } self.screens.panels.sub_seen = n; }
        div().id("subview").size_full().flex().flex_col()
            .child(div().w_full().flex().justify_center().border_b_1().border_color(c("line")).child(top))
            .child(div().id("sub-steps").flex_1().min_h_0().overflow_y_scroll().track_scroll(&h).flex().flex_col().items_center().child(col))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use serde_json::json;
    #[test] fn the_view_drawn_is_the_one_asked_for_over_its_own_thread() {
        let mut p = super::super::panels::Panels::default();
        let t = json!({"id": "h|t1", "subView": {"key": "c1", "entries": []}});
        assert!(wanted(&p, &t).is_none() && shown(&p, &t).is_none(), "none asked");
        p.sub_view = Some(("h|t1".into(), "c1".into())); assert!(shown(&p, &t).is_some());
        p.sub_view = Some(("h|t1".into(), "c2".into())); assert_eq!(wanted(&p, &t).as_deref(), Some("c2")); assert!(shown(&p, &t).is_none(), "a late answer for another is not drawn");
        p.sub_view = Some(("h|t2".into(), "c1".into())); assert!(wanted(&p, &t).is_none(), "another thread's");
    }
    #[test] fn a_long_result_shows_three_lines_until_opened() {
        assert_eq!(result_lines("a\nb", false), ("a\nb".into(), false));
        assert_eq!(result_lines("a\nb\nc\nd", false), ("a\nb\nc".into(), true));
        assert_eq!(result_lines("a\nb\nc\nd", true), ("a\nb\nc\nd".into(), false));
        assert_eq!((mark("running"), mark("failed"), mark("done")), ("\u{25b8}", "\u{2717}", "\u{2713}"));
    }
}

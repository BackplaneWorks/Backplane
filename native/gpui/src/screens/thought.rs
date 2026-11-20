//! A thought the agent sent (#1325): the hub's activity row with `folds`, `open` and `value` (present/thread.rs `fold_thought`). Shut, its first line
//! beside a chevron, as the setting agent.thinking says ("folded"); a click, or Enter or Space on it, sends the hub's `fold` action as a run of tool
//! calls does, and the open row draws the whole text as Markdown (the hub's `blocks`), selectable as a reply is. No decision lives here.
use crate::script::{Ctl, CtlEntry};
use crate::{art, string, theme, ui::{ic, with_rect}, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

/// The line a shut thought shows: its first line with words, and " …" when more follows.
pub fn head_line(text: &str) -> String {
    let t = text.trim();
    let first = t.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if first.len() < t.len() { format!("{first} …") /* i18n-ok: the thought's own words and an ellipsis */ } else { first.to_string() }
}

/// A fold's head (a thought, a run of tool calls): a click flips it (the hub's `fold` with `value`) and gives it the keyboard. It is a tab stop with the
/// accent border while focused, and GPUI clicks a focused tab stop on Enter or Space, so the keyboard flips it through the same handler.
pub fn keyed(head: Stateful<Div>, key: &str, value: String, light: bool, cx: &mut Context<Backplane>) -> Stateful<Div> {
    let focus = crate::screens::copyable::focus_of(&SharedString::from(format!("fold-key-{key}")), cx);
    let f = focus.clone();
    // the keyboard is taken on the press, as a copy button's (copyable.rs): the timeline's own press would hand it to the composer
    head.track_focus(&focus).focus(move |s| s.border_color(theme::color(light, "accent")))
        .on_mouse_down(MouseButton::Left, move |_, w, cx| { w.focus(&f); cx.stop_propagation(); })
        .on_click(cx.listener(move |this, _, _, cx| { this.fold_click(value.clone()); cx.notify(); }))
}

/// The thought `e`; `body` its Markdown blocks once open (drawn by the timeline).
pub fn row(app: &mut Backplane, cx: &mut Context<Backplane>, light: bool, e: &Value, body: Vec<AnyElement>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (id, text, value, open) = (string(e, "id"), string(e, "text"), string(e, "value"), e["open"] == true);
    let line = head_line(&text);
    app.screens.parts.rows_ctl.push(CtlEntry { id: format!("think:{id}"), label: line.clone(), ctl: Ctl::Fold(value.clone()) });
    let head = with_rect(div().id(SharedString::from(format!("t-{id}"))), || format!("think:{id}")).w_full().min_w_0().flex().items_center().gap_2().px_1().py_1()
        .border_1().border_color(transparent_black()).font(art::mono_font()).text_size(crate::scale::tpx(12.)).text_color(c("dim")).cursor_pointer().hover(|s| s.bg(c("raise")))
        .child(div().flex_shrink_0().flex().items_center().child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 12., "faint")))
        .child(div().flex_shrink_0().text_color(c("faint")).child(crate::i18n::trs(&string(e, "label"))))
        .when(!open, |d| d.child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(line)));
    let head = keyed(head, &format!("think-{id}"), value, light, cx);
    let mut col = div().id(SharedString::from(format!("e-{id}"))).w_full().flex().flex_col().child(head);
    if open {
        // the whole thought, a step in from the chevron, a line at its left so it reads as the agent's thinking and not its reply
        let mut b = with_rect(div().id(SharedString::from(format!("tb-{id}"))), || format!("think-body:{id}")).ml(px(10.)).pl_3().py_1().border_l_1().border_color(c("line")).flex().flex_col().gap_2().opacity(0.85);
        for el in body { b = b.child(el) }
        col = col.child(b);
    }
    col.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::head_line;
    // the phones' DataFold `thoughtLine` is held to the same table (DataFoldTest, scripts/mobile/fold-check.sh)
    #[test] fn a_shut_thought_shows_its_first_line_and_says_more_follows() {
        let cases: serde_json::Value = serde_json::from_str(include_str!("../../../../test/mobile/thought_lines.json")).unwrap();
        for c in cases.as_array().unwrap() { assert_eq!(head_line(c["text"].as_str().unwrap()), c["line"].as_str().unwrap(), "{c}") }
    }
}

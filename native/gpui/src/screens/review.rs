//! The review bar over the message box (#1305): `thread.review` from the hub (native/host/src/agents/review_mode.rs `bar`), null while the thread
//! is no review. `{on, text, button, tip, value}`: while on, the thread is read-only and the bar says why; its one button switches review mode off
//! (Allow edits) or, just after, back on (Read-only again), as the hub's `thread.review`. The hub decides what it says; the bar only draws it.
use crate::{i18n::trs, script::{Ctl, CtlEntry}, string, theme, ui::{ic, with_rect}, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Bar { pub on: bool, pub text: String, pub button: String, pub tip: String, pub value: String }

pub fn parse(v: &Value) -> Option<Bar> {
    if !v.is_object() { return None }
    Some(Bar { on: v["on"] == true, text: string(v, "text"), button: string(v, "button"), tip: string(v, "tip"), value: if v["value"] == true { "true" } else { "false" }.into() })
}
/// The action the button sends (session/composer.rs `review-set`).
pub const ACTION: &str = "review-set";
/// The id the e2e drivers press it by.
pub const CTL: &str = "ui:review";

/// The button as a control the harness presses.
pub fn control(v: &Value) -> Option<CtlEntry> { parse(v).map(|b| CtlEntry { id: CTL.into(), label: b.button, ctl: Ctl::Node(ACTION.into(), b.value) }) }

pub fn view(cx: &mut Context<Backplane>, light: bool, v: &Value) -> Option<AnyElement> {
    let b = parse(v)?;
    let c = |n: &str| theme::color(light, n);
    let tone = if b.on { "warn" } else { "dim" };
    let value = b.value.clone();
    // (an id of each state's own: a tooltip shown over the button does not outlive the press that changed what it says)
    let btn = with_rect(div().id(if b.on { "review-btn-off" } else { "review-btn-on" }), || CTL.to_string()).flex_shrink_0().flex().items_center().gap_1().px_2().py(px(2.)).text_size(crate::scale::tpx(12.)).cursor_pointer().border_1()
        .border_color(c(if b.on { "warn" } else { "line" })).text_color(c(if b.on { "warn" } else { "text" })).hover(|s| s.bg(c("raise")))
        .tooltip(crate::keys::tip(&b.tip, "", light))
        .child(ic(light, if b.on { "square-pen" } else { "search" }, 12., tone)).child(trs(&b.button))
        .on_click(cx.listener(move |this, _, _, cx| { this.go(ACTION, value.clone(), cx); }));
    Some(div().id("review-bar").flex().items_center().gap_2().px_3().py_1().border_1().border_color(c(if b.on { "warn" } else { "line" })).bg(c("panel")).text_size(crate::scale::tpx(12.))
        .child(ic(light, if b.on { "search" } else { "square-pen" }, 13., tone))
        .child(div().flex_1().min_w_0().text_color(c(if b.on { "text" } else { "dim" })).child(trs(&b.text)))
        .child(btn).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    use serde_json::json;
    /// The hub's two bars (native/host/src/agents/review_mode.rs): on, the button switches it off; off, back on; none while no review.
    #[test] fn the_bar_draws_the_hubs_state_and_its_button_switches_it() {
        let on = json!({"on":true,"text":"Read-only: this request looked like a review, so Backplane refuses changes to the design files.","button":"Allow edits","tip":"t","value":false});
        let b = parse(&on).unwrap(); assert!(b.on && b.text.starts_with("Read-only: this request looked like a review") && b.button == "Allow edits" && b.value == "false");
        let c = control(&on).unwrap(); assert_eq!((c.id.as_str(), c.label.as_str()), (CTL, "Allow edits")); assert!(matches!(c.ctl, Ctl::Node(ref a, ref v) if a == ACTION && v == "false"));
        let off = json!({"on":false,"text":"Edits allowed","button":"Read-only again","tip":"t","value":true});
        assert!(matches!(control(&off).unwrap().ctl, Ctl::Node(_, ref v) if v == "true"), "pressed again: back on");
        assert!(parse(&Value::Null).is_none() && control(&Value::Null).is_none(), "no review: no bar");
        let icons: serde_json::Map<String, Value> = serde_json::from_str(include_str!("../../../../assets/icons.json")).unwrap();
        for n in ["search", "square-pen"] { assert!(icons.contains_key(n), "{n}") }
    }
}

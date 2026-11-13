//! The halt card: a turn that ended out of memory or out of context, drawn from the hub's entry (native/host/src/agents/halt_view.rs `entry`)
//! `{kind:"halt", cause, head, text, facts:[{label, value, fill?}], buttons:[{label, action, value, primary}], done, detail}`. What it says and
//! offers is the hub's; the card only draws it, sends a button's action, and opens its Details (the provider's own words) in place.
use crate::{script::{Ctl, CtlEntry}, string, theme, ui::{ic, with_rect}, Backplane};
use crate::i18n::{t, trs};
use gpui::{prelude::*, *};
use serde_json::Value;

/// The icon for a card's cause: a chip for memory, a gauge for the context.
pub fn icon_of(cause: &str) -> &'static str { if cause == "memory" { "cpu" } else { "gauge" } }
/// A meter's filled width, `w` px wide at `fill` (0..1, clamped; a sliver stays visible).
pub fn meter_w(w: f32, fill: f64) -> f32 { if fill <= 0.0 { 0.0 } else { (w * fill.min(1.0) as f32).max(2.0) } }

pub fn card(this: &mut Backplane, cx: &mut Context<Backplane>, light: bool, e: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (id, cause, head, text, done, detail) = (string(e, "id"), string(e, "cause"), string(e, "head"), string(e, "text"), string(e, "done"), string(e, "detail"));
    let open = this.screens.halt_open.contains(&id);
    let tone = if done.is_empty() { "warn" } else { "faint" };
    let top = div().flex().items_center().gap_2().child(ic(light, icon_of(&cause), 14., tone))
        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c("text")).child(trs(&head)));
    let mut facts = div().flex().flex_col().gap_1().mt_1();
    for f in e["facts"].as_array().into_iter().flatten() {
        let mut row = div().flex().items_center().gap_3().text_size(px(12.))
            .child(div().w(px(64.)).flex_shrink_0().text_color(c("faint")).child(trs(&string(f, "label"))));
        if let Some(fill) = f["fill"].as_f64() {
            row = row.child(div().w(px(140.)).h(px(4.)).flex_shrink_0().bg(c("line")).child(div().h_full().w(px(meter_w(140., fill))).bg(c(if fill >= 0.99 { "warn" } else { "dim" }))));
        }
        facts = facts.child(row.child(div().min_w_0().text_color(c("dim")).child(trs(&string(f, "value")))));
    }
    let mut acts = div().flex().flex_wrap().items_center().gap_2().mt_2();
    if !done.is_empty() {
        acts = acts.child(div().flex().items_center().gap_1().text_size(px(12.)).text_color(c("ok")).child(ic(light, "circle-check", 12., "ok")).child(trs(&done)));
    }
    for (i, b) in e["buttons"].as_array().into_iter().flatten().enumerate() {
        let (label, action, value, primary) = (string(b, "label"), string(b, "action"), string(b, "value"), b["primary"] == true);
        let key = format!("halt:{label}");
        this.screens.parts.rows_ctl.push(CtlEntry { id: key.clone(), label: label.clone(), ctl: Ctl::Go(action.clone(), value.clone()) });
        let btn = with_rect(div().id(SharedString::from(format!("halt-{id}-{i}"))), || key).px_3().py_1().text_size(px(12.)).cursor_pointer();
        let btn = if primary { btn.bg(c("accent")).text_color(c("on-accent")).hover(|s| s.opacity(0.9)) } else { btn.border_1().border_color(c("line")).text_color(c("text")).hover(|s| s.bg(c("raise"))) };
        acts = acts.child(btn.child(trs(&label)).on_click(cx.listener(move |this, _, _, cx| { this.go(&action, value.clone(), cx); })));
    }
    if !detail.trim().is_empty() {
        let key = id.clone();
        acts = acts.child(div().id(SharedString::from(format!("halt-d-{id}"))).ml_auto().flex().items_center().gap_1().text_size(px(11.)).text_color(c("faint")).cursor_pointer().hover(|s| s.text_color(c("dim")))
            .child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 11., "faint")).child(t("gpui.halt.details"))
            .on_click(cx.listener(move |this, _, _, cx| { if !this.screens.halt_open.remove(&key) { this.screens.halt_open.insert(key.clone()); } cx.notify(); })));
    }
    let mut body = div().flex_1().min_w_0().flex().flex_col().gap_1().px_3().py_2().child(top)
        .child(div().text_size(px(13.)).text_color(c("text")).child(trs(&text))).child(facts).child(acts);
    if open && !detail.trim().is_empty() {
        body = body.child(div().mt_1().px_2().py_1().bg(c("view")).border_1().border_color(c("line")).font(crate::art::mono_font()).text_size(px(11.)).text_color(c("dim")).child(detail));
    }
    div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().child(
        div().w_full().max_w(px(640.)).flex().border_1().border_color(c("line")).bg(c("panel"))
            .child(div().w(px(3.)).flex_shrink_0().bg(c(tone))).child(body)).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[core::prelude::v1::test] fn the_card_draws_its_cause_and_its_meter() {
        assert_eq!((icon_of("memory"), icon_of("context")), ("cpu", "gauge"));
        let icons: serde_json::Map<String, Value> = serde_json::from_str(include_str!("../../../../assets/icons.json")).unwrap();
        for n in ["cpu", "gauge", "circle-check", "chevron-down", "chevron-right"] { assert!(icons.contains_key(n), "{n} is in icons.json") }
        assert_eq!(meter_w(140., 0.5), 70.); assert_eq!(meter_w(140., 1.7), 140.); assert_eq!(meter_w(140., 0.001), 2.); assert_eq!(meter_w(140., 0.), 0.);
    }
}

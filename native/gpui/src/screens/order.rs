//! A buy thread's Order card: `thread.order` from the hub (settings/order.rs `card`), null for any other thread.
//! The hub decides everything: the steps and which is on, the line, the total with its customs line, and Buy now's link
//! (only while the quote can be paid). Buy now opens that link in the browser, as the phones do.
use crate::{i18n::{tf, trs}, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Card { pub name: String, pub steps: Vec<(String, String)>, pub line: String, pub waiting: bool, pub total: String, pub customs: String, pub pay: String, pub label: String, pub requote: bool }

pub fn parse(order: &Value) -> Option<Card> {
    if !order.is_object() { return None; }
    Some(Card {
        name: string(order, "name"),
        steps: order["steps"].as_array().into_iter().flatten().map(|s| (string(s, "label"), string(s, "mark"))).collect(),
        line: string(order, "line"), waiting: order["waiting"].as_bool().unwrap_or(false),
        total: string(order, "total"), customs: string(order, "customs"),
        pay: string(order, "pay"), label: string(order, "label"), requote: order["requote"].as_bool().unwrap_or(false),
    })
}

/// Only an http(s) link is opened: anything else the hub sent is drawn without a button.
pub fn payable(card: &Card) -> bool { card.pay.starts_with("https://") || card.pay.starts_with("http://") }

/// What Re-quote tells the agent: the parts could not be priced for lack of a distributor answer; make a new quote.
pub const REQUOTE_TEXT: &str = "Make a new quote: the parts could not be priced a moment ago (sourcing was unavailable); nothing needs to change on the board.";
/// The request a Re-quote press sends (as the composer's Send does); `thread` is the window's key, `<hub>|<id>`.
pub fn requote_request(thread: &str) -> Value { serde_json::json!({"thread": thread.rsplit('|').next().unwrap_or(""), "text": REQUOTE_TEXT, "mode": "steer"}) }
pub const REQUOTE_TAG: u64 = 0xF1_0000_0000;

pub fn view(cx: &mut Context<Backplane>, light: bool, order: &Value) -> Option<AnyElement> {
    let card = parse(order)?;
    let c = |n: &str| theme::color(light, n);
    let steps = div().flex().flex_wrap().gap_1().items_center().children(card.steps.iter().enumerate().map(|(i, (label, mark))| {
        let (fg, bold) = match mark.as_str() { "done" => (c("dim"), false), "on" => (c("accent"), true), _ => (c("faint"), false) };
        /* i18n-ok: a mark and the hub's step, drawn through trs */ let t = if mark == "done" { format!("✓ {}", trs(label)) } else { trs(label) };
        div().flex().gap_1().items_center().text_size(crate::scale::tpx(11.)).text_color(fg).when(bold, |d| d.font_weight(FontWeight::BOLD))
            .when(i > 0, |d| d.child(div().text_color(c("faint")).child("›"))).child(t).into_any_element()
    }).collect::<Vec<_>>());
    let mut col = div().id("order-card").flex().flex_col().gap_2().p_3().bg(c("panel")).border_1().border_color(c("line"))
        .child(div().flex().items_center().gap_2().child(div().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(13.)).child(if card.name.is_empty() { trs("Order") } else { tf("gpui.order.named", &[("name", card.name.as_str().into())]) })))
        .child(steps)
        .child(div().text_size(crate::scale::tpx(12.)).text_color(if card.waiting { c("dim") } else { c("text") }).child(trs(&card.line)));
    if !card.total.is_empty() {
        col = col.child(div().flex().flex_col().gap_1().child(div().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(14.)).child(card.total.clone()))
            .children((!card.customs.is_empty()).then(|| div().text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(trs(&card.customs)))));
    }
    if payable(&card) {
        let link = card.pay.clone();
        let label = if card.label.is_empty() { trs("Buy now") } else { trs(&card.label) };
        /* i18n-ok: a ui: id is what drivers press */ col = col.child(div().flex().child(crate::ui::with_rect(div().id("order-pay"), || "ui:Buy now".to_string()).px_3().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer()
            .bg(c("accent")).text_color(c("on-accent")).border_1().border_color(c("accent")).child(label)
            .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&link)))));
    }
    if card.requote {
        /* i18n-ok: a ui: id is what drivers press */ col = col.child(div().flex().child(crate::ui::with_rect(div().id("order-requote"), || "ui:Re-quote".to_string()).px_3().py_1().text_size(crate::scale::tpx(12.)).cursor_pointer()
            .bg(c("accent")).text_color(c("on-accent")).border_1().border_color(c("accent")).child(tf("gpui.order.requote", &[]))
            .on_click(cx.listener(move |this, _, _, cx| { let r = requote_request(&this.selected); let _ = this.tx.try_send(crate::hub::Command::Ask("turn.start".into(), r, REQUOTE_TAG)); cx.notify(); }))));
    }
    Some(col.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn requote_is_the_hubs_and_sends_a_steer_turn() {
        let v: Value = serde_json::from_str(r#"{"name":"b","steps":[],"line":"x","waiting":false,"total":"$1","customs":"","pay":"","label":"","requote":true}"#).unwrap();
        let c = parse(&v).unwrap(); assert!(c.requote && !payable(&c), "no Buy now beside Re-quote");
        let r = requote_request("hub|t_9"); assert_eq!(r["thread"], "t_9"); assert_eq!(r["mode"], "steer"); assert!(r["text"].as_str().unwrap().contains("new quote"));
        assert!(!parse(&serde_json::json!({"name":"b"})).unwrap().requote);
    }
    #[test] fn a_thread_without_an_order_has_no_card() { assert_eq!(parse(&Value::Null), None); }
    #[test] fn the_card_is_the_hubs() {
        let v: Value = serde_json::from_str(r#"{"name":"Main board","steps":[{"label":"Details","mark":"done"},{"label":"Quote","mark":"on"},{"label":"Pay","mark":""}],
            "line":"Quoted. Buy now to place the order.","waiting":false,"total":"$27.20","customs":"Includes import VAT and duties: nothing to pay on delivery.","pay":"http://127.0.0.1:3871/q/q_1?t=x","label":"Buy now · $27.20"}"#).unwrap();
        let c = parse(&v).unwrap();
        assert_eq!(c.steps[1], ("Quote".into(), "on".into())); assert_eq!(c.total, "$27.20"); assert!(payable(&c));
        let mut v2 = v.clone(); v2["pay"] = "".into(); assert!(!payable(&parse(&v2).unwrap()), "no link, no button");
        v2["pay"] = "javascript:alert(1)".into(); assert!(!payable(&parse(&v2).unwrap()), "only http(s) opens");
    }
}

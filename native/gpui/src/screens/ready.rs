//! The Ready to order bar above the composer (`shown.ready` from the hub: present/ready.rs): what there is to order and its estimate, put away with its ×
//! (`fab-min`) and brought back by a small "Ready" chip (`fab-show`). Buy it itself is not here: the hub has no `order.buy` yet, so nothing on this bar starts an order.
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

/// The estimate beside the line: the button's words less "Buy it · " ("from $5"); "" when it has none.
pub fn price(buy: &str) -> String { buy.strip_prefix("Buy it · ").unwrap_or("").to_string() }

pub fn view(cx: &mut Context<Backplane>, light: bool, ready: &Value) -> Option<AnyElement> {
    if !ready.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    let (min, again) = (ready["min"] == true, ready["again"] == true);
    if min {
        return Some(div().flex().justify_end().child(div().id("ready-show").px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).text_color(c("dim"))
            .child(if again { "Ordered" } else { "Ready" }).on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("fab-show".into(), String::new())); cx.notify(); })).into_any_element()).into_any_element());
    }
    let p = price(&string(ready, "buy"));
    Some(div().id("ready-bar").flex().items_center().gap_2().px_3().py_1().border_1().border_color(c("line")).bg(c("raise")).text_size(px(12.))
        .child(div().flex_1().min_w_0().child(string(ready, "line")))
        .children((!p.is_empty() && !again).then(|| div().text_color(c("dim")).child(p)))
        .child(div().id("ready-away").px_1().cursor_pointer().text_color(c("dim")).child("×").on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("fab-min".into(), String::new())); cx.notify(); })))
        .into_any_element())
}

#[cfg(test)]
mod tests {
    use super::price;
    #[test]
    fn the_price_is_what_follows_buy_it() {
        assert_eq!(price("Buy it \u{b7} from $5"), "from $5");
        assert_eq!(price("Buy it"), "");
        assert_eq!(price("Buy again"), "");
    }
}

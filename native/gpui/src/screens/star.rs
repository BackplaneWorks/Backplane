//! The GitHub star suggestion: a strip at the top of the window while the hub says its offer is out and undecided. The hub decides
//! everything (state/star.rs): it sends `{"t":"star","offer"}` at the person's fifth prompt and `offer:null` once it is decided, and
//! answers `star.offer` on every (re)connect. The link opens the URL and decides `open`; Dismiss decides `dismiss`.
use crate::{hub::Command, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Offer { pub token: String, pub text: String, pub url: String, pub label: String }

/// The offer the hub sent, None when there is none to show (null, or a shape this client cannot act on).
pub fn of(v: &Value) -> Option<Offer> {
    let g = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let o = Offer { token: g("token"), text: g("text"), url: g("url"), label: g("label") };
    (!o.token.is_empty() && !o.text.is_empty() && o.url.starts_with("https://")).then_some(o)
}
/// The request that records the person's choice.
pub fn decision(o: &Offer, action: &str) -> Command { Command::Rpc("star.nudge".into(), Secret(json!({"token": o.token, "action": action})), "star-nudge") }

pub fn strip(cx: &mut Context<Backplane>, o: Option<&Offer>, light: bool) -> Option<AnyElement> {
    let o = o?;
    let c = |n: &str| theme::color(light, n);
    let btn = |id: &'static str, label: String, strong: bool| div().id(id).px_2().h(px(16.)).flex().items_center().cursor_pointer().text_size(px(11.)).font_weight(FontWeight::BOLD)
        .bg(if strong { c("accent") } else { c("raise") }).text_color(if strong { c("on-accent") } else { c("dim") }).child(label);
    let row = div().flex().items_center().gap_1()
        .child(btn("star-open", if o.label.is_empty() { "Star on GitHub".into() } else { o.label.clone() }, true).on_click(cx.listener(|this, _, _, cx| this.star_decide("open", cx))))
        .child(btn("star-dismiss", "Dismiss".into(), false).on_click(cx.listener(|this, _, _, cx| this.star_decide("dismiss", cx))));
    Some(div().id("star-nudge").h(px(22.)).flex_shrink_0().flex().items_center().justify_between().px_3().bg(c("raise")).border_b_1().border_color(c("accent"))
        .child(div().text_size(px(11.)).text_color(c("text")).overflow_hidden().child(o.text.clone())).child(row).into_any_element())
}

impl Backplane {
    /// The hub's word on the offer (a frame, or star.offer's answer on connect).
    pub(crate) fn star_offer(&mut self, v: &Value, cx: &mut Context<Self>) { let o = of(v); if self.screens.star != o { self.screens.star = o; cx.notify(); } }
    /// The person's choice: open also opens the page. The strip goes at once; the hub tells every other client to hide it.
    pub(crate) fn star_decide(&mut self, action: &str, cx: &mut Context<Self>) {
        let Some(o) = self.screens.star.take() else { return };
        if action == "open" { if let Err(e) = super::improvements::open_target(&o.url) { self.error = e; } }
        let _ = self.tx.try_send(decision(&o, action));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    fn offer() -> Value { json!({"token":"m5","text":"Like Backplane? Give us a star on GitHub!","url":"https://github.com/BackplaneWorks/Backplane","label":"Star on GitHub"}) }
    #[test] fn the_hubs_offer_is_shown_as_it_says() {
        let o = of(&offer()).unwrap();
        assert_eq!((o.token.as_str(), o.text.as_str(), o.url.as_str(), o.label.as_str()), ("m5", "Like Backplane? Give us a star on GitHub!", "https://github.com/BackplaneWorks/Backplane", "Star on GitHub"));
    }
    #[test] fn nothing_is_shown_without_an_offer() {
        assert_eq!(of(&Value::Null), None, "decided, or never offered");
        let mut v = offer(); v["token"] = json!(""); assert_eq!(of(&v), None);
        let mut v = offer(); v["url"] = json!("file:///etc/passwd"); assert_eq!(of(&v), None, "only a web page is opened");
    }
    #[test] fn a_choice_names_the_offered_token() {
        let o = of(&offer()).unwrap();
        for a in ["open", "dismiss"] { match decision(&o, a) { Command::Rpc(m, Secret(p), tag) => { assert_eq!((m.as_str(), tag), ("star.nudge", "star-nudge")); assert_eq!(p, json!({"token":"m5","action":a})); } c => panic!("{c:?}") } }
    }
}

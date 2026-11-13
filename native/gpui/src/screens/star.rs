//! The GitHub star suggestion: a small card in the window's upper right corner (nothing behind it is dimmed or blocked) while the hub says its offer is out and undecided.
//! The hub decides everything (state/star.rs): it sends `{"t":"star","offer"}` at the person's fifth prompt (or after a few days of use, or when a
//! Later's week is up) and `offer:null` once it is decided, and answers `star.offer` on every (re)connect. Star on GitHub opens the URL and decides
//! `open`; Later decides `later` (a week); Don't ask again decides `dismiss`. The card waits (`due`) while the thread on screen runs a turn,
//! while the composer has text and during setup, so it comes at a pause after a turn, never under the person's typing.
use crate::{hub::Command, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Offer { pub token: String, pub text: String, pub url: String, pub label: String, pub repo: String, pub why: String }

/// The offer the hub sent, None when there is none to show (null, or a shape this client cannot act on).
pub fn of(v: &Value) -> Option<Offer> {
    let g = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let o = Offer { token: g("token"), text: g("text"), url: g("url"), label: g("label"), repo: g("repo"), why: g("why") };
    (!o.token.is_empty() && !o.text.is_empty() && o.url.starts_with("https://")).then_some(o)
}
/// The request that records the person's choice.
pub fn decision(o: &Offer, action: &str) -> Command { Command::Rpc("star.nudge".into(), Secret(json!({"token": o.token, "action": action})), "star-nudge") }

/// Whether the card is drawn now: an offer, and a pause (no turn running in the thread on screen, nothing typed, setup not up).
pub fn due(o: Option<&Offer>, data: &Value, draft: &str) -> bool {
    o.is_some() && data["thread"]["sendAct"] != "interrupt" && draft.trim().is_empty() && !data["tour"].is_object() && !super::migrate::is_up(data)
}

pub fn card(cx: &mut Context<Backplane>, o: Option<&Offer>, light: bool) -> Option<AnyElement> {
    let o = o?;
    let c = |n: &str| theme::color(light, n);
    let tr = crate::i18n::trs;
    let rect = |d: Stateful<Div>, n: &'static str| if crate::art::rects_on() { d.relative().child(canvas(|_, _, _| (), move |r: Bounds<Pixels>, _, w: &mut Window, _| crate::art::report(n, r, w.scale_factor())).absolute().top_0().left_0().size_full()) } else { d };
    let small = |id: &'static str, label: &str| div().id(id).flex_1().h(crate::scale::tpx(26.)).flex().items_center().justify_center().cursor_pointer().text_size(crate::scale::tpx(12.)).text_color(c("dim")).bg(c("raise")).hover(|s| s.bg(c("line"))).child(tr(label));
    let repo = if o.repo.is_empty() { o.url.trim_start_matches("https://").trim_start_matches("github.com/").to_string() } else { o.repo.clone() };
    /* i18n-ok: drawn through tr */ let label = if o.label.is_empty() { "Star on GitHub".to_string() } else { o.label.clone() };
    let card = div().id("star-card").w(px(340.)).flex().flex_col().gap_2().p_4().bg(c("bg")).border_1().border_color(c("accent")).shadow_md().occlude()
        .child(div().flex().items_start().gap_2().child(div().flex_shrink_0().pt(px(1.)).child(crate::ui::ic(light, "star", 16., "accent")))
            .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                .child(div().text_size(crate::scale::tpx(13.)).font_weight(FontWeight::BOLD).text_color(c("text")).child(tr(&o.text)))
                .child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(format!("github.com/{repo}")))))
        .when(!o.why.is_empty(), |d| d.child(div().text_size(crate::scale::tpx(12.)).line_height(crate::scale::tpx(17.)).text_color(c("dim")).child(tr(&o.why))))
        .child(rect(div().id("star-open").w_full().h(crate::scale::tpx(30.)).flex().items_center().justify_center().gap_2().cursor_pointer().bg(c("accent")).text_color(c("on-accent")).text_size(crate::scale::tpx(13.)).font_weight(FontWeight::BOLD)
            .child(crate::ui::ic(light, "star", 14., "on-accent")).child(tr(&label)), "star:open").on_click(cx.listener(|this, _, _, cx| this.star_decide("open", cx))))
        .child(div().flex().gap_2()
            /* i18n-ok: drawn through tr */ .child(rect(small("star-later", "Later"), "star:later").on_click(cx.listener(|this, _, _, cx| this.star_decide("later", cx))))
            /* i18n-ok: drawn through tr */ .child(rect(small("star-dismiss", "Don't ask again"), "star:dismiss").on_click(cx.listener(|this, _, _, cx| this.star_decide("dismiss", cx)))));
    // under the top bar, clear of the right rail: where a conversation is emptiest
    Some(div().id("star-nudge").absolute().right(px(88.)).top(px(48.)).child(rect(card, "star")).into_any_element())
}

impl Backplane {
    /// The hub's word on the offer (a frame, or star.offer's answer on connect).
    pub(crate) fn star_offer(&mut self, v: &Value, cx: &mut Context<Self>) { let o = of(v); if self.screens.star != o { self.screens.star = o; cx.notify(); } }
    /// The card is on screen now.
    pub(crate) fn star_up(&self, cx: &App) -> bool { due(self.screens.star.as_ref(), &self.data, &self.input.read(cx).content) }
    /// The person's choice: open also opens the page. The card goes at once; the hub tells every other client to hide it.
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
    fn offer() -> Value { json!({"token":"m5","text":"Like Backplane? Give us a star on GitHub!","url":"https://github.com/BackplaneWorks/Backplane","label":"Star on GitHub","repo":"BackplaneWorks/Backplane","why":"Stars help."}) }
    #[test] fn the_hubs_offer_is_shown_as_it_says() {
        let o = of(&offer()).unwrap();
        assert_eq!((o.token.as_str(), o.text.as_str(), o.url.as_str(), o.label.as_str()), ("m5", "Like Backplane? Give us a star on GitHub!", "https://github.com/BackplaneWorks/Backplane", "Star on GitHub"));
        assert_eq!((o.repo.as_str(), o.why.as_str()), ("BackplaneWorks/Backplane", "Stars help."));
    }
    #[test] fn the_card_waits_for_a_pause() {
        let o = of(&offer()).unwrap(); let idle = json!({"thread": {"sendAct": "send"}});
        assert!(due(Some(&o), &idle, ""), "after a turn, nothing typed");
        assert!(!due(None, &idle, ""), "no offer");
        assert!(!due(Some(&o), &idle, "half a sentence"), "never under the person's typing");
        assert!(due(Some(&o), &idle, "  \n"), "blank is not typing");
        assert!(!due(Some(&o), &json!({"thread": {"sendAct": "interrupt"}}), ""), "a turn running in the thread on screen");
        assert!(!due(Some(&o), &json!({"tour": {"step": "agents"}}), ""), "setup");
        assert!(!due(Some(&o), &json!({"migrate": {"state": "running"}}), ""), "an import");
        assert!(due(Some(&o), &json!({}), ""), "no thread open");
    }
    #[test] fn nothing_is_shown_without_an_offer() {
        assert_eq!(of(&Value::Null), None, "decided, or never offered");
        let mut v = offer(); v["token"] = json!(""); assert_eq!(of(&v), None);
        let mut v = offer(); v["url"] = json!("file:///etc/passwd"); assert_eq!(of(&v), None, "only a web page is opened");
    }
    #[test] fn a_choice_names_the_offered_token() {
        let o = of(&offer()).unwrap();
        for a in ["open", "dismiss", "later"] { match decision(&o, a) { Command::Rpc(m, Secret(p), tag) => { assert_eq!((m.as_str(), tag), ("star.nudge", "star-nudge")); assert_eq!(p, json!({"token":"m5","action":a})); } c => panic!("{c:?}") } }
    }
}

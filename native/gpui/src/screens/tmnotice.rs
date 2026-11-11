//! The one-time telemetry notice of an install that predates the sender (hub: settings/telemetry.rs, docs/telemetry.md): a strip along the
//! bottom of the window, above the star strip, while the hub offers it. The hub decides everything: it answers `telemetry.notice` on every
//! (re)connect and sends `{"t":"telemetry.notice","notice"}` when it changes (null hides it). Drawing it the first time sends `shown`
//! (the hub starts sending only once the notice was on a screen); Turn off sends `off`, the close sends `close`; Settings opens Privacy.
use crate::{hub::Command, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Notice { pub token: String, pub text: String, pub off: String, pub settings: String }

/// The notice the hub sent, None when there is none to show (null, or one with no token or words).
pub fn of(v: &Value) -> Option<Notice> {
    let g = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let n = Notice { token: g("token"), text: g("text"), off: g("off"), settings: g("settings") };
    (!n.token.is_empty() && !n.text.is_empty()).then_some(n)
}
/// The request that tells the hub what happened to the notice.
pub fn act(n: &Notice, action: &str) -> Command { Command::Rpc("telemetry.notice.act".into(), Secret(json!({"token": n.token, "action": action})), "tm-notice") }

pub fn strip(cx: &mut Context<Backplane>, n: Option<&Notice>, shown: &str, light: bool) -> Option<AnyElement> {
    let n = n?;
    // drawn now: the hub hears it once per notice (deferred: a render sends nothing itself)
    if shown != n.token { let me = cx.entity(); cx.defer(move |cx| me.update(cx, |this, cx| this.tm_notice_shown(cx))); }
    let c = |k: &str| theme::color(light, k);
    let btn = |id: &'static str, label: String, strong: bool| div().id(id).px_2().h(px(16.)).flex().items_center().cursor_pointer().text_size(px(11.)).font_weight(FontWeight::BOLD)
        .bg(if strong { c("accent") } else { c("raise") }).text_color(if strong { c("on-accent") } else { c("dim") }).child(crate::i18n::trs(&label));
    let or = |s: &str, d: &str| if s.is_empty() { d.to_string() } else { s.to_string() };
    let row = div().flex().items_center().gap_1().flex_shrink_0()
        /* i18n-ok: drawn through trs in btn */ .child(btn("tm-off", or(&n.off, "Turn off"), true).on_click(cx.listener(|this, _, _, cx| this.tm_notice_act("off", cx))))
        /* i18n-ok: drawn through trs in btn */ .child(btn("tm-settings", or(&n.settings, "Settings"), false).on_click(cx.listener(|this, _, _, cx| this.tm_notice_settings(cx))))
        /* i18n-ok: drawn through trs in btn */ .child(btn("tm-close", "Close".into(), false).on_click(cx.listener(|this, _, _, cx| this.tm_notice_act("close", cx))));
    Some(div().id("tm-notice").min_h(px(22.)).flex_shrink_0().flex().items_center().justify_between().gap_2().px_3().py_1().bg(c("raise")).border_t_1().border_color(c("accent"))
        .child(div().flex_1().min_w_0().text_size(px(11.)).text_color(c("text")).child(crate::i18n::trs(&n.text))).child(row).into_any_element())
}

impl Backplane {
    /// The hub's word on the notice (a frame, or telemetry.notice's answer on connect).
    pub(crate) fn tm_notice(&mut self, v: &Value, cx: &mut Context<Self>) { let n = of(v); if self.screens.tm_notice != n { self.screens.tm_notice = n; cx.notify(); } }
    /// The strip was drawn: `shown`, once per notice and run of the app.
    pub(crate) fn tm_notice_shown(&mut self, _cx: &mut Context<Self>) {
        let Some(n) = self.screens.tm_notice.clone() else { return };
        if self.screens.tm_shown == n.token { return }
        if self.tx.try_send(act(&n, "shown")).is_ok() { self.screens.tm_shown = n.token; }
    }
    /// Turn off or Close: the strip goes at once; the hub tells every other client to hide it.
    pub(crate) fn tm_notice_act(&mut self, action: &str, cx: &mut Context<Self>) {
        let Some(n) = self.screens.tm_notice.take() else { return };
        let _ = self.tx.try_send(act(&n, action));
        cx.notify();
    }
    /// Settings, Privacy (the strip stays until it is closed or answered).
    pub(crate) fn tm_notice_settings(&mut self, cx: &mut Context<Self>) {
        if !self.data["settings"].is_object() { let _ = self.tx.try_send(Command::Action("flag".into(), "settings".into())); }
        super::settings::go_page(self, "privacy");
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    fn notice() -> Value { json!({"token":"telemetry-notice-1","text":"Backplane now sends anonymous usage counts.","off":"Turn off","settings":"Settings"}) }
    #[test] fn the_hubs_notice_is_shown_as_it_says() {
        let n = of(&notice()).unwrap();
        assert_eq!((n.token.as_str(), n.off.as_str(), n.settings.as_str()), ("telemetry-notice-1", "Turn off", "Settings"));
        assert_eq!(of(&Value::Null), None); assert_eq!(of(&json!({"token":"","text":"x"})), None); assert_eq!(of(&json!({"token":"t","text":""})), None);
    }
    #[test] fn each_press_names_the_notice_and_what_happened() {
        let n = of(&notice()).unwrap();
        for a in ["shown", "close", "off"] { match act(&n, a) { Command::Rpc(m, Secret(p), tag) => { assert_eq!((m.as_str(), tag), ("telemetry.notice.act", "tm-notice")); assert_eq!(p, json!({"token":"telemetry-notice-1","action":a})); } c => panic!("{c:?}") } }
    }
}

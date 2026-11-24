//! Proactive zero-provider notice over the composer (`shown["noProvider"]`), plus the one-click Accounts action
//! beside no-provider send-guard errors (`crate::settings::provider_notice`).
use crate::{i18n::{t, trs}, string, theme, ui::{ic, with_rect}, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub head: String,
    pub text: String,
    pub button: String,
    pub url: String,
}

/// Parse the hub's `shown["noProvider"]` object; None when null or empty.
pub fn parse(v: &Value) -> Option<Notice> {
    let head = v["head"].as_str().filter(|h| !h.trim().is_empty())?.to_string();
    let text = string(v, "text");
    let button = v["button"].as_str().filter(|b| !b.trim().is_empty()).map(str::to_string).unwrap_or_else(|| t("provider.notice.button"));
    let url = v["url"].as_str().filter(|u| !u.trim().is_empty()).unwrap_or("backplane://settings/accounts").to_string();
    Some(Notice { head, text, button, url })
}

pub fn is_send_error(err: &str) -> bool {
    err.starts_with("No AI provider connected yet.")
}

pub fn view(cx: &mut Context<Backplane>, light: bool, v: &Value, hidden: bool) -> Option<AnyElement> {
    if hidden { return None }
    let n = parse(v)?;
    let c = |k: &str| theme::color(light, k);
    let top = div().flex().items_center().gap_2()
        .child(ic(light, "plug", 14., "warn"))
        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c("text")).child(trs(&n.head)));
    let btn = with_rect(div().id("no-provider-open-accounts"), || "no-provider:accounts".to_string())
        .px_3().py_1().text_size(px(12.)).cursor_pointer()
        .bg(c("accent")).text_color(c("on-accent")).hover(|s| s.opacity(0.9))
        .child(trs(&n.button))
        .on_click(cx.listener(|this, _, _, cx| { this.open_accounts(cx); }));
    let acts = div().flex().flex_wrap().items_center().gap_2().mt_1().child(btn);
    let mut body = div().flex_1().min_w_0().flex().flex_col().gap_1().px_3().py_2().child(top);
    if !n.text.is_empty() {
        body = body.child(div().text_size(px(12.)).text_color(c("dim")).child(trs(&n.text)));
    }
    Some(with_rect(div().id("no-provider"), || "no-provider".to_string())
        .w_full().flex().border_1().border_color(c("line")).bg(c("panel"))
        .child(div().w(px(3.)).flex_shrink_0().bg(c("warn")))
        .child(body.child(acts))
        .into_any_element())
}

pub fn error_action(cx: &mut Context<Backplane>, light: bool, err: &str) -> Option<AnyElement> {
    if !is_send_error(err) { return None }
    let c = |k: &str| theme::color(light, k);
    Some(with_rect(div().id("no-provider-error-accounts"), || "no-provider:error-accounts".to_string())
        .px_2().py(px(2.)).border_1().border_color(c("line")).bg(c("panel")).text_size(px(11.)).text_color(c("accent")).cursor_pointer()
        .hover(|s| s.bg(c("raise")))
        .child(t("provider.notice.button"))
        .on_click(cx.listener(|this, _, _, cx| { this.open_accounts(cx); }))
        .into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn parses_hub_notice_and_detects_send_error() {
        let v = serde_json::json!({
            "reason": "none_installed",
            "head": "No AI provider connected",
            "text": "Connect Claude, Codex, Antigravity, or another provider to start chatting.",
            "button": "Open Settings → Accounts",
            "url": "backplane://settings/accounts",
        });
        let n = parse(&v).unwrap();
        assert_eq!(n.head, "No AI provider connected");
        assert_eq!(n.text, "Connect Claude, Codex, Antigravity, or another provider to start chatting.");
        assert_eq!(n.button, "Open Settings → Accounts");
        assert_eq!(n.url, "backplane://settings/accounts");
        assert_eq!(parse(&Value::Null), None);
        assert_eq!(parse(&serde_json::json!({"head": ""})), None);
        assert!(is_send_error("No AI provider connected yet. Open Settings → Accounts to connect Claude, Codex, Antigravity, or another provider."));
        assert!(!is_send_error("No AI provider connected on boat-hub. Open Settings → Accounts on boat-hub to connect Claude, Codex, Antigravity, or another provider."));
        assert!(!is_send_error("Some other error"));
        let icons: serde_json::Map<String, Value> = serde_json::from_str(include_str!("../../../../assets/icons.json")).unwrap();
        assert!(icons.contains_key("plug"));
    }
}

//! #1304: the notice over the composer when the agent has not answered a message the person sent into its running turn, drawn from the hub's
//! `thread.unanswered` (native/host/src/agents/unanswered.rs `picture`): `{head, text, since, msgs, buttons:[{label, action, value, primary}]}`, null
//! when there is none. What it says and offers is the hub's; a button sends its action (Interrupt: `interrupt`; Send again: `again`).
use crate::{i18n::trs, string, theme, ui::{ic, with_rect}, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Notice { pub head: String, pub text: String, pub buttons: Vec<(String, String, String, bool)> }

/// The hub's notice, or None (null, or one with nothing to say).
pub fn parse(v: &Value) -> Option<Notice> {
    let head = v["head"].as_str().filter(|h| !h.trim().is_empty())?.to_string();
    let buttons = v["buttons"].as_array().into_iter().flatten()
        .filter_map(|b| Some((b["label"].as_str()?.to_string(), b["action"].as_str()?.to_string(), string(b, "value"), b["primary"] == true))).collect();
    Some(Notice { head, text: string(v, "text"), buttons })
}

pub fn view(cx: &mut Context<Backplane>, light: bool, v: &Value) -> Option<AnyElement> {
    let n = parse(v)?;
    let c = |k: &str| theme::color(light, k);
    let top = div().flex().items_center().gap_2().child(ic(light, "clock", 14., "warn"))
        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(c("text")).child(trs(&n.head)));
    let mut acts = div().flex().flex_wrap().items_center().gap_2().mt_1();
    for (i, (label, action, value, primary)) in n.buttons.into_iter().enumerate() {
        let key = format!("unanswered:{action}");
        let btn = with_rect(div().id(SharedString::from(format!("unanswered-{i}"))), move || key).px_3().py_1().text_size(px(12.)).cursor_pointer();
        let btn = if primary { btn.bg(c("accent")).text_color(c("on-accent")).hover(|s| s.opacity(0.9)) } else { btn.border_1().border_color(c("line")).text_color(c("text")).hover(|s| s.bg(c("raise"))) };
        acts = acts.child(btn.child(trs(&label)).on_click(cx.listener(move |this, _, _, cx| { this.go(&action, value.clone(), cx); })));
    }
    let mut body = div().flex_1().min_w_0().flex().flex_col().gap_1().px_3().py_2().child(top);
    if !n.text.is_empty() { body = body.child(div().text_size(px(12.)).text_color(c("dim")).child(format!("\u{201c}{}\u{201d}", n.text))) }
    Some(with_rect(div().id("unanswered"), || "unanswered".to_string()).w_full().flex().border_1().border_color(c("line")).bg(c("panel"))
        .child(div().w(px(3.)).flex_shrink_0().bg(c("warn"))).child(body.child(acts)).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn draws_the_hubs_notice_and_nothing_for_null() {
        let v: Value = serde_json::from_str(r#"{"head":"The agent hasn't answered your message yet","text":"Status?","since":1795108909,"msgs":["m1"],
            "buttons":[{"label":"Interrupt","action":"interrupt","value":"","primary":false},{"label":"Send again as a new turn","action":"again","value":"","primary":true}]}"#).unwrap();
        let n = parse(&v).unwrap();
        assert_eq!(n.head, "The agent hasn't answered your message yet"); assert_eq!(n.text, "Status?");
        assert_eq!(n.buttons, vec![("Interrupt".into(), "interrupt".into(), String::new(), false), ("Send again as a new turn".into(), "again".into(), String::new(), true)]);
        assert_eq!(parse(&Value::Null), None); assert_eq!(parse(&serde_json::json!({"head": ""})), None);
        let icons: serde_json::Map<String, Value> = serde_json::from_str(include_str!("../../../../assets/icons.json")).unwrap(); assert!(icons.contains_key("clock"));
    }
}

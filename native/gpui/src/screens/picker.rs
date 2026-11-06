//! The model / effort / provider picker under the composer: the hub's `thread.picker` lists (models, efforts,
//! providers) each with the one in force marked; a press is the view action `model`, `effort` or `provider` with the
//! choice's value, as on the phones. Nothing is decided here.
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Choice { pub label: String, pub value: String, pub on: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Section { pub title: &'static str, pub action: &'static str, pub choices: Vec<Choice> }

pub fn parse(picker: &Value) -> Vec<Section> {
    let list = |key: &str| -> Vec<Choice> {
        picker[key].as_array().into_iter().flatten().map(|c| Choice { label: string(c, "label"), value: string(c, "value"), on: c["on"].as_bool().unwrap_or(false) }).collect()
    };
    [("Model", "model", "models"), ("Effort", "effort", "efforts"), ("Provider", "provider", "providers")].into_iter()
        .map(|(title, action, key)| Section { title, action, choices: list(key) }).filter(|s| !s.choices.is_empty()).collect()
}
pub fn press(s: &Section, c: &Choice) -> Command { Command::Action(s.action.to_string(), c.value.clone()) }

pub fn view(cx: &mut Context<Backplane>, light: bool, picker: &Value, open: bool) -> Option<AnyElement> {
    if !open { return None; }
    let c = |n: &str| theme::color(light, n);
    let sections = parse(picker);
    if sections.is_empty() { return None; }
    let mut col = div().id("picker").max_h(px(320.)).overflow_y_scroll().flex().flex_col().border_1().border_color(c("line")).bg(c("panel")).py_1();
    for (si, s) in sections.iter().enumerate() {
        col = col.child(div().px_3().pt_1().text_size(px(11.)).text_color(c("faint")).child(s.title.to_uppercase()));
        for (ci, ch) in s.choices.iter().enumerate() {
            let (s2, c2) = (s.clone(), ch.clone());
            col = col.child(div().id(SharedString::from(format!("pick-{si}-{ci}"))).flex().gap_2().px_3().py_1().cursor_pointer().hover(|st| st.bg(c("raise")))
                .child(div().w(px(14.)).text_color(c("accent")).child(if ch.on { "✓" } else { "" })).child(ch.label.clone())
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(press(&s2, &c2)); this.screens.picker_open = false; cx.notify(); })));
        }
    }
    Some(col.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn sections_in_order_with_the_marked_choice_and_empty_ones_dropped() {
        let p: Value = serde_json::from_str(r#"{"label":"x","models":[{"label":"Opus","value":"opus","on":true},{"label":"Haiku","value":"haiku","on":false}],"efforts":[{"label":"auto","value":"","on":true}],"providers":[]}"#).unwrap();
        let s = parse(&p);
        assert_eq!(s.iter().map(|x| x.action).collect::<Vec<_>>(), ["model", "effort"]);
        assert!(s[0].choices[0].on && !s[0].choices[1].on);
        match press(&s[0], &s[0].choices[1]) { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("model", "haiku")), c => panic!("{c:?}") }
        assert!(parse(&Value::Null).is_empty());
    }
}

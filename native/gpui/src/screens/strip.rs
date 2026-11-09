//! The side-by-side notice: one line under the top bar while the hub says it is serving a copy (`sideBySide`, in /hello and the view's info).
//! Nothing is drawn without the flag. Square corners, theme tokens only.
use crate::theme;
use gpui::{prelude::*, *};
use serde_json::Value;

/// The words, exactly as agreed with the switch-over lane.
pub const TEXT: &str = /* i18n-ok: the English words; drawn through tr */ "This is a copy of your data. Agents you start here run in your real project folders: their file changes are real, and going back does not undo them. Nothing else runs by itself.";
/// The strip's height in px.
pub const H: f32 = 36.;

fn truthy(v: &Value) -> bool { match v { Value::Bool(b) => *b, Value::String(s) => matches!(s.as_str(), "1" | "true" | "on" | "yes"), Value::Number(n) => n.as_u64() == Some(1), _ => false } }
/// Whether the hub says this is a side-by-side copy: the picture's own `sideBySide`, or the view info key of that name.
pub fn side_by_side(data: &Value) -> bool { truthy(&data["sideBySide"]) || truthy(&data["info"]["sideBySide"]) }

/// The strip, or nothing.
pub fn view(light: bool, data: &Value) -> Option<AnyElement> {
    if !side_by_side(data) { return None; }
    let c = |n: &str| theme::color(light, n);
    Some(div().id("side-by-side").h(px(H)).flex_shrink_0().w_full().flex().items_center().px_3().bg(c("raise")).border_b_1().border_color(c("line")).text_size(px(11.)).line_height(px(15.)).text_color(c("text")).overflow_hidden()
        .child(crate::i18n::trs(TEXT)).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn the_strip_is_built_when_the_flag_is_true_and_not_otherwise() {
        assert!(side_by_side(&j(r#"{"sideBySide":true}"#))); assert!(side_by_side(&j(r#"{"info":{"sideBySide":"1"}}"#))); assert!(side_by_side(&j(r#"{"sideBySide":"true"}"#)));
        assert!(!side_by_side(&Value::Null)); assert!(!side_by_side(&j(r#"{"sideBySide":false}"#))); assert!(!side_by_side(&j(r#"{"info":{"sideBySide":"0"}}"#))); assert!(!side_by_side(&j(r#"{"thread":{}}"#)));
        assert!(view(false, &j(r#"{"sideBySide":true}"#)).is_some()); assert!(view(false, &Value::Null).is_none());
    }
    #[test] fn the_words_are_the_agreed_ones() {
        assert!(TEXT.starts_with("This is a copy of your data.")); assert!(TEXT.ends_with("Nothing else runs by itself.")); assert!(TEXT.contains("their file changes are real, and going back does not undo them."));
    }
}

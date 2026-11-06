//! A project's badge in the sidebar: its colour and icon as the hub names them (`project.css`, `project.icon`), and the editor the project menu opens
//! (`project.look`: every choice carries the `look-set` value it sends). The choices, the current mark and the writes are the hub's (present/badge.rs).
use crate::{art, hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

/// The icons the hub may name (its list, present/badge.rs); anything else is the folder.
const ICONS: [&str; 14] = ["folder", "circuit-board", "cpu", "zap", "radio", "box", "layers", "code", "terminal", "globe", "wrench", "flask-conical", "book", "star"];
pub fn icon_name(n: &str) -> &'static str { ICONS.iter().copied().find(|i| *i == n).unwrap_or("folder") }
/// "#rrggbb" as a colour; None for anything else.
pub fn parse_css(css: &str) -> Option<Rgba> {
    let h = css.strip_prefix('#').filter(|h| h.len() == 6)?;
    u32::from_str_radix(h, 16).ok().map(rgb)
}
/// The project's mark: its icon in its colour (the theme's `ok` while the hub names none).
pub fn mark(light: bool, project: &Value) -> AnyElement {
    art::icon(icon_name(project["icon"].as_str().unwrap_or("")), 14., parse_css(project["css"].as_str().unwrap_or("")).unwrap_or_else(|| theme::color(light, "ok")))
}
/// The editor's choices as (label, value, on, css).
pub fn choices(look: &Value, key: &str) -> Vec<(String, String, bool, String)> {
    look[key].as_array().into_iter().flatten().map(|c| (c["label"].as_str().unwrap_or("").to_string(), c["value"].as_str().unwrap_or("").to_string(), c["on"].as_bool().unwrap_or(false), c["css"].as_str().unwrap_or("").to_string())).collect()
}

/// The editor under a project's header: the colours, then the icons.
pub fn editor(cx: &mut Context<Backplane>, light: bool, pid: &str, look: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut colors = div().flex().flex_wrap().gap_1();
    for (label, value, on, css) in choices(look, "colors") {
        let fill = parse_css(&css).unwrap_or_else(|| c("dim"));
        colors = colors.child(div().id(SharedString::from(format!("look-color-{pid}-{label}"))).size(px(18.)).bg(fill).cursor_pointer().border_2().border_color(if on { c("text") } else { fill })
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("look-set".into(), value.clone())); cx.notify(); })));
    }
    let mut icons = div().flex().flex_wrap().gap_1();
    for (label, value, on, _) in choices(look, "icons") {
        icons = icons.child(div().id(SharedString::from(format!("look-icon-{pid}-{label}"))).size(px(24.)).flex().items_center().justify_center().cursor_pointer().border_1()
            .border_color(if on { c("text") } else { c("line") }).hover(|s| s.bg(c("raise")))
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("look-set".into(), value.clone())); cx.notify(); }))
            .child(art::icon(icon_name(&label), 14., c(if on { "text" } else { "dim" }))));
    }
    div().id(SharedString::from(format!("look-{pid}"))).flex().flex_col().gap_2().px_3().py_2().border_b_1().border_color(c("line")).bg(c("panel")).child(colors).child(icons).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*; use core::prelude::v1::test;
    #[test] fn the_hubs_colour_and_icon_become_a_mark_and_unknown_ones_fall_back() {
        assert!(parse_css("#e06fb4").is_some() && parse_css("e06fb4").is_none() && parse_css("#fff").is_none() && parse_css("").is_none());
        assert_eq!((icon_name("cpu"), icon_name("skull"), icon_name("")), ("cpu", "folder", "folder"));
    }
    #[test] fn the_editor_lists_what_the_hub_sends() {
        let look: Value = serde_json::from_str(r##"{"colors":[{"label":"blue","css":"#5b9cf5","on":true,"value":"project.color.p=blue"}],"icons":[{"label":"cpu","on":false,"value":"project.icon.p=cpu"}]}"##).unwrap();
        assert_eq!(choices(&look, "colors"), [("blue".to_string(), "project.color.p=blue".to_string(), true, "#5b9cf5".to_string())]);
        assert_eq!(choices(&look, "icons")[0].1, "project.icon.p=cpu"); assert!(choices(&Value::Null, "colors").is_empty());
    }
}

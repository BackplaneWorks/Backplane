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

/// A thread's machine marker as the hub gives it (`mark` {name, short, away}; none on this machine's threads): the text drawn (`short` on
/// a row, `name` in the header; an away machine's says so), its opacity (dimmer when away) and the hover text (the full name).
pub fn machine_text(mark: &Value, full: bool) -> Option<(String, f32, String)> {
    let name = mark["name"].as_str().filter(|n| !n.is_empty())?;
    let away = mark["away"].as_bool().unwrap_or(false);
    let t = if full { name } else { mark["short"].as_str().filter(|s| !s.is_empty()).unwrap_or(name) };
    Some((if away && full { format!("{t} · away") } else { t.to_string() }, if away { 0.5 } else { 1.0 }, if away { format!("{name} · away") } else { name.to_string() }))
}
/// The marker drawn: dim text, small, quieter than the state dots; the full name on hover. `id` makes the hover's element unique.
pub fn machine_mark(light: bool, mark: &Value, full: bool, id: &str) -> Option<AnyElement> {
    let (t, o, hover) = machine_text(mark, full)?;
    Some(div().id(SharedString::from(format!("mark-{id}"))).flex_shrink_0().max_w(px(if full { 200. } else { 80. })).overflow_hidden().text_ellipsis().whitespace_nowrap()
        .text_size(px(11.)).text_color(theme::color(light, "faint")).opacity(o).tooltip(crate::keys::tip(&hover, "", light)).child(t).into_any_element())
}
/// Where a project or thread is when it is another machine's, as the hub words it (`mark.label` "on desktop"; a project of this machine that
/// is also on others: its `also` "also on desktop"): the text, its opacity (dimmer when away) and the hover (the full name). None: this machine's, drawn as always.
pub fn place_text(mark: &Value, also: &str) -> Option<(String, f32, String)> {
    if mark["name"].as_str().map_or(true, str::is_empty) { return (!also.is_empty()).then(|| (also.to_string(), 1.0, also.to_string())) }
    let (_, o, hover) = machine_text(mark, true)?;
    let l = mark["label"].as_str().filter(|l| !l.is_empty()).map_or_else(|| format!("on {}", mark["short"].as_str().unwrap_or("")), str::to_string);
    Some((if o < 1.0 { format!("{l} · away") } else { l }, o, hover))
}
/// The one place mark: the link icon and the words, quiet (the `faint` text of either theme), the full name on hover. `id` keeps the hover unique.
pub fn place(light: bool, mark: &Value, also: &str, id: &str) -> Option<AnyElement> {
    let (t, o, hover) = place_text(mark, also)?;
    let col = theme::color(light, "dim");
    Some(div().id(SharedString::from(format!("place-{id}"))).flex_shrink_0().flex().items_center().gap(px(3.)).max_w(px(220.)).overflow_hidden().whitespace_nowrap()
        .text_size(px(11.)).text_color(col).opacity(o).tooltip(crate::keys::tip(&hover, "", light))
        .child(art::icon("link", 11., col)).child(div().overflow_hidden().text_ellipsis().child(t)).into_any_element())
}
/// The note under the composer of another machine's thread: where an agent sent from here runs (`mark.runs` "runs on desktop"). None for this machine's.
pub fn runs_text(mark: &Value) -> Option<String> {
    mark["name"].as_str().filter(|n| !n.is_empty())?;
    Some(mark["runs"].as_str().filter(|r| !r.is_empty()).map_or_else(|| format!("runs on {}", mark["short"].as_str().unwrap_or("")), str::to_string))
}
/// A page header's source on another machine: "desktop: boards/x.kicad_pcb"; this machine's is just what it shows.
pub fn from_machine(mark: &Value, what: &str) -> String {
    match mark["short"].as_str().filter(|s| !s.is_empty() && !mark["name"].as_str().unwrap_or("").is_empty()) { Some(m) if what.is_empty() => m.to_string(), Some(m) => format!("{m}: {what}"), None => what.to_string() }
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
    #[test] fn a_machine_marker_is_short_on_rows_full_in_the_header_and_dim_when_away() {
        let m = serde_json::json!({"name":"framework-1.tail.ts.net","short":"framework-1","away":false});
        assert_eq!(machine_text(&m, false), Some(("framework-1".into(), 1.0, "framework-1.tail.ts.net".into())));
        assert_eq!(machine_text(&m, true), Some(("framework-1.tail.ts.net".into(), 1.0, "framework-1.tail.ts.net".into())));
        let a = serde_json::json!({"name":"desk","short":"desk","away":true});
        assert_eq!(machine_text(&a, false), Some(("desk".into(), 0.5, "desk · away".into()))); assert_eq!(machine_text(&a, true).unwrap().0, "desk · away");
        assert_eq!(machine_text(&Value::Null, false), None, "this machine's threads look as they always did");
    }
    #[test] fn a_place_reads_the_same_everywhere_and_a_local_one_says_nothing() {
        let m = serde_json::json!({"name":"desktop.tail.ts.net","short":"desktop","away":false,"label":"on desktop","runs":"runs on desktop"});
        assert_eq!(place_text(&m, ""), Some(("on desktop".into(), 1.0, "desktop.tail.ts.net".into())));
        let a = serde_json::json!({"name":"desktop","short":"desktop","away":true,"label":"on desktop","runs":"runs on desktop"});
        assert_eq!(place_text(&a, "").map(|p| (p.0, p.1)), Some(("on desktop · away".into(), 0.5)));
        assert_eq!(place_text(&Value::Null, "also on desktop").unwrap().0, "also on desktop", "a merged project says where else it is");
        assert_eq!(place_text(&Value::Null, ""), None, "a project of this machine looks as it always did");
        assert_eq!((runs_text(&m).as_deref(), runs_text(&Value::Null)), (Some("runs on desktop"), None));
        assert_eq!((from_machine(&m, "boards/x.kicad_pcb"), from_machine(&Value::Null, "boards/x.kicad_pcb")), ("desktop: boards/x.kicad_pcb".to_string(), "boards/x.kicad_pcb".to_string()));
        assert_eq!(from_machine(&m, ""), "desktop");
    }
}

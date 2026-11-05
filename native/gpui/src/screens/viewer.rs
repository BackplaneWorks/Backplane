//! The viewer panel beside the thread: the tabs the hub lists (`thread.viewer.choices`) and the page it has open.
//! Files (folder rows), Diff (the thread's changes) and Review (the report as Markdown, page chips) are drawn here from
//! the hub's rows; every press is the view action the row names. The board, schematic and 3D pages draw plot chunks
//! (screens::plot, next). Pure parsing below, tested; the element code only lays the rows out.
use super::{md, timeline::{self, Reg}};
use crate::{hub::Command, string, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Tab { pub label: String, pub value: String, pub on: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Crumb { pub label: String, pub action: String, pub value: String }
#[derive(Clone, Debug, PartialEq)]
pub struct FileRow { pub label: String, pub action: String, pub value: String, pub detail: String, pub kind: String }
#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine { pub k: u32, pub t: String, pub old: String, pub new: String }
#[derive(Clone, Debug, PartialEq)]
pub struct DiffFile { pub name: String, pub status: String, pub lines: Vec<DiffLine> }

pub const MAX_ROWS: usize = 1000;

/// Open a viewer kind, or close the viewer with "": the phones' idempotent `view` action. The client never toggles the panel and never
/// guesses its state: pressing twice before the reply asks for the same thing twice.
pub fn view_kind(kind: &str) -> Command { Command::Action("view".into(), kind.to_string()) }
pub fn close() -> Command { view_kind("") }

fn arr<'a>(v: &'a Value, k: &str) -> impl Iterator<Item = &'a Value> { v[k].as_array().into_iter().flatten() }
pub fn tabs(viewer: &Value) -> Vec<Tab> { arr(viewer, "choices").map(|c| Tab { label: string(c, "label"), value: string(c, "value"), on: c["on"].as_bool().unwrap_or(false) }).collect() }
pub fn crumbs(files: &Value) -> Vec<Crumb> { arr(files, "trail").map(|c| Crumb { label: string(c, "label"), action: string(c, "action"), value: string(c, "value") }).collect() }
pub fn file_rows(files: &Value) -> Vec<FileRow> { arr(files, "rows").map(|r| FileRow { label: string(r, "label"), action: string(r, "action"), value: string(r, "value"), detail: string(r, "detail"), kind: string(r, "kind") }).collect() }
pub fn diff_files(diff: &Value) -> Vec<DiffFile> {
    arr(diff, "files").map(|f| DiffFile { name: string(f, "name"), status: string(f, "status"),
        lines: arr(f, "lines").map(|l| DiffLine { k: l["k"].as_u64().unwrap_or(0) as u32, t: string(l, "t"), old: string(l, "o"), new: string(l, "n") }).collect() }).collect()
}
/// The sign and the kind of colour a diff line takes: 1 added, 2 removed, 4 a hunk header, the rest context.
pub fn diff_sign(k: u32) -> &'static str { match k { 1 => "+", 2 => "-", _ => " " } }

/// The kind the viewer shows: the hub's marked tab, or the one just pressed until the hub answers.
pub fn active_kind(viewer: &Value, pending: Option<&str>) -> String {
    pending.map(str::to_string).unwrap_or_else(|| tabs(viewer).into_iter().find(|t| t.on).map(|t| t.value).unwrap_or_default())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Part { pub label: String, pub value: String, pub on: bool }
/// The Mechanical page's parts (`viewer.mech.parts`): a press is `mech-part value`.
pub fn mech_parts(viewer: &Value) -> Vec<Part> { arr(&viewer["mech"], "parts").map(|p| Part { label: string(p, "label"), value: string(p, "value"), on: p["on"].as_bool().unwrap_or(false) }).collect() }

/// The 3D page may be shown only when the model it holds is the source the viewer asks for (`viewer.key`): one page serves every thread, so a
/// switched thread or a new part waits for its own frame instead of showing the last model.
pub fn solid_ready(want: &str, have: &str) -> bool { want.is_empty() || want == have }

/// Whether the panel is open: a viewer page is on, or the thread has a diff.
pub fn is_open(thread: &Value) -> bool { thread["diff"].is_object() || thread["viewer"]["open"].as_str().map_or(false, |o| !o.is_empty()) }

fn btn(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, cmd: Command) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = div().id(SharedString::from(id)).px_2().py_1().text_size(px(12.)).cursor_pointer().border_1();
    let d = if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
    let cmd = std::cell::RefCell::new(Some(cmd));
    d.child(label).on_click(cx.listener(move |this, _, _, cx| { if let Some(Command::Action(a, v)) = cmd.borrow().as_ref().map(|c| match c { Command::Action(a, v) => Command::Action(a.clone(), v.clone()), _ => Command::Page(String::new()) }) { let _ = this.tx.try_send(Command::Action(a, v)); } cx.notify(); })).into_any_element()
}

/// Mech: a chip per part (and Renders) over the part's 3D; with no part chosen the page says what there is (or why not) and lists them.
fn mech_chrome(cx: &mut Context<Backplane>, light: bool, viewer: &Value, key_empty: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mech = &viewer["mech"];
    let mut col = div().flex().flex_col().gap_1().px_3().py_2().border_b_1().border_color(c("line"));
    let parts = mech_parts(viewer);
    if key_empty {
        let say = [string(mech, "say"), string(mech, "empty"), string(mech, "note")].into_iter().find(|s| !s.is_empty());
        if let Some(s) = say { col = col.child(div().text_size(px(12.)).text_color(c("dim")).child(s)); }
    }
    let mut row = div().flex().flex_wrap().gap_2();
    for (i, p) in parts.iter().enumerate() { row = row.child(btn(cx, light, format!("mech-part-{i}"), p.label.clone(), p.on, Command::Action("mech-part".into(), p.value.clone()))); }
    // (no Renders button: the renders page is not drawn here yet, and an offered button must lead somewhere)
    row = row.child(btn(cx, light, "mech-refresh".into(), "Refresh".into(), false, Command::Action("mech-refresh".into(), String::new())));
    col.child(row).into_any_element()
}

fn files_page(cx: &mut Context<Backplane>, light: bool, files: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col();
    let trail = crumbs(files);
    col = col.child(div().flex().flex_wrap().gap_1().px_3().py_2().text_size(px(12.)).children(trail.into_iter().enumerate().map(|(i, cr)| {
        let (a, v) = (cr.action.clone(), cr.value.clone());
        div().id(SharedString::from(format!("crumb-{i}"))).px_1().cursor_pointer().text_color(c("accent")).hover(|s| s.bg(c("raise"))).child(format!("{} /", cr.label))
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); })).into_any_element()
    }).collect::<Vec<_>>()));
    let rows = file_rows(files);
    for (i, r) in rows.iter().take(MAX_ROWS).enumerate() {
        let (a, v) = (r.action.clone(), r.value.clone());
        col = col.child(div().id(SharedString::from(format!("file-{i}"))).flex().gap_3().px_3().py_1().cursor_pointer().hover(|s| s.bg(c("raise")))
            .child(div().w(px(14.)).text_color(c(if r.kind == "folder" { "ok" } else { "dim" })).child(if r.kind == "folder" { "▸" } else { "·" }))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().child(r.label.clone()))
            .child(div().text_size(px(11.)).text_color(c("dim")).child(r.detail.clone()))
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); })));
    }
    if rows.len() > MAX_ROWS { col = col.child(div().px_3().py_1().text_color(c("dim")).child(format!("… {} more", rows.len() - MAX_ROWS))); }
    if rows.is_empty() { col = col.child(div().px_3().py_2().text_color(c("dim")).child(if files["ready"] == false { "Loading…" } else { "Empty folder" })); }
    col.into_any_element()
}

fn diff_page(cx: &mut Context<Backplane>, light: bool, diff: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().text_size(px(12.));
    col = col.child(div().flex().gap_2().px_3().py_2().items_center().child(div().flex_1().text_color(c("dim")).child(string(diff, "summary")))
        .children([("Commit", "commit_push"), ("PR", "commit_push_pr")].into_iter().map(|(l, v)| btn(cx, light, format!("git-{v}"), l.to_string(), false, Command::Action("git".into(), v.into()))).collect::<Vec<_>>()));
    let mut budget = MAX_ROWS;
    for f in diff_files(diff) {
        if budget == 0 { break; }
        col = col.child(div().flex().gap_2().px_3().py_1().mt_1().bg(c("raise")).border_y_1().border_color(c("line")).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(f.name.clone())).child(div().text_color(c("dim")).child(f.status.clone())));
        for l in &f.lines {
            if budget == 0 { break; } budget -= 1;
            let bg = match l.k { 1 => Some(theme::color(light, "ok")), 2 => Some(theme::color(light, "bad")), _ => None };
            let mut row = div().flex().gap_2().px_3().font(crate::art::mono_font());
            if let Some(mut b) = bg { b.a = 0.14; row = row.bg(b); }
            if l.k == 4 { row = row.text_color(c("accent")); }
            col = col.child(row.child(div().w(px(34.)).text_color(c("faint")).child(if l.k == 2 { l.old.clone() } else { l.new.clone() })).child(div().w(px(10.)).child(diff_sign(l.k))).child(div().flex_1().whitespace_nowrap().child(l.t.clone())));
        }
    }
    if budget == 0 { col = col.child(div().px_3().py_1().text_color(c("dim")).child("… the rest of the diff is not shown")); }
    col.into_any_element()
}

/// Board or schematic: the plot the hub sent for this file, or what it says instead, with the hub's layer list beside it.
fn plot_page(cx: &mut Context<Backplane>, light: bool, open: &str, viewer: &Value, plot: &super::plot::Store, pv: &mut super::plotview::PlotView) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let Some(frame) = plot.frame.as_ref() else { return div().p_3().text_color(c("dim")).child(if open == "board" { "Loading the board…" } else { "Loading the schematic…" }).into_any_element() };
    // the frame must be the source this viewer asks for: another project's board stays off the screen until its own plot arrives
    let want = string(viewer, "key");
    if !want.is_empty() && !frame.key.is_empty() && frame.key != want { return div().p_3().text_color(c("dim")).child(if open == "board" { "Loading the board…" } else { "Loading the schematic…" }).into_any_element(); }
    if !frame.none.is_empty() { return div().p_3().text_color(c("dim")).child(frame.none.clone()).into_any_element(); }
    let (w, h) = pv.bounds.map_or((480., 600.), |b| (f32::from(b.size.width), f32::from(b.size.height)));
    let view = super::plotview::ensure_view(pv, frame, w, h);
    let mut col = div().size_full().flex().flex_col();
    let note = string(viewer, "note");
    if !note.is_empty() { col = col.child(div().px_3().py_1().text_size(px(11.)).text_color(c("dim")).child(note)); }
    // Layers: the menu lists the layers this plot has; the hub's masks decide what the canvas draws
    let present: std::collections::BTreeSet<u32> = frame.chunks.iter().map(|c| c.layer).collect();
    let strip = crate::viewers::layers::strip(viewer, &present);
    col = col.child(crate::viewers::layers::strip_el(cx, light, &strip, pv.layers_open));
    let mut area = div().relative().flex().flex_col().flex_1().min_h_0().child(super::plotview::canvas_el(cx, light, frame, view, crate::viewers::layers::masks(viewer)));
    if let Some(p) = crate::viewers::layers::panel_el(cx, light, &strip, pv.layers_open) { area = area.child(p); }
    col.child(area).into_any_element()
}

fn review_page(cx: &mut Context<Backplane>, reg: &Reg, light: bool, review: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().gap_2().p_3();
    col = col.child(div().flex().flex_wrap().gap_2().children(arr(review, "tabs").enumerate().map(|(i, t)| btn(cx, light, format!("rv-{i}"), string(t, "label"), t["on"].as_bool().unwrap_or(false), Command::Action("rv-page".into(), string(t, "page")))).collect::<Vec<_>>()));
    let title = string(review, "title");
    if !title.is_empty() { col = col.child(div().font_weight(FontWeight::BOLD).text_size(px(15.)).child(title)); }
    let blocks: Vec<Value> = review["blocks"].as_array().cloned().unwrap_or_default();
    if blocks.is_empty() { col = col.child(div().text_color(c("dim")).child(if review["busy"] == true { "Working…".to_string() } else { string(review, "note") })); }
    else { let parsed = md::parse(&blocks); for el in timeline::blocks(cx, reg, light, &parsed, "review") { col = col.child(el); } }
    col.into_any_element()
}

/// The panel (None while nothing is open). 520 px wide, between the thread and the rail.
pub fn view(cx: &mut Context<Backplane>, reg: &Reg, light: bool, thread: &Value, plot: &super::plot::Store, pv: &mut super::plotview::PlotView, solid: Option<&Entity<crate::viewers::solid::SolidPage>>, browser: Option<&Entity<crate::viewers::browser::BrowserPage>>) -> Option<AnyElement> {
    if !is_open(thread) { return None; }
    let c = |n: &str| theme::color(light, n);
    let viewer = &thread["viewer"];
    let open = viewer["open"].as_str().unwrap_or("");
    let diff_on = thread["diff"].is_object();
    let mut bar = div().h(px(34.)).flex_shrink_0().flex().items_center().gap_1().px_2().border_b_1().border_color(c("line"));
    for (i, t) in tabs(viewer).into_iter().enumerate() { bar = bar.child(btn(cx, light, format!("vtab-{i}"), t.label, t.on && !diff_on, view_kind(&t.value))); }
    bar = bar.child(div().flex_1()).child(div().id("viewer-close").px_2().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("text"))).child("Close")
        .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(close()); cx.notify(); })));
    let body: AnyElement = if diff_on { diff_page(cx, light, &thread["diff"]) }
        else if open == "files" && viewer["files"].is_object() { files_page(cx, light, &viewer["files"]) }
        else if open == "review" && viewer["review"].is_object() { review_page(cx, reg, light, &viewer["review"]) }
        else if matches!(open, "board" | "schematic") { plot_page(cx, light, open, viewer, plot, pv) }
        else if matches!(open, "3d" | "mech") {
            let key_empty = string(viewer, "key").is_empty();
            let want = string(viewer, "key");
            // what the page HOLDS, not what was queued for it: a big model's decode, a frame that fails or is replaced in the worker's slot
            let have = solid.map(|s| s.read(cx).key().to_string()).unwrap_or_default();
            let model: AnyElement = if open == "mech" && key_empty { div().into_any_element() } else { match solid { Some(s) if solid_ready(&want, &have) => div().flex_1().min_h_0().child(s.clone()).into_any_element(), _ => div().p_3().text_color(c("dim")).child("Loading the model…").into_any_element() } };
            if open == "mech" { div().size_full().flex().flex_col().child(mech_chrome(cx, light, viewer, key_empty)).child(model).into_any_element() } else { div().size_full().flex().flex_col().child(model).into_any_element() }
        }
        else if open == "browser" { match browser { Some(b) => div().size_full().child(b.clone()).into_any_element(), None => div().p_3().text_color(c("dim")).child("The browser page needs a hub on plain http://.").into_any_element() } }
        else { div().p_3().text_color(c("dim")).child(match open { "split" => "Split is not drawn here yet", _ => "Opening…" }).into_any_element() };
    Some(div().w(px(520.)).h_full().flex_shrink_0().border_l_1().border_color(c("line")).flex().flex_col()
        .child(bar).child(div().id("viewer-body").flex_1().min_h_0().overflow_y_scroll().child(body)).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn tabs_files_and_diff_parse_from_the_hubs_rows() {
        let v = j(r#"{"open":"files","choices":[{"label":"Board","value":"board","on":false},{"label":"Files","value":"files","on":true}],
            "files":{"trail":[{"label":"project","action":"fm-cd","value":""}],"rows":[{"label":"src","action":"fm-cd","value":"src","detail":"","kind":"folder","icon":"folder"},{"label":"a.rs","action":"fm-open","value":"a.rs","detail":"1 KB","kind":"text","icon":"file"}],"ready":true}}"#);
        assert_eq!(tabs(&v).iter().map(|t| (t.value.as_str(), t.on)).collect::<Vec<_>>(), [("board", false), ("files", true)]);
        assert_eq!(crumbs(&v["files"])[0].action, "fm-cd");
        let r = file_rows(&v["files"]); assert_eq!((r[0].kind.as_str(), r[1].detail.as_str()), ("folder", "1 KB"));
        let d = diff_files(&j(r#"{"summary":"1 file","files":[{"name":"a.rs","status":"M","lines":[{"k":4,"t":"@@ -1 +1 @@","o":"","n":""},{"k":2,"t":"old","o":"1","n":""},{"k":1,"t":"new","o":"","n":"1"}]}]}"#));
        assert_eq!(d[0].lines.len(), 3); assert_eq!((diff_sign(d[0].lines[1].k), diff_sign(d[0].lines[2].k), diff_sign(4)), ("-", "+", " "));
    }
    #[test] fn opening_and_closing_are_the_idempotent_view_action_never_a_toggle() {
        for c in [view_kind("board"), view_kind("board"), view_kind("schematic")] { match c { Command::Action(a, v) => { assert_eq!(a, "view"); assert!(v == "board" || v == "schematic"); } c => panic!("{c:?}") } }
        match close() { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("view", "")), c => panic!("{c:?}") }
    }
    #[test] fn the_3d_page_waits_for_its_own_frame() {
        assert!(solid_ready("2|/a|", "2|/a|")); assert!(!solid_ready("2|/b|", "2|/a|"), "another project's model stays off the screen");
        assert!(!solid_ready("2|/a|mech/new.step", "2|/a|mech/old.step"), "a new part waits for its frame"); assert!(solid_ready("", ""));
        // a frame accepted but not decoded yet: the page still holds nothing (or the old key), so it is not ready
        assert!(!solid_ready("2|/a|", "")); assert!(!solid_ready("2|/b|", "2|/a|"));
    }
    #[test] fn mech_parts_parse_from_the_pages_choices() {
        let v = j(r#"{"mech":{"parts":[{"label":"bracket","value":"mech/bracket.step","on":true},{"label":"lid","value":"mech/lid.step","on":false}],"say":"","rel":"mech/bracket.step"}}"#);
        let p = mech_parts(&v); assert_eq!((p.len(), p[0].value.as_str(), p[0].on, p[1].on), (2, "mech/bracket.step", true, false)); assert!(mech_parts(&Value::Null).is_empty());
    }
    #[test] fn the_pressed_kind_shows_at_once_then_the_hubs_mark_does() {
        let v = j(r#"{"choices":[{"label":"Board","value":"board","on":false},{"label":"Files","value":"files","on":true}]}"#);
        assert_eq!(active_kind(&v, None), "files"); assert_eq!(active_kind(&v, Some("board")), "board"); assert_eq!(active_kind(&Value::Null, None), "");
    }
    #[test] fn the_panel_is_open_for_a_page_or_a_diff_only() {
        assert!(is_open(&j(r#"{"viewer":{"open":"board"}}"#))); assert!(is_open(&j(r#"{"diff":{"summary":"","files":[]}}"#)));
        assert!(!is_open(&j(r#"{"viewer":{"open":""},"diff":null}"#))); assert!(!is_open(&Value::Null));
    }
}

//! The window: layout and colours follow the web client (src/web/style.css tokens).
use crate::script::{Ctl, CtlEntry};
use crate::{art, hub::Command, string, theme, Backplane, Send};
use crate::i18n::{tf, trs};
use gpui::{prelude::*, *};
use serde_json::Value;

#[derive(Default)]
pub(crate) struct RailRow<'a> {
    pub id: &'a str, pub label: &'a str, icon: &'a str, group: &'a str, pub key: &'a str,
    pub state: &'a str, why: &'a str,
}

pub(crate) fn rail_rows(viewer: &Value, simulation: bool) -> Vec<RailRow<'_>> {
    if let Some(rows) = viewer["rail"].as_array() {
        return rows.iter().filter(|r| simulation || r["id"] != "simulation").map(|row| {
            let field = |name: &str| row[name].as_str().unwrap_or("");
            RailRow { id: field("id"), label: field("label"), icon: field("icon"), group: field("group"),
                key: field("key"), state: field("state"), why: field("why") }
        }).collect();
    }
    // Older hubs send choices and simFrame. Keep their existing rail and grouping.
    [("board", "Board", "circuit-board", "design"), ("schematic", "Schematic", "cpu", "design"), // i18n-ok: fallback labels drawn through trs
     ("3d", "3D", "box", "design"), ("gerbers", "Gerbers", "layers", "design"), // i18n-ok: fallback labels drawn through trs
     ("simulation", "Simulation", "gauge", "design"), ("split", "Split", "layout-panel-left", "layout"), // i18n-ok: fallback labels drawn through trs
     ("mech", "Mech", "wrench", "layout"), ("files", "Files", "folder", "work"), ("diff", "Diff", "diff", "work"), // i18n-ok: fallback labels drawn through trs
     ("review", "Review", "circle-check", "work"), ("browser", "Browser", "globe", "work"), // i18n-ok: fallback labels drawn through trs
     ("terminal", "Terminal", "terminal", "tools"), ("commands", "Commands", "search", "tools")] // i18n-ok: fallback labels drawn through trs
        .into_iter().filter(|(id, ..)| *id != "simulation" || simulation)
        .map(|(id, label, icon, group)| RailRow { id, label, icon, group, key: crate::keys::rail_letter(id), ..Default::default() }).collect()
}

pub(crate) fn rail_hint(local: &crate::local::Local, id: &str, key: &str) -> String {
    let binding = match id { "diff" | "files" => id, "terminal" => "term", "commands" => "palette", _ => "" };
    [key.to_uppercase(), crate::keys::hint(local, binding)].into_iter().filter(|k| !k.is_empty()).collect::<Vec<_>>().join(&format!(" {} ", trs("or")))
}

// The footer owns its rows and separator. Only the pages scroll in the remaining height.
fn rail_layout(height: Pixels, row: Pixels, tools: usize) -> (Pixels, Pixels) {
    let footer = if tools == 0 { px(0.) } else { (row * tools as f32 + px(1.)).min(height) };
    (height - footer, footer)
}

// Distances are measured down from the top of the scroll content, including group gaps.
fn reveal_rail_row(offset: Pixels, top: Pixels, row: Pixels, viewport: Pixels) -> Pixels {
    (top + row - viewport).max(offset).min(top).max(px(0.))
}

fn rail_sections(scroll: ScrollHandle, reveal: Option<Pixels>, pages: Vec<AnyElement>, tools: Vec<AnyElement>, rule: Rgba) -> impl IntoElement {
    canvas(move |bounds, window, cx| {
        let (page_h, footer_h) = rail_layout(bounds.size.height, crate::scale::tpx(48.), tools.len());
        if let Some(top) = reveal {
            let old = scroll.offset();
            let y = reveal_rail_row(-old.y, top, crate::scale::tpx(48.), page_h);
            scroll.set_offset(point(old.x, -y));
        }
        let mut sections = div().w(bounds.size.width).h(bounds.size.height).flex().flex_col()
            .child(div().id("rail-pages").h(page_h).overflow_y_scroll().track_scroll(&scroll).flex().flex_col().children(pages))
            .when(!tools.is_empty(), |d| d.child(div().h(footer_h).flex_shrink_0().border_t_1().border_color(rule).flex().flex_col().children(tools)))
            .into_any_element();
        sections.prepaint_as_root(bounds.origin, bounds.size.map(AvailableSpace::Definite), window, cx);
        sections
    }, |_, mut sections, window, cx| sections.paint(window, cx)).size_full()
}

pub(crate) fn rail_enabled(viewer: &Value, id: &str) -> bool {
    viewer["rail"].as_array().is_none_or(|rows| rows.iter().any(|r| r["id"] == id && r["state"] != "disabled"))
}

/// The rail entry lit for an embedded KiCad pane's wanted frame (`KicadPane::tab()`, which can be
/// ahead of KiCad while `pending_show` is set). Symbols and Footprints light their owning design
/// entry (Schematic and Board); Simulator lights Simulation. Pure mapping: never calls back into the pane.
fn kicad_rail_id(frame: &str) -> Option<&'static str> {
    match frame {
        "PCB" | "FOOTPRINT_EDITOR" => Some("board"), // i18n-ok: wire frame kind enum names
        "SCH" | "SYMBOL_EDITOR" => Some("schematic"), // i18n-ok: wire frame kind enum names
        "PCB_3D" => Some("3d"), // i18n-ok: wire frame kind enum names
        "GERBVIEW" => Some("gerbers"), // i18n-ok: wire frame kind enum names
        "SIMULATOR" => Some("simulation"), // i18n-ok: wire frame kind enum names
        _ => None,
    }
}

/// The rail entry lit by the embedded KiCad pane's wanted frame (`KicadPane::tab()`, which can be
/// ahead of KiCad while `pending_show` is set), or `None` when the pane does not govern the rail:
/// requires a local thread whose viewer holds the editor (not covered by Diff), and that the mapped
/// entry exists and is enabled in the drawn `rows` (e.g. GerbView with no Gerbers row does not
/// suppress the hub's lit row and light nothing).
fn kicad_lit_id(rows: &[RailRow], local: bool, holds_editor: bool, diff_on: bool, pane_frame: Option<&str>) -> Option<&'static str> {
    if !local || !holds_editor || diff_on { return None; }
    let id = kicad_rail_id(pane_frame?)?;
    rows.iter().any(|r| r.id == id && r.state != "disabled").then_some(id)
}

/// Whether a viewer rail row is lit. An optimistic rail press (`pending`, including `""` when closing
/// the viewer) leads while set; otherwise when the local embedded KiCad pane is on show (`kicad_on`,
/// mapped from `KicadPane::tab()` / `wanted_kind()` to an enabled drawn row), its wanted frame leads
/// without waiting on a hub round trip; otherwise the hub's `row.state` (or `active_kind` on older hubs) leads.
fn viewer_row_on(row: &RailRow, kicad_on: Option<&str>, pending: Option<&str>, active_kind: &str) -> bool {
    if let Some(pending) = pending { return pending == row.id; }
    if let Some(id) = kicad_on { return id == row.id; }
    if row.state.is_empty() { active_kind == row.id } else { row.state == "on" }
}

/// Whether `BACKPLANE_GPUI_RECTS` is enabled for the process (checked once; off in normal use).
pub(crate) fn rail_rects_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("BACKPLANE_GPUI_RECTS").is_some_and(|v| !v.is_empty() && v != "0"))
}

/// Snapshot of what the right rail draws (`lit`, `entries`, `disabled` using the `rail:<Label>`
/// labels), returning `Some(snapshot)` on first paint and whenever it changes from `last`.
fn rail_metric_change(last: &mut Option<Value>, lit: &[&str], entries: &[&str], disabled: &[&str]) -> Option<Value> {
    let now = serde_json::json!({ "lit": lit, "entries": entries, "disabled": disabled });
    if last.as_ref() == Some(&now) { return None; }
    *last = Some(now.clone());
    Some(now)
}

/// The viewer rail entry currently shown lit for `thread`, using the exact same priority order as
/// `viewer_row_on` so pressing a lit entry (e.g. Schematic after a KiCad strip switch from Board)
/// shuts the viewer consistently.
pub(crate) fn rail_lit_id(thread: &Value, local: bool, pane_frame: Option<&str>, pending: Option<&str>) -> Option<String> {
    let diff_on = thread["diff"].is_object();
    let sim_rail = crate::viewers::kicad_pane::rail_simulation(&thread["viewer"], local).is_some();
    let rows = rail_rows(&thread["viewer"], sim_rail);
    let kicad_on = kicad_lit_id(&rows, local, crate::screens::viewer::holds_editor(thread), diff_on, pane_frame);
    let active_kind = if diff_on { "diff".to_owned() } else { crate::screens::viewer::active_kind(&thread["viewer"], pending) };
    rows.into_iter()
        .find(|r| r.id != "terminal" && r.state != "disabled" && viewer_row_on(r, kicad_on, pending, &active_kind))
        .map(|r| r.id.to_string())
}

fn pager_row(cx: &mut Context<Backplane>, id: String, pager: &Value, light: bool, ctl: &mut Vec<CtlEntry>) -> Option<AnyElement> {
    let c = |name: &str| theme::color(light, name);
    let mut row = div().flex().gap_2().px_3().py_1().text_size(crate::scale::tpx(11.));
    let mut any = false;
    /* i18n-ok: the ctl labels; drawn through trs below */ for (name, key) in [("Newer", "previous"), ("Older", "next")] {
        if let Some(value) = pager[key]["value"].as_str() {
            any = true;
            let value = value.to_string();
            ctl.push(CtlEntry { id: format!("{id}-{key}"), label: name.to_string(), ctl: Ctl::Page(value.clone()) });
            row = row.child(with_rect(div().id(SharedString::from(format!("{id}-{key}"))), || format!("{id}-{key}")).cursor_pointer().text_color(c("accent")).hover(|s| s.bg(c("raise"))).px_1()
                .child(format!("{} ({})", trs(name), pager["total"]))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Page(value.clone())); cx.notify(); })));
        }
    }
    if any { Some(row.into_any_element()) } else { None }
}

fn inline_text(node: &Value, out: &mut String) {
    if let Some(t) = node["text"].as_str() { out.push_str(t); }
    for k in node["kids"].as_array().into_iter().flatten() { inline_text(k, out); }
}

pub(crate) fn block(node: &Value, light: bool, depth: usize) -> AnyElement {
    let c = |name: &str| theme::color(light, name);
    let tag = node["tag"].as_str().unwrap_or("p");
    let mut text = String::new();
    match tag {
        "ul" | "ol" => {
            let mut list = div().flex().flex_col().gap_1().pl(px(16.));
            for (i, item) in node["kids"].as_array().into_iter().flatten().enumerate() {
                let mut t = String::new(); inline_text(item, &mut t);
                let mark = if tag == "ol" { format!("{}.", i + 1) } else { "•".to_string() };
                list = list.child(div().flex().gap_2().child(div().text_color(c("dim")).child(mark)).child(div().flex_1().child(t)));
            }
            list.into_any_element()
        }
        "pre" => { inline_text(node, &mut text); div().font(art::mono_font()).w_full().bg(c("raise")).border_1().border_color(c("line")).p_2().text_size(crate::scale::tpx(12.)).child(text).into_any_element() }
        t if t.starts_with('h') && t.len() == 2 => { inline_text(node, &mut text); div().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(15.)).child(text).into_any_element() }
        "blockquote" => {
            let mut q = div().border_l_2().border_color(c("line")).pl_3().text_color(c("dim"));
            if depth < 4 { for k in node["kids"].as_array().into_iter().flatten() { q = q.child(block(k, light, depth + 1)); } }
            q.into_any_element()
        }
        _ => { inline_text(node, &mut text); div().child(text).into_any_element() }
    }
}

/// The hub's project-home cards: generic nodes {tag, className, label, action, payload, kids|text}.
fn node_text(n: &Value, out: &mut String) {
    if let Some(t) = n["text"].as_str() { out.push_str(t); }
    for k in n["kids"].as_array().into_iter().flatten() { node_text(k, out); }
}
fn node_el(cx: &mut Context<Backplane>, n: &Value, light: bool, depth: usize, path: &str, ctl: &mut Vec<CtlEntry>) -> AnyElement {
    let c = |name: &str| theme::color(light, name);
    if let Some(t) = n["text"].as_str() { return div().child(trs(t)).into_any_element(); }
    if depth > 14 { return div().into_any_element(); }
    let tag = n["tag"].as_str().unwrap_or("div");
    let class = n["className"].as_str().unwrap_or("");
    let kids = |cx: &mut Context<Backplane>, ctl: &mut Vec<CtlEntry>| -> Vec<AnyElement> {
        n["kids"].as_array().into_iter().flatten().take(200).enumerate().map(|(i, k)| node_el(cx, k, light, depth + 1, &format!("{path}.{i}"), ctl)).collect()
    };
    match tag {
        "svg" | "path" | "img" => div().into_any_element(),
        "button" => {
            let (action, payload) = (string(n, "action"), string(n, "payload"));
            let mut label = String::new(); node_text(n, &mut label);
            if label.is_empty() { label = string(n, "label"); }
            if !action.is_empty() && !label.is_empty() { ctl.push(CtlEntry { id: format!("node:{path}"), label: label.clone(), ctl: Ctl::Node(action.clone(), payload.clone()) }); }
            if action.is_empty() || label.is_empty() { return div().flex().gap_1().children(kids(cx, ctl)).into_any_element(); }
            with_rect(div().id(SharedString::from(format!("n{path}"))), || format!("node:{path}")).px_2().py_1().border_1().border_color(c("line")).bg(c("raise")).text_size(crate::scale::tpx(12.)).cursor_pointer().hover(|s| s.bg(c("panel")))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(action.clone(), payload.clone())); cx.notify(); })).child(trs(&label)).into_any_element()
        }
        "h1" => { let mut t = String::new(); node_text(n, &mut t); div().text_size(crate::scale::tpx(22.)).font_weight(FontWeight::BOLD).child(trs(&t)).into_any_element() }
        "h2" | "h3" => { let mut t = String::new(); node_text(n, &mut t); div().font_weight(FontWeight::BOLD).text_size(crate::scale::tpx(13.)).child(trs(&t)).into_any_element() }
        "span" | "p" | "a" | "label" | "code" => {
            let mut t = String::new(); node_text(n, &mut t);
            let mut d = div().child(if tag == "code" { t } else { trs(&t) });
            if class.contains("eyebrow") || class.contains("faint") || class.contains("dim") { d = d.text_size(crate::scale::tpx(11.)).text_color(c("faint")); }
            if class.contains("error") { d = d.text_color(c("bad")); }
            d.into_any_element()
        }
        "section" if class.contains("card") => div().flex().flex_col().gap_2().p_3().border_1().border_color(c("line")).bg(c("panel")).children(kids(cx, ctl)).into_any_element(),
        _ => {
            let row = class.contains("name") || class.contains("row") || class.contains("head") || class.contains("meta");
            let d = div().flex().gap_2();
            (if row { d.items_center() } else { d.flex_col() }).children(kids(cx, ctl)).into_any_element()
        }
    }
}

/// In check mode the control records its window rectangle under its id (the e2e driver aims clicks with it); otherwise nothing is added.
pub(crate) fn with_rect<E: ParentElement + Styled>(e: E, id: impl FnOnce() -> String) -> E { if art::rects_on() { e.relative().child(art::probe_rect(id())) } else { e } }
pub(crate) fn ic(light: bool, name: &'static str, size: f32, tok: &str) -> AnyElement { art::icon(name, size, theme::color(light, tok)) }
/// A rail label: it grows with the text size until the longest ("Schematic") fills the rail's 72 px, and no further (it would wrap).
/// An activity line that is said whole (wrapped) instead of cut at the pane's width: an error, a restart, and a warning, which
/// is something the person has to read to the end to act on ("Design files not committed to main: …" was cut after "Author i").
fn act_whole(tone: &str) -> bool { tone.contains("error") || tone.contains("restart") || tone.contains("warn") }
#[cfg(test)] mod act_tests {
    #[test] fn a_warning_line_is_said_whole_like_an_error() {
        /* i18n-ok: tone names the hub sends, not drawn */ for t in ["error", "warn", "info restart"] { assert!(super::act_whole(t), "{t}") }
        for t in ["info", "", "tool"] { assert!(!super::act_whole(t), "{t}") }
    }
}
// Fit all labels to the 72 px rail, less the separator and reserved selection stripe.
fn rail_label_fit(target: f32, fits: impl Fn(f32) -> bool) -> (f32, bool) {
    const FLOOR: f32 = 9.5;
    let target = target.max(FLOOR);
    if fits(target) { return (target, false); }
    if !fits(FLOOR) { return (FLOOR, true); }
    // Find the largest fitting size at 1/64 px precision using actual shaped widths.
    let (mut lo, mut hi) = ((FLOOR * 64.) as u32, (target * 64.).ceil() as u32);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if fits(mid as f32 / 64.) { lo = mid; } else { hi = mid; }
    }
    (lo as f32 / 64., false)
}

#[derive(Default)]
pub(crate) struct RailLabels {
    labels: Vec<String>,
    font: String,
    target: f32,
    fit: (f32, bool),
}

impl RailLabels {
    fn size(&mut self, labels: &[String], family: &str, target: f32, window: &mut Window) -> (f32, bool) {
        if self.target != target || self.font != family || self.labels != labels {
            let font = gpui::font(SharedString::from(family.to_owned()));
            self.fit = rail_label_fit(target, |size| labels.iter().all(|label| {
                let run = TextRun { len: label.len(), font: font.clone(), color: Hsla::default(),
                    background_color: None, underline: None, strikethrough: None };
                window.text_system().shape_line(SharedString::from(label.clone()), px(size), &[run], None).width <= px(69.)
            }));
            self.labels = labels.to_vec();
            self.font = family.to_owned();
            self.target = target;
        }
        self.fit
    }
}
fn chip(light: bool, label: String) -> Div {
    let c = |name: &str| theme::color(light, name);
    div().px_3().py_1().flex().items_center().justify_center().border_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(12.)).whitespace_nowrap().child(label)
}

/// A message's references as little cards under it: a bot, thread, project or room goes there on a click; a model is only named.
fn ref_cards(cx: &mut Context<Backplane>, light: bool, mid: &str, refs: &Value) -> Option<Div> {
    let list = refs.as_array().filter(|r| !r.is_empty())?;
    let c = |n: &str| theme::color(light, n);
    let mut row = div().flex().flex_wrap().gap_2();
    for (i, r) in list.iter().enumerate() {
        let (kind, id, label, word) = (string(r, "kind"), string(r, "id"), string(r, "label"), string(r, "word"));
        let mark = word.chars().next().map(String::from).unwrap_or_default();
        let go = !id.is_empty();
        let mut card = div().id(SharedString::from(format!("ref-{mid}-{i}"))).flex().items_center().gap_2().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(12.))
            .child(div().text_color(c(if kind == "bot" { "accent" } else { "ok" })).font_weight(FontWeight::BOLD).child(mark))
            .child(div().max_w(px(220.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(label))
            .child(div().text_size(crate::scale::tpx(10.)).text_color(c("faint")).child(trs(&kind)));
        if go {
            card = card.cursor_pointer().hover(|s| s.bg(c("raise"))).on_click(cx.listener(move |this, _, window, cx| {
                match kind.as_str() {
                    "bot" => this.go("bot", id.clone(), cx),
                    "room" => this.go("room", id.clone(), cx),
                    "project" => { this.open_project(id.rsplit('|').next().unwrap_or(&id).to_string(), cx); }
                    _ => { this.select(id.clone(), window, cx); }
                }
                cx.notify();
            }));
        }
        row = row.child(card);
    }
    Some(row)
}

/// "Replying to ..." over the composer: its first line, a click goes to the message, × drops it.
fn reply_chip(cx: &mut Context<Backplane>, light: bool, replying: &Value) -> Option<AnyElement> {
    if !replying.is_object() { return None }
    let c = |n: &str| theme::color(light, n);
    let (target, line) = (string(replying, "id"), string(replying, "text"));
    Some(div().flex().items_center().gap_2().px_2().py_1().mb_1().border_1().border_color(c("line")).bg(c("panel")).text_size(crate::scale::tpx(12.))
        .child(div().id("reply-chip").flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("dim")).cursor_pointer().child(tf("gpui.b.replying.to", &[("line", line.into())]))
            .on_click(cx.listener(move |this, _, _, cx| { this.timeline_goto(&target); cx.notify(); })))
        .child(div().id("reply-drop").px_1().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("bad"))).child("\u{00d7}")
            .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("reply-drop".into(), String::new())); cx.notify(); }))).into_any_element())
}

/// Copy and Reply under a message (a reply puts its first line over the composer and the whole text ahead of what is sent, for the agent).
fn msg_tools(cx: &mut Context<Backplane>, light: bool, id: &str, text: &str) -> Div {
    let c = |n: &str| theme::color(light, n);
    let (copy, reply, rid) = (text.to_string(), format!("{id}\u{1f}{text}"), id.to_string());
    let btn = |tag: &str, label: &'static str| div().id(SharedString::from(format!("{tag}-{rid}"))).px_1().cursor_pointer().text_size(crate::scale::tpx(11.)).text_color(c("faint")).hover(|s| s.text_color(c("text"))).child(trs(label));
    // Copy message: the copyable primitive's copy (the exact text, its check and "Copied" for a moment)
    let cid = SharedString::from(format!("msg-{rid}")); let done = crate::screens::copyable::just_copied(&cid);
    let copy_btn = div().id(SharedString::from(format!("copy-{rid}"))).px_1().cursor_pointer().text_size(crate::scale::tpx(11.)).text_color(c(if done { "ok" } else { "faint" })).hover(|s| s.text_color(c("text")))
        .flex().items_center().gap_1().child(crate::art::icon(if done { "check" } else { "copy" }, 11., c(if done { "ok" } else { "faint" })))
        /* i18n-ok: drawn through trs */ .child(trs(if done { "Copied" } else { "Copy" }))
        .relative().child(crate::art::probe_rect(format!("msgcopy:{rid}"))).on_click(move |_, window, cx| { cx.stop_propagation(); crate::screens::copyable::copy(&cid, &copy, window, cx) });
    /* i18n-ok: drawn through trs in btn */ div().flex().gap_2().child(copy_btn)
        /* i18n-ok: drawn through trs in btn */ .child(btn("reply", "Reply").on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("reply".into(), reply.clone())); cx.notify(); })))
}

/// The confirm of a message's Restore to before this, under its bubble: what it does, the snapshot's short info (the hub's `restore.info`), Restore and Cancel.
/// Restore sends `thread.restore {thread, turn, commit}`; the hub says in the thread what it restored.
fn restore_confirm(cx: &mut Context<Backplane>, light: bool, thread: &str, id: &str, r: &Value) -> Stateful<Div> {
    let c = |n: &str| theme::color(light, n);
    let p = serde_json::json!({"thread": thread, "turn": r["turn"].as_u64().unwrap_or(0), "commit": string(r, "commit")});
    let btn = |tag: &str, label: &'static str, main: bool| div().id(SharedString::from(format!("{tag}-{id}"))).px_2().py(px(2.)).border_1().cursor_pointer().text_size(crate::scale::tpx(12.))
        .border_color(c(if main { "accent" } else { "line" })).text_color(c(if main { "accent" } else { "dim" })).hover(|s| s.bg(c("raise"))).child(trs(label));
    div().id(SharedString::from(format!("restore-ask-{id}"))).px_3().py_2().border_1().border_color(c("line")).flex().flex_col().gap_1().text_size(crate::scale::tpx(12.))
        /* i18n-ok: drawn through trs */ .child(div().text_color(c("text")).child(trs("Restore the project to before this message?")))
        .child(div().text_color(c("faint")).font(art::mono_font()).child(string(r, "info")))
        /* i18n-ok: drawn through trs */ .child(div().text_color(c("dim")).child(trs("Restoring makes a new commit. The project as it is now is kept as a commit too.")))
        .child(div().flex().justify_end().gap_2().mt_1()
            /* i18n-ok: drawn through trs in btn */ .child(btn("restore-no", "Cancel", false).on_click(cx.listener(|this, _, _, cx| { this.screens.restore_ask.clear(); cx.notify(); })))
            /* i18n-ok: drawn through trs in btn */ .child(btn("restore-go", "Restore", true).on_click(cx.listener(move |this, _, _, cx| { this.screens.restore_ask.clear(); let _ = this.tx.try_send(Command::Rpc("thread.restore".into(), crate::screens::settings::Secret(p.clone()), "restore")); cx.notify(); }))))
}

impl Backplane {
    /// One thread row of the sidebar (a project's, or the Active view's: `sub` names the project there, `faded` is a settled one).
    pub(crate) fn side_thread(&self, cx: &mut Context<Self>, light: bool, t: &Value, pid: &str, sub: &str, faded: bool, ctl: &mut Vec<CtlEntry>) -> AnyElement {
        let c = |name: &str| theme::color(light, name);
                let id = string(t, "id");
                let raw = id.rsplit_once('|').map_or(id.as_str(), |(_, i)| i);
                let selected = raw == self.selected && self.project_page.is_none() && !crate::screens::swarm::over_thread(&self.data); // a project's page on show: its head is the one highlighted, no row (nor on a room's page)
                ctl.push(CtlEntry { id: format!("thread:{raw}"), label: string(t, "title"), ctl: Ctl::Select(id.clone()) });
                let (depth, kids, open) = (t["depth"].as_u64().unwrap_or(0).min(6) as f32, t["children"].as_u64().unwrap_or(0), t["expanded"].as_bool().unwrap_or(false));
                // only a project's tree unfolds: an Active-view or Conversation-history row with subthreads has no rows to show under it, so no chevron
                // a row under its project row (`under`, the sections) is one of its few fixed rows: no tree of its own either
                let under = t["under"] == true;
                let tree = sub.is_empty() && !pid.is_empty() && !under;
                let gutter = if under { 44. } else { 6. + depth * 14. + 16. }; // the indent and the chevron's slot (under a project: its title lines up with the project's name, the L hangs from the project's icon)
                // the chevron's target is the row's full height and the whole gutter left of the dot (the arrow alone, 16 x 14, was easy to miss: a near miss selected the thread)
                let toggle = (kids > 0 && tree).then(|| {
                    let tid = id.clone();
                    with_rect(div().id(SharedString::from(format!("tree-{id}"))), { let r = raw.to_string(); move || format!("tree:{r}") }).flex_shrink_0().w(px(gutter)).h_full().pl(px(6. + depth * 14.)).flex().items_center().cursor_pointer().hover(|s| s.bg(c("line"))).child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim"))
                        .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("thread-tree".into(), format!("{tid}|{}", if open { 0 } else { 1 }))); cx.stop_propagation(); cx.notify(); }))
                });
                /* i18n-ok: a ctl label */ if kids > 0 && tree { ctl.push(CtlEntry { id: format!("tree:{raw}"), label: format!("{} subthreads", kids), ctl: Ctl::Node("thread-tree".into(), format!("{id}|{}", if open { 0 } else { 1 })) }); }
                let pin_group = SharedString::from(format!("row-{id}"));
                let pin = crate::screens::siderow::pin_button(cx, light, t, pin_group.clone());
                { let (action, value) = crate::screens::siderow::pin_press(t); ctl.push(CtlEntry { id: format!("pin:{raw}"), label: action.into(), ctl: Ctl::Node(action.into(), value) }); }
                with_rect(div().id(SharedString::from(format!("thread-{id}"))), || format!("thread:{raw}")).group(pin_group).flex().items_center().gap_2().pr_3().h(crate::scale::tpx(32.)).when(faded || t["off"] == true, |d| d.opacity(0.55))
                    // a row under its project: no bar and no block when open (the person, 2026-11-15); its L says where it is
                    .border_l_2().border_color(if selected && !under { c("accent") } else { transparent_black().into() })
                    .bg(if selected && !under { c("raise") } else { transparent_black().into() }).hover(|s| s.bg(c("raise"))).cursor_pointer()
                    .on_click(cx.listener({ let (sid, off) = (id.clone(), t["off"] == true); move |this, _, window, cx| if !off { this.select(sid.clone(), window, cx) } })) // (a thread of a machine that is off: listed from the registry, nothing to open)
                    .when(sub.is_empty(), |d| d.on_drag(DragRow { id: raw.to_string(), label: string(t, "title") }, |d, _, _, cx| { let l = d.label.clone(); cx.new(|_| Ghost(l)) })
                        .drag_over::<DragRow>({ let a = c("accent"); move |s, _, _, _| s.border_t_2().border_color(a) }))
                    .on_drop(cx.listener({ let target = raw.to_string(); move |this, d: &DragRow, _, cx| { let _ = this.tx.try_send(Command::Action("reorder".into(), format!("{}\u{1f}{}", d.id, target))); cx.notify(); } }))
                    .on_mouse_down(MouseButton::Right, { let (row, pid2) = (t.clone(), pid.to_string()); cx.listener(move |this, e: &MouseDownEvent, _, cx| { let items = crate::screens::ctxmenu::thread_items(&row, &pid2); this.ctx_open(e.position, items, cx); }) })
                    // a row under its project row: a small faint L in the indent (half a row down, then right to the dot), so it reads
                    // as the project's own; the open one's L a shade brighter
                    .child(toggle.map_or_else(|| div().w(px(gutter)).h_full().flex_shrink_0().relative().when(under, |d| { let l = c(if selected { "dim" } else { "line" });
                        d.child(div().absolute().left(px(36.)).top_0().w(px(1.)).h(relative(0.5)).bg(l))
                         .child(div().absolute().left(px(36.)).top(relative(0.5)).w(px(11.)).h(px(1.)).bg(l)) }).into_any_element(), |t| t.into_any_element())).child(crate::screens::siderow::dot(light, t)).child(if crate::screens::siderow::renaming(&self.data, &id, "thread") { crate::screens::siderow::field(light, &self.screens.fields, &id) } else { div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().flex().gap_2().relative().child(div().min_w_0().overflow_hidden().text_ellipsis().child(string(t, "title"))).children((!sub.is_empty()).then(|| div().flex_shrink_0().max_w(px(80.)).overflow_hidden().text_ellipsis().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(sub.to_string())))
                        // held Ctrl: this row's Ctrl+digit; held Alt, in the Active view: its project's Alt+digit; over the title's end, so nothing moves (screens/hints.rs)
                        .children(crate::screens::hints::digit_of(&self.screens.hold_threads, &id).or_else(|| (!sub.is_empty()).then(|| crate::screens::hints::digit_of(&self.screens.hold_projects, pid)).flatten()).map(|n| crate::screens::hints::over_end(crate::screens::hints::digit(light, n)))).into_any_element() })
                    .child(pin) // the pin: always on a pinned row, on hover on the others
                    .children(crate::screens::badge::machine_mark(light, &t["mark"], false, &id)) // (another machine's thread: its name, dim; none for this machine's)
                    .child({ let want = string(t, "want"); // a red row says what it wants where its age goes (0.14 Side.want, r01)
                        if want.is_empty() { div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(if kids > 0 && !under && string(t, "agents").is_empty() { format!("{kids}") } else { trs(&string(t, "ago")) }) }
                        else { div().text_size(crate::scale::tpx(11.)).text_color(c(if want == "failed" { "dim" } else { "bad" })).child(trs(&want)) } }).into_any_element()
    }
}

impl Backplane {
    /// One row of the timeline, as the list asks for it: only rows on screen are drawn.
    /// Item `ix` of a project's page (screens/projects.rs `item`): its head, a README block, its tail; in the page's column.
    pub(crate) fn page_item(&mut self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        use crate::screens::projects::{self as pr, PageItem as Item, Part};
        let light = self.light();
        let Some(pid) = self.project_page.clone() else { return div().into_any_element() };
        let home = &self.screens.projects.home;
        let n = if home.pid == pid { home.readme.len() } else { 0 };
        let reg = self.screens.reg.clone();
        // (its width as the page's render measured it: the list's own state is borrowed while it lays out its items)
        let doc = crate::screens::timeline::Doc { imgs: &self.screens.images, target: self.screens.attacher.target.as_ref(), width: self.screens.home_doc_w };
        let mut ctl = Vec::new();
        let el = match pr::item(ix, n) {
            Item::Head => div().flex().flex_col().gap_3().pb_3().children(pr::page(cx, &reg, light, &self.data, home, &pid, &mut ctl, &doc, Part::Head)).into_any_element(),
            Item::Block(i) => pr::readme_block(cx, &reg, light, home, i, &doc),
            Item::Tail => div().flex().flex_col().gap_3().children(pr::page(cx, &reg, light, &self.data, home, &pid, &mut ctl, &doc, Part::Tail)).into_any_element(),
        };
        // the page's column: 930 at most, centred
        div().w_full().flex().justify_center().child(div().w_full().max_w(px(930.)).px_4().when(ix == 0, |d| d.pt_4()).when(matches!(pr::item(ix, n), Item::Tail), |d| d.pb_4()).child(el)).into_any_element()
    }
    pub(crate) fn timeline_row(&mut self, ei: usize, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        let c = |name: &str| theme::color(light, name);
        let reg = self.screens.reg.clone();
        let e = self.data["thread"]["entries"][ei].clone();
        let id = string(&e, "id");
        reg.borrow_mut().enter(ei, &id);
        let kind = e["kind"].as_str().unwrap_or("");
        let text = string(&e, "text");
        let stamp = string(&e, "stamp");
        // a reply whose stream was on screen takes the streamed row's place as it is: no fade, no slide
        let was_streamed = kind == "assistant" && !id.starts_with('@') && !self.screens.streamed.is_empty() && { let a: String = self.screens.streamed.trim().chars().take(60).collect(); text.trim_start().starts_with(&a) };
        let row: AnyElement = match kind {
            "user" => {
                let wide = crate::screens::timeline::bubble_full_width(&text, &e["attachments"]);
                let ref_words: Vec<String> = e["refs"].as_array().into_iter().flatten().map(|r| string(r, "word")).collect();
                let body = crate::screens::timeline::user_text(&reg, light, &text, &ref_words);
                let atts = crate::screens::timeline::attachments(cx, light, &e["attachments"], &id, self.screens.attacher.target.as_ref(), &self.screens.images);
                let tools = msg_tools(cx, light, &id, &text);
                let cards = ref_cards(cx, light, &id, &e["refs"]);
                // r05: a follow-up the agent has not read yet: its text a step dimmer, its time ("not read yet · 06:13") in the queued colour
                let unread = e["unread"] == true;
                let spoken = crate::screens::voice::bubble_mark(cx, light, &self.screens.voice, &id, &e["voice"]);
                // Restore to before this (the hub's `restore`: the snapshot before the turn this message started): on hover; its confirm shows the snapshot
                let group = SharedString::from(format!("um-{id}"));
                let asking = e["restore"].is_object() && self.screens.restore_ask == id;
                let restore = (e["restore"].is_object() && !asking).then(|| { let rid = id.clone();
                    div().id(SharedString::from(format!("restore-{id}"))).px_1().cursor_pointer().invisible().group_hover(group.clone(), |s| s.visible()).hover(|s| s.text_color(c("text")))
                        /* i18n-ok: drawn through trs */ .child(trs("Restore to before this")).on_click(cx.listener(move |this, _, _, cx| { this.screens.restore_ask = rid.clone(); cx.notify(); })) });
                let confirm = asking.then(|| restore_confirm(cx, light, &self.selected, &id, &e["restore"]));
                let stamp_row = div().flex().justify_end().items_center().gap_3().text_size(crate::scale::tpx(11.)).text_color(c("faint")).children(restore).children(spoken).child(tools).child(div().when(unread, |d| d.text_color(c("queued"))).child(crate::i18n::trs(&stamp)));
                let body = div().when(unread, |d| d.opacity(0.7)).child(body);
                let reply_to = e["replyTo"].is_object().then(|| {
                    let (target, quote) = (string(&e["replyTo"], "id"), string(&e["replyTo"], "quote"));
                    div().id(SharedString::from(format!("rt-{id}"))).mb_1().max_w_full().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(crate::scale::tpx(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                        .child(tf("gpui.b.in.reply.to", &[("quote", quote.into())])).on_click(cx.listener(move |this, _, _, cx| { this.timeline_goto(&target); cx.notify(); }))
                });
                let row = div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().group(group);
                // A text that may wrap sits in block layout like a reply's (a shrink-to-fit flex bubble measured it at one width and painted it at another);
                // a message of short lines keeps the snug flex bubble.
                if wide {
                    row.child(div().w_full().max_w(px(640.)).ml_auto().px_3().py_2().bg(c("raise")).border_1().border_color(c("line")).children(reply_to).child(body).children(cards.map(|k| k.mt_1())).children(atts.into_iter().map(|a| div().mt_1().child(a))).child(div().mt_1().child(stamp_row))).children(confirm.map(|k| div().w_full().max_w(px(640.)).ml_auto().mt_1().child(k))).into_any_element()
                } else {
                    row.flex().flex_col().items_end().child(div().max_w(px(640.)).px_3().py_2().bg(c("raise")).border_1().border_color(c("line")).flex().flex_col().gap_1().children(reply_to).child(body).children(cards).children(atts).child(stamp_row)).children(confirm.map(|k| div().max_w(px(640.)).mt_1().child(k))).into_any_element()
                }
            }
            "fold" => {
                let value = string(&e, "value");
                self.screens.parts.rows_ctl.push(CtlEntry { id: format!("fold:{id}"), label: text.clone(), ctl: Ctl::Fold(value.clone()) });
                // the label is one clipped line beside the chevron, never wider than the row; the chevron is centred on it
                div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().child(
                    with_rect(div().id(SharedString::from(format!("f-{id}"))), || format!("fold:{id}")).w_full().min_w_0().overflow_hidden().flex().items_center().gap_2().px_2().py_1().border_1().border_color(c("line")).text_size(crate::scale::tpx(12.)).text_color(c("dim")).cursor_pointer()
                        .hover(|s| s.bg(c("raise")))
                        .on_click(cx.listener(move |this, _, _, cx| { this.fold_click(value.clone()); cx.notify(); }))
                        .child(div().flex_shrink_0().flex().items_center().child(ic(light, if e["open"] == true { "chevron-down" } else { "chevron-right" }, 14., "dim")))
                        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(trs(&text)))).into_any_element()
            }
            // the agent still working: a blue square that breathes beside the word (the one animation on screen, only while this row is; blue as the sidebar's and the head's working dots, 0.14's --st-work)
            "assistant" if id.starts_with("@live:") || id.starts_with("@working:") => {
                // drawn at crate::part::PULSE's rate, not every display frame: each step draws this part (the visible messages) and the root's shell again
                let (d, dots) = crate::part::pulse(self.started.elapsed());
                self.screens.parts.tick(Backplane::timeline_part, crate::part::PULSE, cx);
                let square = div().size(px(12.)).flex_shrink_0().flex().items_center().justify_center().child(div().size(px(5. + 7. * d)).opacity(0.45 + 0.55 * d).bg(c("accent")));
                div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().flex().items_center().gap_2().text_color(c("dim")).child(square)
                    // the dots count up and start over: "Working." "Working.." "Working..." (a fixed width so the line does not jitter)
                    .child(div().min_w(px(86.)).child(format!("{}{}", trs("Working"), ".".repeat(dots)))).into_any_element()
            }
            "assistant" => {
                let mut col = div().id(SharedString::from(format!("e-{id}"))).w_full().py_2().flex().flex_col().gap_2();
                match e["blocks"].as_array() {
                    Some(blocks) if !blocks.is_empty() => { let parsed = self.screens.parsed.get(&id, &text, blocks); for el in crate::screens::timeline::blocks(cx, &reg, light, &parsed, &id) { col = col.child(el); } }
                    _ => { col = col.child(crate::screens::timeline::user_text(&reg, light, &text, &[])); }
                }
                if let Some(im) = crate::screens::timeline::images(cx, light, &e["images"], &id, self.screens.attacher.target.as_ref(), &self.screens.images) { col = col.child(im); }
                // the reply being streamed: no Copy/Reply until its message lands
                let streaming = e["streaming"] == true; if streaming { self.screens.streamed = text.clone(); }
                let tools = if streaming { div() } else { msg_tools(cx, light, &id, &text) };
                // s04: a reply inside a turn has no stamp of its own; its time (`hover`) shows while the pointer is on it
                let hover = string(&e, "hover"); let group = SharedString::from(format!("msg-{id}"));
                // (after Copy and Reply, so the hidden time leaves no gap in front of them)
                let foot = div().flex().items_center().gap_3().text_size(crate::scale::tpx(11.)).text_color(c("faint"));
                let foot = if stamp.is_empty() && !hover.is_empty() { foot.child(tools).child(div().invisible().group_hover(group.clone(), |s| s.visible()).child(crate::i18n::trs(&hover))) } else { foot.child(crate::i18n::trs(&stamp)).child(tools) };
                col.group(group).when(!streaming, |d| d.child(foot)).into_any_element()
            }
            "term" => self.term_card(&e, light, cx),
            "handoff" => crate::screens::handoff::card(cx, light, &e),
            "halt" => crate::screens::halt::card(self, cx, light, &e),
            "step" => crate::screens::histbar::step_row(cx, light, &e),
            // an Agent or Task call: its subagent's own view opens from it (the hub names it `sub`)
            _ if !string(&e, "sub").is_empty() => {
                let key = string(&e, "sub");
                self.screens.parts.rows_ctl.push(CtlEntry { id: format!("sub:{key}"), label: string(&e, "label"), ctl: Ctl::Sub(key.clone()) });
                let rid = format!("sub:{key}");
                with_rect(div().id(SharedString::from(format!("e-{id}"))), || rid).w_full().flex().items_center().gap_2().py_1().font(art::mono_font()).text_size(crate::scale::tpx(12.)).text_color(c("dim")).cursor_pointer().hover(|s| s.bg(c("raise")))
                    .on_click(cx.listener(move |this, _, _, cx| this.sub_open(key.clone(), cx)))
                    .child(div().flex_shrink_0().text_color(c("faint")).child(string(&e, "label"))).child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(text.lines().next().unwrap_or("").to_string()))
                    .child(div().flex_shrink_0().text_color(c("accent")).child(trs("Open"))).child(ic(light, "chevron-right", 12., "accent")).into_any_element()
            }
            // s03: the call that runs now, in an open fold of the running turn: its name in the accent with the live mark beside it
            _ if !text.is_empty() && e["now"] == true => div().id(SharedString::from(format!("e-{id}"))).flex().items_center().gap_2().py_1().font(art::mono_font()).text_size(crate::scale::tpx(12.)).text_color(c("text"))
                .child(div().size(px(6.)).flex_shrink_0().bg(c("accent"))).child(div().text_color(c("accent")).child(string(&e, "label"))).child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(text.lines().next().unwrap_or("").to_string())).into_any_element(),
            // an activity: its kind faint, what it did on one line (a heredoc's first line, "…"), as 0.14's .act; an error, a warning or a restart says all of it
            _ if !text.is_empty() => {
                let tone = string(&e, "tone"); let whole = act_whole(&tone);
                let line = if whole { crate::i18n::trs(&text) } else { let mut l = text.lines().next().unwrap_or("").to_string(); if text.trim_end().contains('\n') { l.push_str(" …"); } l };
                let body = div().flex_1().min_w_0().when(!whole, |d| d.overflow_hidden().whitespace_nowrap().text_ellipsis()).when(tone.contains("error") && !tone.contains("restart"), |d| d.text_color(c("bad"))).child(line);
                let row = div().id(SharedString::from(format!("e-{id}"))).w_full().flex().gap_2().py_1().font(art::mono_font()).text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(div().flex_shrink_0().text_color(c("faint")).child(crate::i18n::trs(&string(&e, "label")))).child(body);
                // a notice's code (the hub's `copy`): each piece with its copy button, and Run in terminal for a command
                match crate::screens::copyable::pieces(cx, light, &format!("act-{id}"), &e["copy"]) { Some(p) => div().w_full().flex().flex_col().child(row).child(p).into_any_element(), None => row.into_any_element() }
            }
            // a call whose summary is only its name (WebSearch, TaskUpdate...): its name alone, so an open fold shows as many rows as it counts
            _ if !string(&e, "label").is_empty() => div().id(SharedString::from(format!("e-{id}"))).flex().gap_2().py_1().font(art::mono_font()).text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(string(&e, "label")).into_any_element(),
            _ => div().into_any_element(),
        };
        // the message the find bar (or Find across threads) is on: a band in the accent beside it
        let marked = !id.is_empty() && id == self.screens.find.mark;
        let inner = div().w_full().max_w(px(930.)).px_4().when(marked, |d| d.bg(c("raise")).border_l_2().border_color(c("accent"))).child(row);
        // A row that arrives while the thread is on screen (a sent message, the reply starting) fades and slides in over 220 ms instead of popping in.
        // Only the newest rows, only within a moment of their first draw, so scrolling back to a row or opening a thread never replays it.
        let len = self.data["thread"]["entries"].as_array().map_or(0, |r| r.len());
        let settled = self.screens.shown_at.elapsed().as_millis() > 800;
        // born: when the row came in with a picture (glue.rs timeline_picture), not when it was first drawn: a row scrolled to for the first time,
        // or drawn after a page re-laid the list, did not just arrive (Nov 10: the newest rows faded in again on opening Bub)
        let fresh = settled && ei + 3 >= len && self.screens.born.get(&id).map_or(false, |t| t.elapsed().as_millis() < 260) && !was_streamed;
        let inner: AnyElement = if fresh {
            inner.with_animation(SharedString::from(format!("arrive-{id}")), Animation::new(std::time::Duration::from_millis(220)).with_easing(ease_out_quint()),
                |el, d| el.opacity(d).mt(px(10. * (1. - d)))).into_any_element()
        } else { inner.into_any_element() };
        div().w_full().flex().justify_center().child(inner).into_any_element()
    }
    /// The sidebar's rows, a part of the window (part.rs): the uberbot, projects and their threads, the Active view, pagers, conversation history, Older, BOTS and ROOMS.
    pub(crate) fn sidebar_part(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        let c = |name: &str| theme::color(light, name);
        // bots: (name, mood, note, look seed) copied out so the cat cache can be filled
        // the uberbot (flagged by the hub) is drawn in its own spot above the projects, so it is kept apart from the BOTS list
        let uber_data: Vec<(String, String, String, u64, String, bool)> = self.data["bots"].as_array().into_iter().flatten().filter(|b| b["uber"].as_bool().unwrap_or(false))
            .map(|b| (string(b, "name"), string(b, "mood"), string(b, "note"), b["look"].as_u64().unwrap_or(0), string(b, "id"), b["sel"].as_bool().unwrap_or(false))).collect();
        // each bot's fur colour ("#rrggbb", "" its coat's own), by id
        let colors: std::collections::HashMap<String, String> = self.data["bots"].as_array().into_iter().flatten().map(|b| (string(b, "id"), string(b, "color"))).collect();
        let fur = |id: &str| colors.get(id).cloned().unwrap_or_default();
        let uber_rigs: Vec<_> = uber_data.iter().map(|(_, mood, _, look, id, _)| self.cats.rig(*look, mood, &fur(id))).collect();
        // a bot or room of a linked machine carries that machine's marker (the hub's `mark`, as threads elsewhere do); none for this machine's
        let marks: std::collections::HashMap<String, Value> = ["bots", "rooms"].iter().flat_map(|k| self.data[*k].as_array().into_iter().flatten()).filter(|b| b["mark"].is_object()).map(|b| (string(b, "id"), b["mark"].clone())).collect();
        let bot_data: Vec<(String, String, String, u64, String, bool)> = self.data["bots"].as_array().into_iter().flatten().filter(|b| !b["uber"].as_bool().unwrap_or(false))
            .map(|b| (string(b, "name"), string(b, "mood"), string(b, "note"), b["look"].as_u64().unwrap_or(0), string(b, "id"), b["sel"].as_bool().unwrap_or(false))).collect();
        let bot_rigs: Vec<_> = bot_data.iter().map(|(_, mood, _, look, id, _)| self.cats.rig(*look, mood, &fur(id))).collect();
        // a bot's or room's right-click menu: the hub's `menu` on its row (Delete bot; Rename, Delete room), drawn as it is
        let row_menus: std::collections::HashMap<String, Value> = ["bots", "rooms"].iter().flat_map(|k| self.data[*k].as_array().into_iter().flatten()).filter(|b| b["menu"].as_array().map_or(false, |m| !m.is_empty())).map(|b| (string(b, "id"), b["menu"].clone())).collect();
        let started = self.started;
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(r) = crate::screens::onboard::resume_row(cx, light, &self.data) { rows.push(r); }
        // always on: listening is shown at the top of the sidebar whenever it is on
        if let Some(r) = crate::screens::voice::always_row(cx, light, &self.screens.voice, self.screens.voice.always_wanted) { rows.push(r); }
        let mut ctl: Vec<CtlEntry> = Vec::new();
        for ((bname, mood, note, _, bid, bsel), rig) in uber_data.iter().zip(uber_rigs.iter()) {
            let note_color = match mood.as_str() { "wait" | "error" => c("bad"), "work" | "talk" => c("accent"), _ => c("faint") };
            ctl.push(CtlEntry { id: format!("bot:{bname}"), label: bname.clone(), ctl: Ctl::Node("bot".into(), bid.clone()) });
            let bid2 = bid.clone();
            rows.push(with_rect(div().id(SharedString::from(format!("bot-{bid}"))), || format!("bot:{bname}")).flex().items_center().gap_3().px_3().py(px(10.)).border_b_1().border_color(c("line")).cursor_pointer().when(*bsel, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
                .on_click(cx.listener(move |this, _, _, cx| this.go("bot", bid2.clone(), cx)))
                .child(art::cat(rig.clone(), mood, 56., started, if art::rects_on() { format!("cat:{bname}") } else { String::new() }))
                .child(div().flex().flex_col().min_w_0().line_height(crate::scale::tpx(22.)).child(div().text_size(crate::scale::tpx(16.)).overflow_hidden().whitespace_nowrap().child(bname.clone()))
                    .child(div().flex().gap_2().items_center().child(div().text_size(crate::scale::tpx(11.)).text_color(note_color).child(trs(note))).children(marks.get(bid).and_then(|m| crate::screens::badge::machine_mark(light, m, false, bid))))).into_any_element());
        }
        let new_here = crate::keys::new_target(&self.data, &self.selected, self.project_page.as_deref());
        // the hub's sections (Needs you, Pinned, Active, groups, Quiet: screens/sections.rs); an older hub sends projects and the Active view
        let sectioned = crate::screens::sections::on(&self.data);
        if sectioned { self.section_rows(cx, light, &mut rows, &mut ctl); }
        if !sectioned {
        for project in self.data["projects"].as_array().into_iter().flatten() {
            let pid = string(project, "id"); let pid = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
            ctl.push(CtlEntry { id: format!("project:{pid}"), label: string(project, "title"), ctl: Ctl::Home(pid.clone()) });
            for n in crate::screens::ctxmenu::new_items(&pid, &project["newOn"]).into_iter().filter(|_| project["newOn"].is_array()) { ctl.push(CtlEntry { id: format!("new-on:{}", n.label), label: n.label.clone(), ctl: Ctl::Node(n.action, n.value) }); }
            let (pid_new, pid_new2, pid_thr, pid_set) = (pid.clone(), pid.clone(), pid.clone(), pid.clone());
            let (on_page, closed, qpid) = (self.project_page.as_deref() == Some(pid_new.as_str()), project["closed"].as_bool().unwrap_or(false), string(project, "id"));
            let fold_value = format!("project-closed:{pid}");
            /* i18n-ok: a ctl label */ ctl.push(CtlEntry { id: format!("fold:{pid}"), label: format!("{} {}", if closed { "Unfold" } else { "Fold" }, string(project, "title")), ctl: Ctl::Node("flag".into(), fold_value.clone()) });
            // a quiet project (nothing on a live shelf: the hub's `quiet`, the Older ones too) is drawn faded, its head and its rows
            let quiet = project["quiet"] == true;
            rows.push(with_rect(div().id(SharedString::from(format!("proj-{pid}"))), || format!("project:{pid}")).when(quiet, |d| d.opacity(0.55)).flex().items_center().gap_2().pl(px(6.)).pr_3().pt_3().pb_1().text_size(crate::scale::tpx(11.)).text_color(if on_page { c("text") } else { c("dim") }).border_l_2().border_color(if on_page { c("accent") } else { c("bg") }).when(on_page, |d| d.bg(c("raise"))).cursor_pointer().hover(|s| s.text_color(c("text")))
                .on_click(cx.listener(move |this, _, _, cx| { this.open_project(pid.clone(), cx); }))
                .on_drag(DragProj { id: pid_new.clone(), label: string(project, "title") }, |d, _, _, cx| { let l = d.label.clone(); cx.new(|_| Ghost(l)) })
                .drag_over::<DragProj>({ let a = c("accent"); move |s, _, _, _| s.border_t_2().border_color(a) })
                .on_drop(cx.listener({ let target = pid_new.clone(); move |this, d: &DragProj, _, cx| { let _ = this.tx.try_send(Command::Action("reorder-project".into(), format!("{}\u{1f}{}", d.id, target))); cx.notify(); } }))
                .on_mouse_down(MouseButton::Right, { let pidm = pid_new.clone(); let proj = project.clone(); cx.listener(move |this, e: &MouseDownEvent, _, cx| { let items = crate::screens::ctxmenu::project_items_of(&pidm, &proj); this.ctx_open(e.position, items, cx); }) })
                .child(with_rect(div().id(SharedString::from(format!("fold-{pid_new}"))), || format!("fold:{pid_new}")).w(px(16.)).flex_shrink_0().flex().items_center().cursor_pointer().hover(|s| s.bg(c("raise"))).child(ic(light, if closed { "chevron-right" } else { "chevron-down" }, 14., "dim"))
                    .on_click(cx.listener(move |this, _, _, cx| { cx.stop_propagation(); let _ = this.tx.try_send(Command::Action("flag".into(), fold_value.clone())); cx.notify(); })))
                .child(crate::screens::badge::mark(light, project)).child(div().flex_1().min_w_0().flex().items_center().gap_2().relative().child(if crate::screens::siderow::renaming(&self.data, &qpid, "project") { crate::screens::siderow::field(light, &self.screens.fields, &qpid) } else { div().child(string(project, "title").to_uppercase()).into_any_element() }).children(crate::screens::badge::place(light, &project["mark"], project["also"].as_str().unwrap_or(""), &format!("proj-{pid_new}")))
                    /* i18n-ok: ctl labels and ui: ids the drivers press */ .children(crate::screens::hints::digit_of(&self.screens.hold_projects, &pid_new).map(|n| crate::screens::hints::over_end(crate::screens::hints::digit(light, n))))).child(with_rect(div().id(SharedString::from(format!("new-{pid_new}"))), || "ui:New thread".to_string()).cursor_pointer().tooltip(crate::keys::tip("New thread", &if pid_new == new_here { crate::keys::hint(&self.local, "new") } else { String::new() }, light)).on_click({ let new_on = project["newOn"].clone(); cx.listener(move |this, e: &ClickEvent, _, cx| { cx.stop_propagation();
                        // a project also on linked machines: the + asks which machine the thread starts on
                        let items = crate::screens::ctxmenu::new_items(&pid_new2, &new_on); if items.len() > 1 { this.ctx_open(e.position(), items, cx); return }
                        this.arm_new(); let _ = this.tx.try_send(Command::Action("new-thread".into(), pid_new2.clone())); cx.notify(); }) }).child(ic(light, "plus", 16., "dim"))).into_any_element());
            if project["look"].is_object() {
                for v in crate::screens::badge::choices(&project["look"], "colors").into_iter().chain(crate::screens::badge::choices(&project["look"], "icons")) { ctl.push(CtlEntry { id: format!("look:{}", v.1), label: v.0, ctl: Ctl::Node("look-set".into(), v.1) }); }
                if let Some((label, value)) = crate::screens::badge::again(&project["look"]) { ctl.push(CtlEntry { id: "look:again".into(), label, ctl: Ctl::Node("look-again".into(), value) }); }
                rows.push(crate::screens::badge::editor(cx, light, &pid_thr, &project["look"]));
            }
            if closed { continue } // folded: the head alone (0.14 project-closed:<id>)
            // the project's swarms (docs/SWARM.md), above its threads: the hub leaves their members out of the shelves
            { let open_room = if self.data["bot"]["kind"] == "room" { string(&self.data["bot"], "id") } else { String::new() }; rows.extend(crate::screens::swarm::side_rows(cx, light, project, &open_room, &mut ctl)); }
            for t in project["threads"].as_array().into_iter().flatten() { rows.push(self.side_thread(cx, light, t, &pid_thr, "", quiet, &mut ctl)); }
            // a long list: the hub's page controls for the rest (at most 48 rows a picture)
            if let Some(row) = pager_row(cx, format!("pg-t-{pid_thr}"), &project["threadsPage"], light, &mut ctl) { rows.push(row); }
            // the Settled shelf: a drop-down at the foot of the project; the hub builds its rows only while it is open
            let settled_n = project["settledCount"].as_u64().unwrap_or_else(|| project["settled"].as_array().map_or(0, |a| a.len() as u64));
            let settled_open = project["open"].as_bool().unwrap_or(false);
            if settled_n > 0 || settled_open {
                let value = string(project, "id");
                /* i18n-ok: a ctl label */ ctl.push(CtlEntry { id: format!("settled:{pid_set}"), label: format!("Settled ({settled_n})"), ctl: Ctl::Node("flag".into(), value.clone()) });
                rows.push(with_rect(div().id(SharedString::from(format!("settled-{pid_set}"))), || format!("settled:{pid_set}")).flex().items_center().gap_1().pl(px(6.)).pr_3().h(crate::scale::tpx(26.)).text_size(crate::scale::tpx(11.)).text_color(c("faint")).cursor_pointer().hover(|s| s.bg(c("raise")))
                    .child(div().w(px(16.)).flex_shrink_0().child(ic(light, if settled_open { "chevron-down" } else { "chevron-right" }, 14., "dim"))).child(tf("gpui.b.settled.shelf", &[("n", settled_n.into())]))
                    .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("flag".into(), value.clone())); cx.notify(); })).into_any_element());
                if settled_open { for t in project["settled"].as_array().into_iter().flatten() { rows.push(self.side_thread(cx, light, t, &pid_thr, "", true, &mut ctl)); } if let Some(row) = pager_row(cx, format!("pg-s-{pid_thr}"), &project["settledPage"], light, &mut ctl) { rows.push(row); } }
            }
            // the Archived shelf: under Settled, folded until opened (the hub's `arch:<project>` flag); a row's menu brings it back
            let arch_n = project["archivedCount"].as_u64().unwrap_or_else(|| project["archived"].as_array().map_or(0, |a| a.len() as u64));
            let arch_open = project["archOpen"].as_bool().unwrap_or(false);
            if arch_n > 0 || arch_open {
                let value = string(project, "value");
                ctl.push(CtlEntry { id: format!("archived:{pid_set}"), label: format!("Archived ({arch_n})") /* i18n-ok: a ctl label */, ctl: Ctl::Node("flag".into(), value.clone()) });
                rows.push(with_rect(div().id(SharedString::from(format!("archived-{pid_set}"))), { let p = pid_set.clone(); move || format!("archived:{p}") }).flex().items_center().gap_1().pl(px(6.)).pr_3().h(crate::scale::tpx(26.)).text_size(crate::scale::tpx(11.)).text_color(c("faint")).cursor_pointer().hover(|s| s.bg(c("raise")))
                    .child(div().w(px(16.)).flex_shrink_0().child(ic(light, if arch_open { "chevron-down" } else { "chevron-right" }, 14., "dim"))).child(crate::i18n::tf("gpui.sidebar.archived.count", &[("n", arch_n.into())]))
                    .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("flag".into(), value.clone())); cx.notify(); })).into_any_element());
                if arch_open { for t in project["archived"].as_array().into_iter().flatten() { rows.push(self.side_thread(cx, light, t, &pid_thr, "", true, &mut ctl)); } if let Some(row) = pager_row(cx, format!("pg-a-{pid_thr}"), &project["archivedPage"], light, &mut ctl) { rows.push(row); } }
            }
        }
        // the Active view: threads that need the person first, then the settled ones folded under "Settled N"
        for t in self.data["active"].as_array().into_iter().flatten() {
            let pid = string(t, "projectId"); let pid = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
            rows.push(self.side_thread(cx, light, t, &pid, &string(t, "project"), false, &mut ctl));
        }
        if let Some(n) = self.data["activeSettled"]["count"].as_u64().filter(|n| *n > 0) {
            let open = self.data["activeSettled"]["open"].as_bool().unwrap_or(false);
            /* i18n-ok: a ctl label */ ctl.push(CtlEntry { id: "side-settled".into(), label: "Settled".into(), ctl: Ctl::Node("side-settled".into(), String::new()) });
            rows.push(with_rect(div().id("side-settled"), || "side-settled".to_string()).flex().items_center().gap_2().px_3().py_1().mt_2().text_size(crate::scale::tpx(11.)).text_color(c("accent")).cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim")).child(tf("gpui.b.settled.active", &[("n", n.into())]))
                .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("side-settled".into(), String::new())); cx.notify(); })).into_any_element());
            for t in self.data["activeSettled"]["rows"].as_array().into_iter().flatten() {
                let pid = string(t, "projectId"); let pid = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
                rows.push(self.side_thread(cx, light, t, &pid, &string(t, "project"), true, &mut ctl));
            }
        }
        } // (not sectioned)
        for page in self.data["sidebarPages"].as_array().into_iter().flatten().filter(|_| !sectioned) { // (the projects' pagers: the sections page nothing)
            for key in ["projects", "active", "legacy"] { if let Some(row) = pager_row(cx, format!("pg-{key}"), &page[key], light, &mut ctl) { rows.push(row); } }
        }
        // Chats with no project: the old Scratch chats, the last fold of the list and drawn as one (the head of Quiet or Archived: it is the same kind of
        // thing, what is left once the projects are listed). Nothing starts here; a chat that wants attention always has its row, the rest open from this
        // row or from a search. It was "Conversation history" in the accent colour, which said neither what it holds nor why it is apart
        let history = &self.data["conversationHistory"];
        if history["toggle"] == true {
            let (n, open) = (history["count"].as_u64().unwrap_or(0), history["open"] == true);
            /* i18n-ok: a ctl label */ ctl.push(CtlEntry { id: "side-history".into(), label: "Chats with no project".into(), ctl: Ctl::Node("flag".into(), "conversation-history".into()) });
            rows.push(with_rect(div().id("side-history"), || "side-history".to_string()).flex().items_center().gap_2().pl(px(6.)).pr_3().pt_3().pb_1().text_size(crate::scale::tpx(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                .tooltip(crate::keys::tip(&crate::i18n::trs("Old chats that belong to no project. New ones start in a project."), "", light))
                .child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim")).child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().child(tf("gpui.b.history.count", &[("n", n.into())]).to_uppercase()))
                .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("flag".into(), "conversation-history".into())); cx.notify(); })).into_any_element());
        }
        for t in self.data["legacy"].as_array().into_iter().flatten() { rows.push(self.side_thread(cx, light, t, "", "", false, &mut ctl)); }
        // the Older group of projects (no live thread, quiet for a while): a row that opens and shuts it
        if let Some(n) = self.data["olderProjects"]["count"].as_u64().filter(|_| !sectioned) { // (the sections have Quiet instead)
            let open = self.data["olderProjects"]["open"].as_bool().unwrap_or(false);
            /* i18n-ok: a ctl label */ ctl.push(CtlEntry { id: "side-older".into(), label: "Older projects".into(), ctl: Ctl::Node("side-older".into(), String::new()) });
            rows.push(with_rect(div().id("side-older"), || "side-older".to_string()).flex().items_center().gap_2().px_3().py_1().text_size(crate::scale::tpx(11.)).text_color(c("accent")).cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(ic(light, if open { "chevron-down" } else { "chevron-right" }, 14., "dim")).child(tf(if open { "gpui.b.older.open" } else { "gpui.b.older" }, &[("n", n.into())]))
                .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(Command::Action("side-older".into(), String::new())); cx.notify(); })).into_any_element());
        }
        /* i18n-ok: compared below; drawn through trs */ for name in ["BOTS", "ROOMS"] {
            /* i18n-ok: ui: ids */ let (plus_id, plus_action) = if name == "BOTS" { ("ui:New bot", "bot-new") } else { ("ui:New room", "room-new") };
            ctl.push(CtlEntry { id: plus_id.into(), label: plus_id.trim_start_matches("ui:").into(), ctl: Ctl::Node(plus_action.into(), String::new()) });
            rows.push(div().flex().justify_between().px_3().pt_4().text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(trs(name))
                .child(with_rect(div().id(plus_id), || plus_id.to_string()).cursor_pointer().on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(plus_action.into(), String::new())); cx.notify(); })).child(ic(light, "plus", 16., "dim"))).into_any_element());
            if name == "BOTS" {
                for ((bname, mood, note, _, bid, bsel), rig) in bot_data.iter().zip(bot_rigs.iter()) {
                    let note_color = match mood.as_str() { "wait" | "error" => c("bad"), "work" | "talk" => c("accent"), _ => c("faint") };
                    ctl.push(CtlEntry { id: format!("bot:{bname}"), label: bname.clone(), ctl: Ctl::Node("bot".into(), bid.clone()) });
                    let bid2 = bid.clone();
                    rows.push(with_rect(div().id(SharedString::from(format!("bot-{bid}"))), || format!("bot:{bname}")).flex().items_center().gap_2().px_3().py(px(3.)).cursor_pointer().when(*bsel, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
                        .on_click(cx.listener(move |this, _, _, cx| this.go("bot", bid2.clone(), cx)))
                        .when_some(row_menus.get(bid).cloned(), |d, menu| d.on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| { let items = crate::screens::ctxmenu::section_items(&menu); if !items.is_empty() { this.ctx_open(e.position, items, cx); } })))
                        .child(art::cat(rig.clone(), mood, 28., started, if art::rects_on() { format!("cat:{bname}") } else { String::new() }))
                        .child(div().flex().flex_col().min_w_0().line_height(crate::scale::tpx(18.)).child(div().overflow_hidden().whitespace_nowrap().child(bname.clone()))
                            .child(div().flex().gap_2().items_center().child(div().text_size(crate::scale::tpx(11.)).text_color(note_color).child(trs(note))).children(marks.get(bid).and_then(|m| crate::screens::badge::machine_mark(light, m, false, bid))))).into_any_element());
                }
            } else {
                for r in self.data["rooms"].as_array().into_iter().flatten() {
                    let (rid, rname, sel) = (string(r, "id"), string(r, "name"), r["sel"].as_bool().unwrap_or(false));
                    ctl.push(CtlEntry { id: format!("room:{rname}"), label: rname.clone(), ctl: Ctl::Node("room".into(), rid.clone()) });
                    let rid2 = rid.clone();
                    rows.push(with_rect(div().id(SharedString::from(format!("room-{rid}"))), || format!("room:{rname}")).flex().items_center().gap_2().px_3().py_1().cursor_pointer().when(sel, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
                        .on_click(cx.listener(move |this, _, _, cx| this.go("room", rid2.clone(), cx)))
                        .when_some(row_menus.get(&rid).cloned(), |d, menu| d.on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, _, cx| { let items = crate::screens::ctxmenu::section_items(&menu); if !items.is_empty() { this.ctx_open(e.position, items, cx); } })))
                        .child(div().text_color(c("faint")).child("#")).child(div().flex_1().overflow_hidden().whitespace_nowrap().child(rname.clone()))
                        .children(crate::screens::badge::machine_mark(light, &r["mark"], false, &rid))
                        .child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(format!("{}", r["members"]))).into_any_element());
                }
            }
        }
        self.screens.parts.side_ctl = ctl;
        /* i18n-ok: a key context */ div().id("sidebar").size_full().overflow_y_scroll().track_focus(&self.screens.side_focus).key_context("Sidebar").children(rows).into_any_element()
    }
    /// The viewer's page, a part of the window (part.rs): a board or schematic is drawn again only when the page itself changed, not for
    /// a hover in the sidebar or the caret's blink.
    pub(crate) fn viewer_part(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let light = self.light();
        crate::screens::xprobe::zoom_pending(self);
        let reg = self.screens.reg.clone(); reg.borrow_mut().begin_outside(); reg.borrow_mut().app = Some(cx.entity().downgrade());
        // the Backplane KiCad update at Edit: a row of its own above the pane (screens/kicad_update.rs), only while an editor is up
        let update = if self.screens.kicad.is_some() { crate::screens::kicad_update::view(self, cx) } else { None };
        self.screens.plots[0].loading.begin_frame(self.data["thread"]["id"].as_str().unwrap_or(""));
        let review_panel = crate::screens::mgmt::review_panel(self, cx);
        let thread = &self.data["thread"];
        let editor = self.kicad_for(thread, cx);
        let solid = self.screens.active_solid().cloned();
        let (pv, split) = self.screens.plots.views();
        let el = crate::screens::viewer::page(cx, &reg, light, &self.screens.files, &self.screens.images, self.screens.attacher.target.as_ref(), thread, &self.screens.plot, pv, split, solid.as_ref(), self.screens.browser.as_ref().map(|(_, b)| b), editor.as_ref(), update, review_panel, &self.screens.mgmt.checks);
        { let rows = &self.data["thread"]["entries"]; reg.borrow_mut().resolve_by(|id| rows.as_array().and_then(|r| r.iter().position(|e| e["id"].as_str() == Some(id)))); }
        // full screen: the editor alone, without the management panels' strip under it
        let el = if crate::screens::viewer::KICAD_SCREEN.load(std::sync::atomic::Ordering::Relaxed) { el }
            else { crate::screens::mgmt::host(self, el, window, cx) };
        self.screens.plots[0].loading.end_frame_and_arm(cx);
        el
    }
    /// The thread's messages, a part of the window (part.rs): a list that draws only the rows on screen.
    pub(crate) fn timeline_part(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if crate::screens::subview::wanted(&self.screens.panels, &self.data["thread"]).is_some() { self.screens.parts.rows_ctl.clear(); return self.sub_page(cx) }
        let light = self.light();
        let c = |name: &str| theme::color(light, name);
        self.screens.reg.borrow_mut().begin_rows(); self.screens.reg.borrow_mut().app = Some(cx.entity().downgrade()); self.screens.parts.rows_ctl.clear();
        crate::screens::xprobe::want_names(self); self.screens.reg.borrow_mut().design = self.screens.xprobe.design.clone();
        let thread = &self.data["thread"];
        let has_thread = thread.is_object();
        let no_rows = thread["entries"].as_array().map_or(true, |e| e.is_empty());
        div().id("timeline").size_full().relative().flex().flex_col()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e, w, cx| this.timeline_down(e, w, cx))).on_mouse_down(MouseButton::Right, cx.listener(|this, e, w, cx| this.timeline_down(e, w, cx)))
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, _| this.scroll_note(e)))
            .on_mouse_move(cx.listener(|this, e, _, cx| this.timeline_move(e, cx))).on_mouse_up(MouseButton::Left, cx.listener(|this, e, w, cx| this.timeline_up(e, w, cx))).on_mouse_up_out(MouseButton::Left, cx.listener(|this, e, w, cx| this.timeline_up(e, w, cx)))
            .children(if has_thread && self.selected.rsplit('|').next() == Some(self.screens.find.thread.as_str()) { crate::screens::find::bar(cx, light, &self.screens.find) } else { None })
            .when(!self.screens.reg.borrow().hover.is_empty(), |d| { let t = self.screens.reg.borrow().hover.clone(); d.child(div().absolute().left_0().bottom_0().max_w_full().px_2().py_px().bg(c("raise")).border_t_1().border_r_1().border_color(c("line")).text_size(crate::scale::tpx(11.)).text_color(c("dim")).overflow_hidden().whitespace_nowrap().child(t)) })
            .when(no_rows, |d| d.child(div().flex_1().flex().justify_center().items_end().child(div().w_full().max_w(px(930.)).px_4().py_6().text_color(c("dim")).child(if has_thread && thread["loading"] == true { trs("Loading…") } else if has_thread { trs("Start a conversation.") } else { trs("Pick a thread, or start one with + next to a project.") }))))
            .when(!no_rows, |d| d.child(list(self.timeline.clone(), cx.processor(|this, ix, _, cx| this.timeline_row(ix, cx))).flex_1().min_h_0()))
            .children(thread["live"].as_array().into_iter().flatten().map(|live| div().w_full().flex().justify_center().child(div().w_full().max_w(px(930.)).px_4().py_2().child(trs(&string(live, "text"))))))
            // after the list has drawn its rows: put them in reading order, and see whether history is wanted
            .child(canvas({ let (me, reg) = (cx.entity().downgrade(), self.screens.reg.clone()); move |_, _, cx| {
                let Some(this) = me.upgrade() else { return };
                { let rows = &this.read(cx).data["thread"]["entries"]; reg.borrow_mut().resolve_by(|id| rows.as_array().and_then(|r| r.iter().position(|e| e["id"].as_str() == Some(id)))); }
                cx.defer(move |cx| this.update(cx, |this, _| this.history_want()));
            } }, |_, _, _, _| {}).absolute().size_0())
            // far up a long thread: "Latest" goes back to the end (0.14 had the page's scrollbar); it shows and goes as the list is read
            .child(canvas({ let me = cx.entity().downgrade(); move |_, _, cx| {
                let Some(this) = me.upgrade() else { return };
                if this.read(cx).note_far() { cx.defer(move |cx| this.update(cx, |_, cx| cx.notify())); }
            } }, |_, _, _, _| {}).absolute().size_0())
            .when(!no_rows && self.far_from_end(), |d| d.child(div().absolute().bottom_3().left_0().right_0().flex().justify_center()
                .child(div().id("to-latest").flex().items_center().gap_1().px_3().py_1().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel")))
                    .text_size(crate::scale::tpx(12.)).text_color(c("text")).tooltip(crate::keys::tip("Go to the newest message", "", light)).child(ic(light, "chevron-down", 14., "dim")).child(trs("Latest"))
                    .on_click(cx.listener(|this, _, _, cx| { this.timeline_to_end(); cx.notify(); })))))
            .into_any_element()
    }
}

impl Render for Backplane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::scale::set(self.local.text_pct); // every text size this frame draws at goes through it (scale.rs)
        self.voice_watch(window.is_window_active()); // always on: the open thread and the window's focus, to the hub when they change
        self.screens.plots[0].layers_key = crate::keys::hint(&self.local, "layers");
        let t_render = std::time::Instant::now();
        if crate::perf::on() { crate::perf::frame_start(); cx.defer(|_| crate::perf::frame_end()); }
        crate::screens::improvements::tick(self, window, cx);
        // a 3D page not drawn this frame gives its pictures back to the GPU (they stayed in the atlas until the window closed)
        // and its old pictures leave the atlas once the GPU is done with them (SolidPage::collect)
        // the 3D pages' background follows the setting and the theme, live
        for page in self.screens.retired_solids.drain(..) { page.update(cx, |p, _| p.release(window)); }
        let back = self.local.viewer_backdrop(self.light()); back.make_current();
        if let Some(s) = self.screens.active_solid().cloned() { let (part, split) = (crate::screens::plotview::probed_part(self), self.data["thread"]["viewer"]["open"] == "split"); s.update(cx, |p, _| { p.set_backdrop(back); p.set_probe(&part); p.names_probe = !split; }); }
        for (slot, page) in &self.screens.solids {
            if page.read(cx).atlas_images() > 0 {
                let shown = self.screens.solid_slot.as_ref().is_some_and(|(active, _)| active == slot)
                    && crate::screens::viewer::shows_solid(&self.data["thread"]);
                page.update(cx, |p, _| { if !shown { p.put_away(); } p.collect(window); });
            }
        }
        crate::screens::desk::tick(self, window, cx);
        self.term_layout(window, cx);
        self.term_script(window, cx);
        if self.want_focus && self.screens.pad.is_none() { self.want_focus = false; self.input.focus_handle(cx).focus(window); }
        if std::mem::take(&mut self.screens.acct.script_menu) { self.combo_account(window, cx); }
        if self.models_pending { self.models_pending = false; self.picker_toggle(window, cx); }
        if let Some(w) = self.screens.projects.focus.take() { let e = if w == "folder" { self.screens.projects.folder.clone() } else { self.screens.projects.search.clone() }; if let Some(e) = e { window.focus(&e.focus_handle(cx)); } }
        if let Some(n) = self.screens.focus_field.take() { if let Some(s) = self.screens.fields.get(&n) { window.focus(&s.input.focus_handle(cx)); } }
        if self.screens.focus_settings { self.screens.focus_settings = false; window.focus(&self.screens.settings_focus); }
        if self.screens.setup.want_focus { self.screens.setup.want_focus = false; window.focus(&self.screens.setup_focus); }
        if self.screens.palette.focus { self.screens.palette.focus = false; if let Some(i) = self.screens.palette.input.clone() { window.focus(&i.focus_handle(cx)); } }
        if self.screens.find.focus { self.screens.find.focus = false; if let Some(i) = self.screens.find.input.clone() { window.focus(&i.focus_handle(cx)); } }
        self.frame_marks(window, cx);
        self.hints_frame(window, cx);
        let light = self.light();
        if self.swatches { return crate::swatch::view(light).into_any_element(); }
        if self.gallery { return crate::swatch::gallery(light).into_any_element(); }
        if self.fonts { return crate::swatch::fonts(light).into_any_element(); }
        if self.pair.is_some() { let (el, ctl) = crate::screens::pair::view(self, light, window, cx); self.controls = ctl; return el; }
        let c = |name: &str| theme::color(light, name);
        // the selectable text is the rows' part's and the viewer part's (part.rs); with the viewer shut, what its page drew is gone
        if !crate::screens::viewer::is_open(&self.data["thread"]) { self.screens.reg.borrow_mut().begin_outside(); }
        self.screens.images.frame();
        if self.input.read(cx).light != light { self.input.update(cx, |i, _| i.light = light); for i in self.screens.ask_inputs.values() { i.update(cx, |i, _| i.light = light); } }
        let thread = &self.data["thread"];
        let dictating = self.screens.voice.views.get(&self.selected).map_or(false, |v| v["on"] == true);
        let has_thread = thread.is_object();
        // how the window's width is shared: in a narrow window the thread gives way, then the viewer, then the sidebar folds (the rail stays)
        let (win_w, viewer_open) = (f32::from(window.viewport_size().width), crate::screens::viewer::is_open(thread));
        if crate::screens::viewer::fit(win_w, self.local.sidebar_px as f32, self.sidebar_open, false, viewer_open).0 { self.screens.side_force = false; } // room again: "anyway" is over
        let (side_shown, thread_least, pane_least, share) = crate::screens::viewer::fit(win_w, self.local.sidebar_px as f32, self.sidebar_open, self.screens.side_force, viewer_open);
        // the embedded editor at full size: the sidebar folds and the thread gives all its room (screens/viewer.rs pane)
        let kedit = viewer_open && self.screens.kicad.is_some() && self.screens.kicad_window.is_none() && matches!(thread["viewer"]["open"].as_str(), Some("board" | "schematic" | "3d"));
        // full screen: the editor and its strip are the whole window (no top bar, sidebar, thread, rail or pane bar)
        if !kedit { self.screens.kicad_screen = false; } // the editor left the pane: the next Edit starts in the pane, not over the window
        let kscreen = self.screens.kicad_screen && kedit;
        let kfull = kscreen || (self.screens.kicad_full && viewer_open && self.screens.kicad.is_some() && matches!(thread["viewer"]["open"].as_str(), Some("board" | "schematic" | "3d")));
        crate::screens::viewer::KICAD_SCREEN.store(kscreen, std::sync::atomic::Ordering::Relaxed);
        // whether the page holds the embedded editor this frame (popped out: its own window does), from this frame's state: macOS hides
        // KiCad's frame by it when the pane is not drawn (#1199: the viewer closed, the PCB Editor stayed on screen)
        if let Some(p) = self.screens.kicad.clone() {
            let in_page = self.screens.kicad_window.is_some() || (self.kicad_for(thread, cx).is_some() && crate::screens::viewer::holds_editor(thread));
            if p.read(cx).in_page != in_page { p.update(cx, |p, _| p.in_page = in_page); }
        }
        // Follow agent in KiCad: the hub learns whether this thread's page holds KiCad and which frame it shows (viewer.state, on change)
        let kshown = match self.screens.kicad.clone() { Some(p) if p.read(cx).in_page => (true, p.read(cx).tab()), _ => (false, String::new()) };
        if self.screens.kicad_shown != kshown { self.screens.kicad_shown = kshown; let me = cx.entity(); cx.defer(move |cx| me.update(cx, |this, _| crate::screens::plotview::report_state(this))); }
        let (side_shown, thread_least) = if kfull { (false, 0.) } else { (side_shown, thread_least) };
        crate::screens::viewer::KICAD_FULL.store(kfull, std::sync::atomic::Ordering::Relaxed); crate::screens::viewer::KICAD_POPPED.store(self.screens.kicad_window.is_some(), std::sync::atomic::Ordering::Relaxed);
        crate::screens::viewer::set_least(pane_least); crate::screens::viewer::set_share(share); self.screens.side_shown = side_shown;
        // an open panel at the design's side widens the pane at the thread's cost, down to the thread's least (screens/viewer.rs `with_panel`)
        crate::screens::viewer::set_most(win_w - if side_shown { self.local.sidebar_px as f32 } else { crate::screens::viewer::STRIP_W } - crate::screens::viewer::RAIL_W - thread_least);
        crate::screens::viewer::set_panel(if viewer_open { crate::screens::mgmt::side_room(thread, &self.screens.mgmt) } else { 0. });
        let mut ctl: Vec<CtlEntry> = Vec::new(); // the controls drawn this frame (the e2e harness's inventory)
        // project home replaces the thread when the hub selected it
        let home = &self.data["projectHome"];
        let page = self.project_page.clone().filter(|p| crate::screens::projects::find(&self.data, p).is_some());
        let has_home = home.is_object() || page.is_some();
        // the project home replaces the rows' part: what it drew last is not on screen (its controls, its text for selection)
        if has_home { self.screens.parts.rows_ctl.clear(); self.screens.reg.borrow_mut().begin_rows(); }
        // a project's page: a list, so only what is on screen is laid out and painted (Hubble_DK's README drawn whole: 135 ms a frame,
        // every hover); its controls are all listed for the drivers, drawn or not
        let page_list = if let (false, Some(p)) = (home.is_object(), page.as_ref()) {
            self.screens.home_doc_w = page_doc_w(&self.screens.home_list);
            let doc = crate::screens::timeline::Doc { imgs: &self.screens.images, target: self.screens.attacher.target.as_ref(), width: self.screens.home_doc_w };
            for part in [crate::screens::projects::Part::Head, crate::screens::projects::Part::Tail] { let _ = crate::screens::projects::page(cx, &self.screens.reg.clone(), light, &self.data, &self.screens.projects.home, p, &mut ctl, &doc, part); }
            let n = if self.screens.projects.home.pid == *p { self.screens.projects.home.readme.len() } else { 0 };
            let want = crate::screens::projects::items(n);
            let st = &self.screens.home_list;
            if self.screens.home_list_key != *p { st.reset(want); self.screens.home_list_key = p.clone(); }
            else if st.item_count() != want { let old = st.item_count(); st.splice(1..old.saturating_sub(1).max(1), n); }
            self.screens.home_anchors = crate::screens::projects::anchors(if n > 0 { &self.screens.projects.home.readme } else { &[] });
            true
        } else { self.screens.home_list_key.clear(); false };
        let home_els: Vec<AnyElement> = if page_list { Vec::new() } else if has_home {
            home["cards"].as_array().into_iter().flatten().take(60).enumerate().map(|(i, k)| node_el(cx, k, light, 0, &format!("h{i}"), &mut ctl)).collect()
        } else { Vec::new() };
        // popup
        // the popup is the hub's: rows are shown when thread.skills is non-empty (no client-side rows)
        let skills = thread["skills"].as_array();
        // the highlighted row starts at the first for each new set of rows; the composer is told so Up/Down/Tab drive the popup while it shows
        { let key = self.popup_rows().join("\u{1f}"); if key != self.screens.popup_key { self.screens.popup_key = key; self.screens.popup_ix = 0; self.screens.popup_scroll.scroll_to_item(0); } }
        let popup_ix = self.screens.popup_ix.min(skills.map_or(0, |s| s.len()).saturating_sub(1));
        // eight rows show; the rest scroll, and the highlighted row is kept in view (glue.rs)
        let mut popup = div().id("popup-list").track_scroll(&self.screens.popup_scroll).max_h(px(8. * 28.)).overflow_y_scroll().flex().flex_col().py_1();
        let mut group = String::new();
        for (index, skill) in skills.into_iter().flatten().enumerate() {
            // a source's rows sit under its name (the hub groups them; rows without one draw as before); the name is drawn in its first row's box,
            // so the list keeps one child per row and scroll_to_item(popup_ix) lands on the highlighted row
            let g = skill.get("group").and_then(Value::as_str).unwrap_or("").to_string();
            let head = (!g.is_empty() && g != group).then(|| div().px_3().pt_1().text_xs().text_color(c("dim")).child(trs(&g)));
            group = g;
            let word = skill.get("word").or_else(|| skill.get("name")).and_then(Value::as_str).unwrap_or("").to_string();
            let label = skill.get("label").and_then(Value::as_str).unwrap_or(&word).to_string();
            let desc = ["text", "hint", "desc", "detail", "about"].iter().find_map(|k| skill.get(*k).and_then(Value::as_str)).unwrap_or("").to_string();
            /* i18n-ok: brackets round the hub's note, drawn through trs */ let desc = match skill.get("note").and_then(Value::as_str) { Some(n) if !n.is_empty() => format!("[{}] {desc}", trs(n)), _ => desc };
            ctl.push(CtlEntry { id: format!("skill:{word}"), label: label.clone(), ctl: Ctl::Skill(word.clone()) });
            popup = popup.child(div().flex().flex_col().children(head).child(with_rect(div().id(("skill", index)), || format!("skill:{word}")).flex().gap_3().px_3().py_1().cursor_pointer().when(index == popup_ix, |d| d.bg(c("raise"))).hover(|s| s.bg(c("raise")))
                .child(div().font_weight(FontWeight::BOLD).flex_shrink_0().child(label)).child(div().flex_1().w_0().overflow_hidden().whitespace_nowrap().text_color(c("dim")).child(desc))
                .on_click(cx.listener(move |this, _, window, cx| {
                    let _ = this.tx.try_send(Command::Action("skill".into(), word.clone())); this.await_completion(cx);
                    this.input.focus_handle(cx).focus(window); cx.notify();
                }))));
        }
        // the reference words the hub has offered become chips in the draft (it knows them as models, bots, threads...)
        { let fresh: Vec<String> = skills.into_iter().flatten().filter_map(|s| s.get("word").and_then(Value::as_str)).filter(|w| w.len() > 1 && w.starts_with(|c| matches!(c, '@' | '>' | '%' | '#' | '&' | '~' | '$')) && !w.starts_with("$install")).map(String::from).collect();
          let mut changed = false;
          for w in fresh { if !self.screens.known_refs.contains(&w) { self.screens.known_refs.push(w); changed = true; } }
          if changed { self.screens.known_refs.sort_by_key(|w| std::cmp::Reverse(w.len())); self.screens.known_refs.truncate(300); let k = self.screens.known_refs.clone(); self.input.update(cx, |i, cx| { i.chips = k; cx.notify(); }); } }
        let shut = self.screens.popup_off.as_deref().map_or(false, |d| d == self.input.read(cx).content.as_ref());
        if !shut { self.screens.popup_off = None; }
        self.screens.popup_shut = shut;
        let show_popup = !shut && skills.map_or(false, |s| !s.is_empty());
        if self.input.read(cx).popup_open != show_popup { self.input.update(cx, |c, _| c.popup_open = show_popup); }
        let (pv, pm, py) = (f32::from(self.screens.popup_scroll.bounds().size.height), f32::from(self.screens.popup_scroll.max_offset().height), -f32::from(self.screens.popup_scroll.offset().y));
        let popup_bar = crate::screens::picker::thumb(pv, pv, pm, py).map(|(top, h)| div().absolute().top(px(top)).right(px(2.)).w(px(6.)).h(px(h)).bg(c("faint")).opacity(0.7));
        // chips
        // Thread permissions are read-only here and are not drawn in the composer: the web's composer chip beside the model chip is the account chip.
        /* i18n-ok: parses the hub's picker label */ let ctx = string(&thread["picker"], "label").split("ctx ").nth(1).map(|s| format!("ctx {}", s.split(' ').next().unwrap_or(""))).unwrap_or_default();
        // the one chip (screens/combo.rs): the row's width last frame, less its other controls, is what the chip may use
        let bar_w = self.screens.acct.bar_w.get();
        // infinite context (the hub's bots/recall/progress.rs): the context chip draws the memory's bar and its words instead of the session's context
        let memory = self.data["memory"].clone(); let mem_on = memory.is_object();
        let (tok, ctx) = if mem_on { (crate::i18n::trs(&string(&memory, "line")), crate::i18n::trs(&string(&memory, "short"))) } else { (string(&self.data, "tokChip"), ctx) };
        let (combo_avail, tok_words) = crate::screens::combo::split(if mem_on && bar_w > 0. { bar_w - crate::screens::combo::MEM_BAR } else { bar_w }, &thread["combo"], &tok, &ctx);
        // the thread's header (screens/head.rs): its state and tools beside the name in the top bar when it has the room, else a row of their own
        // a project's page over a thread: the header is the page's (its project's name), not the thread's
        // a swarm's page over the thread its viewer is drawn for: the page is what is open, so that thread has no header
        let head_thread = has_thread && !has_home && !crate::screens::swarm::over_thread(&self.data);
        crate::keys::set_tip_font(&self.local.font);
        let head = if head_thread { self.screens.head.layout(window, f32::from(window.viewport_size().width), &string(thread, "title"), &crate::screens::tools::tools(thread), &self.local.font, 13. * self.local.text_pct as f32 / 100.) } else { Default::default() };
        let (mut top_mid, mut head_row): (Option<AnyElement>, Option<AnyElement>) = (None, None);
        if head_thread {
            let cells = crate::screens::panels::cells(cx, light, &self.screens.panels, thread);
            let st = crate::screens::head::state(cx, light, thread, cells, self.screens.head.state.clone());
            let tl = crate::screens::head::strip(cx, light, thread, &self.local, head.icons);
            let menu = { let items = crate::screens::tools::context_items(thread); cx.listener(move |this: &mut Self, e: &MouseDownEvent, _, cx| { if !items.is_empty() { this.ctx_open(e.position, items.clone(), cx); } }) };
            if head.one { top_mid = Some(with_rect(div().id("thread-head-top"), || "pane-row-reference".into()).flex_1().min_w_0().h(crate::scale::tpx(crate::screens::head::ROW_H)).overflow_hidden().flex().items_center().mx(px(crate::screens::head::GAP)).on_mouse_down(MouseButton::Right, menu).child(st).child(div().flex_1().min_w(px(crate::screens::head::GAP))).child(tl).into_any_element()); }
            else { head_row = Some(with_rect(div().flex_shrink_0(), || "pane-row-reference".into()).child(div().id("thread-head").h(crate::scale::tpx(crate::screens::head::ROW_H)).flex_shrink_0().flex().items_center().relative().pr(px(crate::screens::head::ROW_END)).border_b_1().border_color(c("rule")).child(crate::screens::head::measure(cx, self.screens.head.row.clone())).on_mouse_down(MouseButton::Right, menu).child(st).child(div().flex_1().min_w(px(crate::screens::head::GAP))).child(tl)).into_any_element()); }
        }
        // the crumb after Backplane: the thread's project, or the project whose page is open, by name
        let crumb = match page.as_ref().and_then(|p| crate::screens::projects::find(&self.data, p)).map(|p| string(p, "title")).filter(|t| !t.is_empty()) {
            Some(t) => t, None if head_thread => { let t = string(&thread["project"], "title"); if t.is_empty() { trs("Project") } else { t } } None => trs("Projects") };
        let mut rail_pages = Vec::new();
        let mut rail_tools = Vec::new();
        // held Ctrl: each rail button's bare key in its corner (the old rail's keycaps), lit on the view on show
        let keycaps = self.screens.held == crate::keys::Held::Threads;
        let keycap = move |k: &str, lit: bool| (keycaps && k.chars().count() == 1).then(|| crate::screens::hints::cap(light, &k.to_uppercase(), lit).absolute().top(px(3.)).right(px(3.)));
        crate::screens::viewer::follow(&self.tx, thread);
        let active_kind = if thread["diff"].is_object() { "diff".to_owned() } else { crate::screens::viewer::active_kind(&thread["viewer"], self.screens.rail_pending.as_deref()) };
        // the rail's Simulation entry (the hub's word, in a window that can hold KiCad)
        let can_embed = crate::viewers::kicad_pane::local(self.screens.attacher.target.as_ref().map(|t| t.addr.as_str()), thread);
        let sim_rail = crate::viewers::kicad_pane::rail_simulation(&thread["viewer"], can_embed).is_some();
        let rows = rail_rows(&thread["viewer"], sim_rail);
        // when the local embedded KiCad pane is on show in this thread, light the rail from its wanted frame (`KicadPane::tab()`,
        // which can be ahead of KiCad while `pending_show` is set), provided the mapped entry exists and is enabled in `rows`
        let pane_frame = can_embed.then(|| self.kicad_for(thread, cx).map(|p| p.read(cx).tab())).flatten();
        let kicad_on = kicad_lit_id(&rows, can_embed, crate::screens::viewer::holds_editor(thread), thread["diff"].is_object(), pane_frame.as_deref());
        let mut previous_group = None;
        let mut page_top = px(0.);
        let mut selected_row = None;
        let collect_inv = crate::art::rects_on();
        let (mut inv_page, mut inv_tool): (Vec<RailInvRow>, Vec<RailInvRow>) = (Vec::new(), Vec::new());
        let emit_rail = rail_rects_on();
        let (mut m_page_lit, mut m_page_entries, mut m_page_disabled): (Vec<&str>, Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new(), Vec::new());
        let (mut m_tool_lit, mut m_tool_entries, mut m_tool_disabled): (Vec<&str>, Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new(), Vec::new());
        let labels: Vec<_> = rows.iter().map(|row| trs(row.label)).collect();
        let (label_size, ellipsis) = self.screens.rail_labels.size(&labels, &self.local.font, f32::from(crate::scale::tpx(11.)), window);
        for (row, label) in rows.into_iter().zip(labels) {
            let is_tool = row.group == "tools";
            let rail = if is_tool { &mut rail_tools } else { &mut rail_pages };
            if !is_tool {
                if previous_group.is_some_and(|g| g != row.group) {
                    rail.push(div().h(px(12.)).flex_shrink_0().into_any_element());
                    page_top += px(12.);
                }
                previous_group = Some(row.group);
            }
            let id = row.id;
            let disabled = row.state == "disabled";
            let active = !disabled && match id {
                "terminal" => self.screens.term.open,
                _ => viewer_row_on(&row, kicad_on, self.screens.rail_pending.as_deref(), &active_kind),
            };
            if !is_tool {
                if active { selected_row = Some((row.id, page_top)); }
                page_top += crate::scale::tpx(48.);
            }
            if emit_rail {
                let (lit, entries, dis) = if is_tool { (&mut m_tool_lit, &mut m_tool_entries, &mut m_tool_disabled) } else { (&mut m_page_lit, &mut m_page_entries, &mut m_page_disabled) };
                entries.push(row.label);
                if disabled { dis.push(row.label); }
                if active { lit.push(row.label); }
            }
            let icon_resolved = art::icon_name(row.icon);
            let mut button = with_rect(div().id(SharedString::from(format!("rail-{}", row.label))), || format!("rail:{}", row.label))
                .relative().flex().flex_col().items_center().h(crate::scale::tpx(48.)).flex_shrink_0().justify_center().gap_1().text_size(px(label_size))
                .border_l_2().border_color(gpui::transparent_black())
                .when(active, |d| d.bg(c("raise")).border_color(c("accent")))
                .text_color(c(if active { "text" } else if disabled { "rail-disabled" } else { "dim" }))
                .child(ic(light, icon_resolved.unwrap_or("folder"), 18., if active { "accent" } else if disabled { "rail-disabled-icon" } else { "dim" })).child(div().max_w_full().when(ellipsis, |d| d.whitespace_nowrap().overflow_hidden().text_ellipsis()).child(label));
            if disabled {
                button = button.tooltip(crate::keys::tip(row.why, "", light));
                if collect_inv {
                    (if is_tool { &mut inv_tool } else { &mut inv_page }).push(RailInvRow {
                        id: id.into(), label: trs(row.label), group: row.group.into(), state: "disabled".into(), disabled: true,
                        icon: row.icon.into(), icon_known: icon_resolved.is_some(),
                        tooltip_label: trs(row.why), tooltip_hint: String::new(),
                        key: row.key.into(), keycap_drawn: String::new(), keycap_eligible: false,
                    });
                }
            } else {
                let letter = row.key;
                let hint = rail_hint(&self.local, id, letter);
                let cap_el = keycap(letter, active);
                if collect_inv {
                    let cap_drawn = if cap_el.is_some() { letter.to_uppercase() } else { String::new() };
                    (if is_tool { &mut inv_tool } else { &mut inv_page }).push(RailInvRow {
                        id: id.into(), label: trs(row.label), group: row.group.into(), state: (if active { "on" } else { "off" }).into(), disabled: false,
                        icon: row.icon.into(), icon_known: icon_resolved.is_some(),
                        tooltip_label: trs(row.label), tooltip_hint: hint.clone(),
                        key: letter.into(), keycap_drawn: cap_drawn, keycap_eligible: letter.chars().count() == 1,
                    });
                }
                button = button.cursor_pointer().hover(|s| s.bg(c("raise")).text_color(c("text")))
                    .tooltip(crate::keys::tip(row.label, &hint, light)).children(cap_el);
                let action = match id {
                    "terminal" => Ctl::ToggleTerminal, "commands" => Ctl::OpenPalette, "simulation" => Ctl::KicadAct("simulation".into()),
                    "diff" => Ctl::Node("diff".into(), "".into()), _ => Ctl::Rail(id.into()),
                };
                ctl.push(CtlEntry { id: format!("rail:{}", row.label), label: row.label.into(), ctl: action });
                let id = id.to_owned();
                button = button.on_click(cx.listener(move |this, _, window, cx| match id.as_str() {
                    "terminal" => this.term_toggle(window, cx), "commands" => this.palette_open(cx),
                    "simulation" => this.open_simulation(cx), "diff" => this.rail_press("diff", "", cx),
                    kind => this.rail_press("view-kind", kind, cx),
                }));
            }
            rail.push(button.into_any_element());
        }
        if collect_inv {
            inv_page.extend(inv_tool);
            emit_rail_inventory_if_changed(self.started, &self.data, can_embed, &self.local, inv_page);
        }
        let selected = selected_row.map(|(id, _)| id);
        let reveal = if !kscreen && self.screens.rail_last_lit.as_deref() != selected {
            self.screens.rail_last_lit = selected.map(str::to_owned);
            selected_row.map(|(_, top)| top)
        } else { None };
        if emit_rail {
            m_page_lit.extend(m_tool_lit);
            m_page_entries.extend(m_tool_entries);
            m_page_disabled.extend(m_tool_disabled);
            if let Some(snap) = rail_metric_change(&mut self.screens.rail_metric, &m_page_lit, &m_page_entries, &m_page_disabled) {
                crate::script::metric(self.started, "rail", serde_json::json!({ "rail": snap }));
            }
        }
        // the open asks' buttons are controls too (the harness answers them by index: ask:<i>:<j>)
        for (i, a) in crate::screens::asks::parse(&thread["asks"]).into_iter().enumerate() {
            for (j, b) in a.buttons.iter().enumerate() { ctl.push(CtlEntry { id: format!("ask:{i}:{j}"), label: b.label.clone(), ctl: Ctl::Node("answer".into(), b.value.clone()) }); }
            if let Some(s) = &a.skip { ctl.push(CtlEntry { id: format!("ask:{i}:skip"), label: s.label.clone(), ctl: Ctl::Node("answer".into(), s.value.clone()) }); }
            // a "Backplane needs" card's typed box and its Continue (the harness clicks the box and types, as a person does)
            if a.needs && !a.pre.is_empty() { ctl.push(CtlEntry { id: format!("ask:{i}:box"), label: a.placeholder.clone(), ctl: Ctl::None }); ctl.push(CtlEntry { id: format!("ask:{i}:go"), label: a.go.clone(), ctl: Ctl::None }); }
        }
        // the review bar's button (#1305): Allow edits, or Read-only again
        ctl.extend(crate::screens::review::control(&thread["review"]));
        // the header's tools (Buy it is tool:fab-buy) and the Order card's Buy now are controls too
        for t in crate::screens::tools::tools(thread) { ctl.push(CtlEntry { id: format!("tool:{}", t.action), label: t.label.clone(), ctl: Ctl::Node(t.action.clone(), t.value.clone()) }); }
        /* i18n-ok: registered controls the e2e drivers press */ if let Some(o) = crate::screens::order::parse(&thread["order"]).filter(crate::screens::order::payable) { ctl.push(CtlEntry { id: "ui:Buy now".into(), label: if o.label.is_empty() { "Buy now".into() } else { o.label }, ctl: Ctl::None }); }
        /* i18n-ok: registered controls the e2e drivers press */ if crate::screens::order::parse(&thread["order"]).is_some_and(|o| o.requote) { ctl.push(CtlEntry { id: "ui:Re-quote".into(), label: "Re-quote".into(), ctl: Ctl::None }); }
        // the Mechanical page's parts are controls too (the harness presses them by index)
        /* i18n-ok: registered controls the e2e drivers press */ for (i, p) in crate::screens::viewer::mech_parts(&thread["viewer"]).into_iter().enumerate() { ctl.push(CtlEntry { id: format!("mech-part-{i}"), label: p.label, ctl: Ctl::Node("mech-part".into(), p.value) }); }
        // the viewer bar's Open in KiCad, and the Subagents panel's row buttons (Release, Cancel, Stop: `<act>-<value>`, as the rows name them)
        /* i18n-ok: registered controls the e2e drivers press */ if crate::screens::viewer::is_open(thread) && crate::screens::viewer::has_design(&thread["viewer"]) { let alt = crate::screens::viewer::is_altium(&thread["viewer"]); ctl.push(CtlEntry { id: if alt { "ui:Open in Altium" } else { "ui:Open in KiCad" }.into(), label: if alt { "Open in Altium" } else { "Open in KiCad" }.into(), ctl: Ctl::Node("kicad-open".into(), crate::screens::viewer::shown_key(&thread["viewer"])) });
            /* i18n-ok: registered controls the e2e drivers press */ if crate::viewers::kicad_pane::local(self.screens.attacher.target.as_ref().map(|t| t.addr.as_str()), thread) { ctl.push(CtlEntry { id: "ui:Edit".into(), label: "Edit".into(), ctl: Ctl::KicadEdit(true) }); if thread["viewer"]["gerbers"] == true && thread["viewer"]["gerberFiles"].as_array().is_some_and(|a| !a.is_empty()) && crate::viewers::kicad_pane::GERBVIEW.load(std::sync::atomic::Ordering::Relaxed) { ctl.push(CtlEntry { id: "ui:Open in GerbView".into(), label: "Open in GerbView".into(), ctl: Ctl::KicadAct("gerbview".into()) }); } if self.screens.kicad.is_some() { /* i18n-ok: ctl ids the drivers press */ for (k, l) in crate::viewers::kicad_pane::TABS { ctl.push(CtlEntry { id: format!("kicad-tab:{k}"), label: (*l).into(), ctl: Ctl::KicadAct((*k).into()) }); } if self.screens.kicad.as_ref().is_some_and(|p| p.read(cx).failed()) { ctl.push(CtlEntry { id: "kicad:retry".into(), label: "Try again".into(), ctl: Ctl::KicadAct("retry".into()) }); } if self.screens.kicad.as_ref().is_some_and(|p| crate::viewers::kicad_pane::open_again(&p.read(cx).seen_info())) { ctl.push(CtlEntry { id: "kicad:open-again".into(), label: "Open the editor again".into(), ctl: Ctl::KicadAct("retry".into()) }); } for (a, l) in [("full", "Full size"), ("screen", "Full screen"), ("popout", "Pop out"), ("popin", "Pop back in")] { ctl.push(CtlEntry { id: format!("kicad:{a}"), label: l.into(), ctl: Ctl::KicadAct(a.into()) }); } } ctl.push(CtlEntry { id: "ui:View".into(), label: "View".into(), ctl: Ctl::KicadEdit(false) }); } }
        // the management panels' tabs, dock buttons and the open panel's rows (screens/mgmt.rs)
        for (id, label, a) in crate::screens::mgmt::controls(thread, &self.screens.mgmt) { ctl.push(CtlEntry { id, label, ctl: Ctl::Mgmt(a) }); }
        // the Diff page's Commit, PR and Revert (and the Revert question's two buttons), by the ids the page draws
        for (id, label, a, v) in crate::screens::viewer::diff_controls(&thread["diff"]) { ctl.push(CtlEntry { id: id.into(), label: label.into(), ctl: Ctl::Node(a.into(), v.into()) }); }
        // a Subagents row opens its own view (sub:<key>); with one open, its Back and its Stop
        for r in thread["subs"]["rows"].as_array().into_iter().flatten().filter(|r| r["detail"].is_object() && !string(r, "key").is_empty()) { ctl.push(CtlEntry { id: format!("subs:{}", string(r, "key")), label: string(r, "title"), ctl: Ctl::Sub(string(r, "key")) }); }
        /* i18n-ok: ctl labels and ui: ids the drivers press */ if crate::screens::subview::wanted(&self.screens.panels, thread).is_some() { ctl.push(CtlEntry { id: "ui:Back".into(), label: "Back to the thread".into(), ctl: Ctl::Sub(String::new()) });
            /* i18n-ok: ctl labels and ui: ids the drivers press */ let v = &thread["subView"]; if !string(v, "act").is_empty() { ctl.push(CtlEntry { id: "ui:Stop subagent".into(), label: string(v, "label"), ctl: Ctl::Node(string(v, "act"), string(v, "value")) }); } }
        for r in thread["subs"]["rows"].as_array().into_iter().flatten().filter(|r| !string(r, "act").is_empty()) { ctl.push(CtlEntry { id: format!("{}-{}", string(r, "act"), string(r, "value")), label: string(r, "label"), ctl: Ctl::Node(string(r, "act"), string(r, "value")) }); }
        // controls with no handler yet are registered too, so the harness finds them dead instead of missing
        /* i18n-ok: registered controls the e2e drivers press */ for (id, label, c) in crate::screens::tools::controls(&self.data) { ctl.push(CtlEntry { id, label, ctl: c }); }
        // the bot form, a bot's tabs and its Webhooks page are controls too (screens::bots::controls)
        /* i18n-ok: registered controls the e2e drivers press */ for (id, label, c) in crate::screens::bots::controls(&self.data).into_iter().chain(crate::screens::swarm::controls(&self.data)) { ctl.push(CtlEntry { id, label, ctl: c }); }
        /* i18n-ok: registered controls the e2e drivers press */ if has_thread { ctl.push(CtlEntry { id: "ui:Thread menu".into(), label: "Thread actions".into(), ctl: Ctl::ThreadMenu }); }
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Settings".into(), label: "Settings".into(), ctl: Ctl::Node("flag".into(), "settings".into()) });
        /* i18n-ok: registered controls the e2e drivers press */ if self.data["settings"].is_object() { for (page, title) in &self.screens.settings_ui.nav { ctl.push(CtlEntry { id: format!("set-nav:{page}"), label: title.clone(), ctl: Ctl::SettingsPage(page.clone()) }); } }
        // Settings > CAD's Install KiCad buttons (Install KiCad, Install, Cancel, Try again): `set:<action>`
        /* i18n-ok: registered controls the e2e drivers press */ for b in crate::screens::settings::parse(&self.data["settings"]).into_iter().flat_map(|r| r.buttons).filter(|b| b.action.starts_with("kicad-install")) { ctl.push(CtlEntry { id: format!("set:{}", b.action), label: b.label, ctl: Ctl::Node(b.action, b.value) }); }
        /* i18n-ok: registered controls the e2e drivers press */ if let Some(p) = self.data["projects"].as_array().and_then(|p| p.first()) { let id = string(p, "id"); let id = id.rsplit_once('|').map_or(id.as_str(), |(_, r)| r).to_string(); ctl.push(CtlEntry { id: "ui:New thread".into(), label: "New thread".into(), ctl: Ctl::Node("new-thread".into(), id) }); }
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "send".into(), label: "Send".into(), ctl: Ctl::Send });
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Context".into(), label: "Context".into(), ctl: Ctl::ToggleContext });
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Jump or run a command".into(), label: "Jump or run a command".into(), ctl: Ctl::OpenPalette });
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Account".into(), label: "Account".into(), ctl: Ctl::AccountMenu });
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Accounts".into(), label: "Accounts".into(), ctl: Ctl::AccountsPage });
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Dictate".into(), label: "Dictate".into(), ctl: Ctl::Dictate });
        crate::screens::improvements::controls(self, &mut ctl);
        /* i18n-ok: registered controls the e2e drivers press */ for (id, label, c) in crate::screens::projects::controls(&self.data) { ctl.push(CtlEntry { id, label, ctl: c }); }
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Search".into(), label: "Search".into(), ctl: Ctl::FocusSearch });
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Fold sidebar".into(), label: "Fold sidebar".into(), ctl: Ctl::ToggleSidebar });
        /* i18n-ok: registered controls the e2e drivers press */ ctl.push(CtlEntry { id: "ui:Attach".into(), label: "Attach".into(), ctl: Ctl::PickFiles });
        /* i18n-ok: registered controls the e2e drivers press */ if !side_shown { ctl.push(CtlEntry { id: "ui:Unfold sidebar".into(), label: "Unfold sidebar".into(), ctl: Ctl::ToggleSidebar }); }
        let connected = self.status == "Connected";
        if !self.screens.img_poll && self.screens.images.loading() { self.screens.img_poll = true; crate::screens::timeline::poll(window, cx); }

        let el = crate::keys::attach(div().size_full().flex().flex_col().bg(c("bg")),self.picker_keys(),cx)
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.attach_event(&crate::screens::composer::ComposerEvent::Files(paths.paths().to_vec()), cx))).text_color(c("text")).text_size(crate::scale::tpx(13.)).font_family(self.local.font.clone()).line_height(crate::scale::tpx(20.))
            .relative().child(div().absolute().size_0().track_focus(&self.screens.root_focus)).on_action(cx.listener(|this, _: &crate::OpenPaletteKey, _, cx| if !this.palette_close(cx) { this.palette_open(cx) })).on_action(cx.listener(|this, _: &crate::screens::improvements::Toggle, _, cx| crate::screens::improvements::toggle(this, cx))).on_action(cx.listener(|this, _: &crate::ToggleSidebarKey, _, cx| this.side_flip(cx))).on_action(cx.listener(|this, _: &crate::FocusSearchKey, _, cx| if !this.palette_close(cx) { this.palette_open_find(cx) })).on_action(cx.listener(|this, _: &crate::FindInThreadKey, _, cx| this.find_open(cx))).on_action(cx.listener(Self::send)).on_action(cx.listener(Self::close_popup)).on_action(cx.listener(Self::copy_selection))
            // the dev build strip, only while a build is being made or waits to relaunch
            .children(self.screens.devbar.clone().map(|b| crate::screens::devbar::strip(cx, &b, light, self.screens.devbar_confirm.is_some())))
            .children({ let s = std::mem::take(&mut self.screens.hubbar); let e = crate::screens::hubbar::strip(cx, &s, light); self.screens.hubbar = s; e })
            // a window on another computer's hub: a thin accent line along the top the whole time, and "on desktop" beside the name
            .when(self.screens.elsewhere.is_some(), |d| d.child(div().id("elsewhere-strip").h(px(2.)).flex_shrink_0().bg(c("accent"))))
            // top bar
            .child(div().h(crate::scale::tpx(36.)).flex_shrink_0().relative().child(art::probe("topbar")).children(crate::perf::mark("topbar")).flex().items_center().justify_between().px_3().border_b_1().border_color(c("rule")).bg(c("bg"))
                .when(kscreen, |d| d.h(px(0.)).border_b_0().overflow_hidden())
                .child(div().relative().min_w_0().flex().items_center().gap_2().child(crate::screens::head::measure(cx, self.screens.head.left.clone()))
                    .child(div().relative().size(px(18.)).child(art::logo(light, 18., "logo"))
                        .child(div().absolute().right(px(-2.)).bottom(px(-1.)).size(px(6.)).bg(if connected { c("ok") } else { c("bad") })))
                    .child(div().font_weight(FontWeight::BOLD).child("Backplane"))
                    // streamer mode is on: said while it lasts, with why when auto-detect turned it on; a click turns it off
                    /* i18n-ok: ui: ids (tips go through keys::tip, words through trs) */ .children(self.screens.streamer.why.clone().map(|why| with_rect(div().id("streamer-on"), || "ui:Streamer mode".to_string()).flex().items_center().gap(px(3.)).px_1().border_1().border_color(c("warn")).text_size(crate::scale::tpx(11.)).text_color(c("warn")).cursor_pointer()
                        .tooltip(crate::keys::tip("Streamer mode: emails, paths, machine names and codes are hidden. Click to turn it off.", "", light))
                        .on_click(cx.listener(|this, _, _, cx| this.streamer_toggle(cx)))
                        .child(ic(light, "radio", 11., "warn")).child(if why.is_empty() { trs("Streamer mode") } else { format!("{} · {}", trs("Streamer mode"), trs(&why)) })))
                    .children(self.screens.elsewhere.clone().map(|(short, full)| div().id("elsewhere").flex().items_center().gap(px(3.)).px_1().border_1().border_color(c("accent")).text_size(crate::scale::tpx(11.)).text_color(c("accent"))
                        .tooltip(crate::keys::tip_raw(&tf("gpui.b.window.on.machine", &[("machine", full.into())]), "", light)).child(ic(light, "link", 11., "accent")).child(tf("gpui.b.on.machine", &[("machine", short.into())]))))
                    .child(div().text_color(c("faint")).child("/")).child(div().text_color(c("dim")).whitespace_nowrap().child(crumb))
                    // another machine's thread: "Project · on desktop" (this machine's looks as always)
                    .when(head_thread, |d| d.children(crate::screens::badge::place(light, &thread["mark"], "", "crumb").map(|p| div().flex().items_center().gap_2().child(div().text_color(c("faint")).child("·")).child(p))))
                    // a buy thread's title is marked an Order with the cart, as 0.14's was (the hub sends its Order card only for a buy thread)
                    .when(head_thread && thread["order"].is_object(), |d| d.child(div().text_color(c("faint")).child("/")).child(div().id("order-mark").flex().items_center().gap(px(3.)).text_size(crate::scale::tpx(11.)).text_color(c("accent"))
                        .tooltip(crate::keys::tip("This thread orders the project's parts", "", light)).child(ic(light, "shopping-cart", 12., "accent")).child(trs("Order"))))
                    // a thread in review mode says so by its name too (#1305); the bar over the message box says why and switches it
                    .when(head_thread && thread["review"]["on"] == true, |d| d.child(div().text_color(c("faint")).child("/")).child(div().id("review-mark").flex().items_center().gap(px(3.)).text_size(crate::scale::tpx(11.)).text_color(c("warn"))
                        .tooltip(crate::keys::tip("Review mode: the design files are read-only for this thread", "", light)).child(ic(light, "search", 12., "warn")).child(trs("Read-only"))))
                    .when(head_thread, |d| d.child(div().text_color(c("faint")).child("/")).child(div().id("thread-title").relative().min_w_0().flex().font_weight(FontWeight::BOLD).child(crate::screens::head::measure(cx, self.screens.head.title.clone())).child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(string(thread, "title")))
                        // the name is not a button (Rename is in its menu): a right click on it, or the ▾ beside it, opens the thread's menu
                        .on_mouse_down(MouseButton::Right, cx.listener(|this, e: &MouseDownEvent, _, cx| { let items = crate::screens::ctxmenu::title_items(&this.data); if !items.is_empty() { this.ctx_open(e.position, items, cx); } })))
                        /* i18n-ok: ui: ids (tips go through keys::tip, words through trs) */ .child(with_rect(div().id("thread-menu"), || "ui:Thread menu".to_string()).px_1().h_full().flex().items_center().cursor_pointer().hover(|s| s.bg(c("raise")))
                            // the menu drops from under the ▾
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, cx| { cx.stop_propagation(); let items = crate::screens::ctxmenu::title_items(&this.data); if !items.is_empty() { this.ctx_open(point(e.position.x - px(8.), e.position.y + px(14.)), items, cx); } }))
                            .child(ic(light, "chevron-down", 14., "dim")))))
                .children(top_mid)
                // the window's own controls: Jump gives up room first (down to 120 px) on a narrow window, so Settings is never cut off
                .child(div().relative().min_w_0().flex().items_center().gap_3().child(crate::screens::head::measure(cx, self.screens.head.right.clone()))
                    /* i18n-ok: ui: ids (tips go through keys::tip, words through trs) */ .child(with_rect(div().id("palette-bar").cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.palette_open(cx))).tooltip(crate::keys::tip("Jump or run a command", &crate::keys::hint(&self.local, "palette"), light)), || "ui:Jump or run a command".to_string()).w(px(260.)).min_w(px(120.)).flex_shrink().overflow_hidden().px_2().flex().items_center().gap_2().border_1().border_color(c("line")).text_size(crate::scale::tpx(12.)).text_color(c("faint")).child(ic(light, "chevron-right", 13., "faint")).child(trs("Jump or run a command"))
                        // held Ctrl: its key at the end of the bar (0.14's "Ctrl K"), in room the bar always has
                        .when(self.screens.held == crate::keys::Held::Threads, |d| d.child(div().flex_1()).child(div().flex().gap(px(2.)).children(crate::keys::caps(&crate::keys::hint(&self.local, "palette")).iter().map(|k| crate::screens::hints::cap(light, k, false))))))
                    .child(with_rect(div().id("improvements-btn"), || "ui:Improvements".to_string()).cursor_pointer().on_click(cx.listener(|this, _, _, cx| crate::screens::improvements::toggle(this, cx))).px_2().flex().items_center().gap_2().border_1().border_color(c("line")).text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(ic(light, "message-square", 15., "dim")).child(trs("Improvements")))
                    .child(with_rect(div().id("search-btn"), || "ui:Search".to_string()).cursor_pointer().tooltip(crate::keys::tip("Filter projects and threads", "", light)).on_click(cx.listener(|this, _, _, cx| this.focus_search(cx))).child(ic(light, "search", 15., "dim"))).child(with_rect(div().id("gear"), || "ui:Settings".to_string()).cursor_pointer().on_click(cx.listener(|this, _, _, cx| { this.open_settings(cx); })).child(ic(light, "settings", 15., "dim")))))
            .children(if kscreen { None } else { crate::screens::strip::view(light, &self.data) })
            .child(div().flex_1().min_h_0().flex()
                // sidebar
                // folded: the 44 px strip, its unfold chevron where the fold chevron is (screens/siderow.rs)
                .when(!side_shown && !kscreen, |d| d.child(div().id("side-strip").w(px(44.)).h_full().flex_shrink_0().overflow_hidden().bg(c("bg")).border_r_1().border_color(c("rule")).flex().flex_col().children(self.side_strip(cx, light))))
                .child(div().w(px(if side_shown { self.local.sidebar_px as f32 } else { 0. })).overflow_hidden().h_full().flex_shrink_0().relative().child(art::probe("sidebar")).children(crate::perf::mark("sidebar")).bg(c("bg")).when(side_shown, |d| d.border_r_1()).border_color(c("rule")).flex().flex_col()
                    .child(crate::screens::hubpick::picker(self, cx, light, &mut ctl))
                    /* i18n-ok: ctl labels and ui: ids the drivers press */ .child(div().flex().items_center().gap_2().h(crate::scale::tpx(crate::screens::siderow::FOLD_ROW)).px_2().child(with_rect(div().id("fold-sidebar"), || "ui:Fold sidebar".to_string()).cursor_pointer().tooltip(crate::keys::tip("Fold the sidebar", &crate::keys::hint(&self.local, "sidebar"), light)).child(ic(light, "chevron-left", 16., "dim")).on_click(cx.listener(|this, _, _, cx| this.side_flip(cx)))).child(crate::screens::projects::search_box(light, &self.screens.projects)).child(with_rect(div().id("add-project"), || "ui:Add project".to_string()).cursor_pointer().on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(crate::screens::projects::open_cmd()); cx.notify(); })).child(ic(light, "plus", 16., "dim"))))
                    // linked machines: All, this one, each other one (the hub filters the list below)
                    .children(crate::screens::machines::switch(cx, light, &self.data["machines"], &mut ctl))
                    .child(self.screens.parts.el(Backplane::sidebar_part, StyleRefinement::default().flex_1().min_h_0(), cx))
                    .child(div().px_3().py_1().border_t_1().border_color(c("line")).text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(crate::i18n::trs(&self.status)))
                    // held Ctrl: the short legend over the sidebar's foot (0.14's keys-legend)
                    .when(self.screens.held == crate::keys::Held::Threads, |d| d.child(crate::screens::hints::legend(cx, light, &self.local))))
                // main
                .child(div().flex_1().min_w(px(thread_least)).when(kfull, |d| d.flex_none().w(px(0.)).overflow_hidden()).h_full().relative().child(art::probe("main")).children(crate::perf::mark("main")).flex().flex_col()
                    .children(crate::screens::bots::bar(cx, light, &self.data["bot"]))
                    .children(head_row)
                    .children(crate::screens::panels::dock(cx, light, &self.screens.fields, &self.screens.panels, thread))
                    .when(page_list, |d| d.child(list(self.screens.home_list.clone(), cx.processor(|this, ix, _, cx| this.page_item(ix, cx))).flex_1().min_h_0()))
                    .when(has_home && !page_list, |d| d.child(div().id("home").track_scroll(&self.screens.home_scroll).overflow_y_scroll().flex_1().min_h_0().flex().flex_col().items_center().child(div().w_full().flex_none().max_w(px(930.)).p_4().flex().flex_col().gap_3().children(home_els))))
                    // a list: only the rows on screen are drawn, however much of the thread is held, and it keeps the reader's place as rows land above them
                    .when(!has_home, |d| d.child(self.screens.parts.el(Backplane::timeline_part, StyleRefinement::default().flex_1().min_h_0(), cx)))
                    .when(!self.error.is_empty(), |d| d.child(div().px_5().py_2().text_color(c("bad")).child(crate::i18n::trs(&self.error))))
                    .when(!self.note.is_empty(), |d| d.child(div().px_5().py_2().text_color(c("dim")).child(crate::i18n::trs(&self.note))))
                    .child(div().flex_shrink_0().relative().child(art::probe("composer")).children(crate::perf::mark("composer")).border_t_1().border_color(c("rule")).flex().justify_center().child(div().w_full().max_w(px(930.)).px_4().py_3().flex().flex_col().gap_2()
                        .when(show_popup, |d| d.child(div().relative().border_1().border_color(c("line")).bg(c("panel")).child(popup).children(popup_bar)))
                        .children(crate::screens::review::view(cx, light, &thread["review"]))
                        .children(crate::screens::order::view(cx, light, &thread["order"]))
                        // #1304: the agent has not answered the person's mid-turn message (screens/unanswered.rs)
                        .children(crate::screens::unanswered::view(cx, light, &thread["unanswered"]))
                        .children(crate::screens::asks::view(cx, light, &self.screens.ask_inputs, &thread["asks"]))
                        .children(crate::screens::voice::pending_view(cx, light, &self.screens.voice, &self.selected.clone()))
                        .children(self.screens.voice.views.get(&self.selected).cloned().and_then(|v| crate::screens::voice::view(cx, light, &v, self.screens.voice.level, self.screens.voice.fix.clone().as_ref())))
                        .children(crate::screens::sidebox::btw(cx, light, &thread["btw"]))
                        .children(crate::screens::sidebox::secret(cx, light, &self.screens.fields, &thread["secretBox"]))
                        .children(reply_chip(cx, light, &thread["replying"]))
                        .children(crate::screens::attach::chips(cx, light, &self.screens.attacher.previews, thread))
                        // the messages waiting for the agent (screens/queue.rs)
                        .children(crate::screens::queue::strip(cx, light, thread, &self.screens.unsent.rows(thread["id"].as_str().unwrap_or(""))))
                        .child(div().id("composer-box").relative().child(canvas({ let at = self.screens.box_at.clone(); move |b, _, _| at.set(b) }, |_, _, _, _| {}).absolute().size_full()).border_1().border_color(c("accent")).bg(c("view")).px_3().py_2().min_h(px(60.)).cursor(CursorStyle::IBeam).on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { this.input.focus_handle(cx).focus(window); })).child(self.input.clone()))
                        // another machine's thread: where an agent sent from here runs
                        .children(if self.project_page.is_some() { None } else { crate::screens::badge::runs_text(&thread["mark"]).map(|r| div().id("runs-on").flex().items_center().gap(px(3.)).text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(ic(light, "link", 11., "dim")).child(r)) })
                        // a project's page: what the hub said when it would not do what the page asked (a New thread refused), one line by the box
                        .children((self.project_page.is_some() && !self.screens.projects.refused.is_empty()).then(|| div().id("page-error").text_size(crate::scale::tpx(11.)).text_color(c("bad")).child(self.screens.projects.refused.clone())))
                        // text a page's Send could not deliver while the box held new text: offered back, never merged (Restore puts it in the box, what was there is kept to restore)
                        .children(self.screens.projects.restore.clone().map(|t| { let preview: String = t.chars().take(60).collect();
                            /* i18n-ok: a ctl id the drivers press */ with_rect(div().id("restore-unsent"), || "ui:restore unsent".to_string()).flex().items_center().gap(px(6.)).text_size(crate::scale::tpx(11.)).text_color(c("dim")).cursor_pointer().hover(|s| s.text_color(c("text")))
                                .child(crate::i18n::tf("gpui.unsent.restore", &[("text", preview.into())]))
                                .on_click(cx.listener(move |this, _, _, cx| { let now = this.input.read(cx).content.to_string(); this.screens.projects.restore = if now.trim().is_empty() { None } else { Some(now) }; let t2 = t.clone(); this.input.update(cx, |i, cx| i.set(t2, cx)); cx.notify(); })) }))
                        // a project's page: where the thread this sends starts ("runs on desktop"), and the machine picker behind it (D6; the hub's `runsOn`, `newOn`)
                        .children(self.project_page.clone().and_then(|pid| crate::screens::projects::find(&self.data, &pid).map(|p| (pid, p.clone()))).filter(|(_, p)| !string(p, "runsOn").is_empty()).map(|(pid, p)| {
                            let items = crate::screens::ctxmenu::run_items(&pid, &p["newOn"]); let more = items.len() > 1;
                            for it in &items { ctl.push(CtlEntry { id: format!("run-on:{}", it.label), label: it.label.clone(), ctl: Ctl::Node(it.action.clone(), it.value.clone()) }); }
                            /* i18n-ok: a ctl id the drivers press */ div().flex().child(with_rect(div().id("runs-on-pick"), || "ui:runs on".to_string()).flex().items_center().gap(px(3.)).text_size(crate::scale::tpx(11.)).text_color(c("dim")).when(more, |d| d.cursor_pointer().hover(|s| s.text_color(c("text"))))
                                .child(ic(light, "link", 11., "dim")).child(trs(&string(&p, "runsOn"))).when(more, |d| d.child(ic(light, "chevron-down", 11., "dim")))
                                // (its own width, not the composer's: the menu opens at the words, under them or over them near the bottom edge)
                                .child(canvas({ let at = self.screens.runs_at.clone(); move |b, _, _| at.set(b) }, |_, _, _, _| {}).absolute().inset_0())
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| { if items.len() > 1 { let b = this.screens.runs_at.get(); this.ctx_open_from(b, window.viewport_size().height, items.clone(), "run-on", cx); } })))
                        }))
                        .child(div().relative().flex().gap_2().items_center().w_full().min_w_0()
                            // the row as drawn: its width sizes the chip's words; the menu floats from where it is (another frame when it moved under an open menu)
                            .child(canvas({ let (me, cell, at, open) = (cx.entity().downgrade(), self.screens.acct.bar_w.clone(), self.screens.pick.row.clone(), self.screens.picker_open); move |b, _, cx| {
                                let moved = at.get() != b; at.set(b);
                                let w = f32::from(b.size.width); let resized = (cell.get() - w).abs() >= 1.;
                                if resized { cell.set(w); }
                                if resized || (moved && open) { if let Some(this) = me.upgrade() { cx.defer(move |cx| this.update(cx, |_, cx| cx.notify())) } }
                            } }, |_, _, _, _| {}).absolute().size_full())
                            // the chip's menu (screens/combo.rs): out of the row's flow, floating over the thread from the row; hidden while something covers the page
                            .children((!self.chip_menu_covered()).then(|| crate::screens::usage::view(cx, light, self, &self.screens.ctx_chip, &self.screens.box_at, (f32::from(window.viewport_size().width), f32::from(window.viewport_size().height)))).flatten())
                            .children((!self.chip_menu_covered()).then(|| crate::screens::combo::menu(cx, light, &thread, self, bar_w, (f32::from(window.viewport_size().width), f32::from(window.viewport_size().height)))).flatten())
                            .child(crate::screens::bar::fit(with_rect(crate::screens::combo::chip(cx, light, thread, combo_avail, (&crate::keys::hint(&self.local, "models"), &crate::keys::hint(&self.local, "account")), &self.screens.pick.chip, self.screens.picker_open), || "ui:Model".to_string()))
                                // held Ctrl: the menu's key over the chip's end (0.14's "Ctrl M"), so the chip keeps its width
                                .when(self.screens.held == crate::keys::Held::Threads, |d| d.relative().child(div().absolute().right(px(4.)).top_0().bottom_0().flex().items_center().gap(px(2.)).children(crate::keys::caps(&crate::keys::hint(&self.local, "models")).iter().map(|k| crate::screens::hints::cap(light, k, false))))))
                            // (the Draw button is gone for now: screens/pad.rs stays, to come back redone)
                            .child(crate::screens::bar::fit(with_rect(chip(light, String::new()).id("ctx-chip"), || "ui:Context".to_string())).min_w_0().overflow_hidden().cursor_pointer().tooltip(crate::keys::tip("Tokens, cost and context: the thread's numbers", "", light)).on_click(cx.listener(|this, _, _, cx| { this.screens.ctx_open = !this.screens.ctx_open; cx.notify(); })).relative().flex().items_center().gap_2().when(self.screens.ctx_open, |d| d.border_color(c("accent"))).child(canvas({ let at = self.screens.ctx_chip.clone(); move |b, _, _| at.set(b) }, |_, _, _, _| {}).absolute().size_full()).child(ic(light, "gauge", 14., if self.screens.ctx_open { "accent" } else { "dim" }))
                                .children(mem_on.then(|| crate::screens::combo::mem_bar(light, &memory)))
                                // the hub's words for the thread's tokens, cost and context (0.14's token button), else the context alone
                                .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(tok_words)))
                            .child(crate::screens::bar::fit(with_rect(chip(light, String::new()).id("attach"), || "ui:Attach".to_string())).flex_shrink_0().cursor_pointer().on_click(cx.listener(|this, _, _, cx| this.pick_files(cx))).child(ic(light, "paperclip", 16., "dim")))
                            .children(crate::screens::voice::always_pill(cx, light, &self.screens.voice))
                            .child(crate::screens::bar::fit(with_rect(chip(light, String::new()).id("dictate"), || "ui:Dictate".to_string())).flex_shrink_0().cursor_pointer()
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.voice_down(cx))).on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.voice_up(cx))).on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| this.voice_up(cx))).child(ic(light, "mic", 16., if dictating { "accent" } else { "dim" })))
                            .child(crate::screens::bar::fit(with_rect(div().id("send"), || "send".to_string())).flex_shrink_0().px_4().py_1().bg(c(if crate::screens::voice::flashing(&self.screens.voice) { "ok" } else { "accent" })).text_color(c("on-accent")).cursor_pointer().child(crate::i18n::trs(&crate::screens::sidebox::send_label(thread))).on_click(cx.listener(|this, _, window, cx| this.send(&Send, window, cx)))))))
                    .children(crate::screens::bots::page(cx, light, &self.screens.fields, &self.data["bot"], self.screens.browser.as_ref().map(|(_, b)| b)))
                    .children(crate::screens::tools::questions(cx, light, &self.screens.fields, &self.data, side_shown))
                    .children(crate::screens::projects::view(cx, light, &self.screens.projects, &self.data))
                    .children(crate::screens::bots::forms(cx, light, &self.screens.fields, &self.data))
                    .children(crate::screens::swarm::form(cx, light, &self.screens.fields, &self.data))
                    .children(crate::screens::palette::view(cx, light, &self.data, &self.screens.palette, &self.local, &self.selected))
                    .children(crate::screens::images::lightbox(cx, self.screens.lightbox.as_ref(), window.scale_factor()))
                )
                .children(crate::perf::mark("viewer"))
                .children(crate::screens::viewer::is_open(thread).then(|| self.screens.parts.el(Backplane::viewer_part, StyleRefinement::default().flex_1().min_h_0(), cx)).and_then(|page| crate::screens::viewer::pane(cx, light, thread, page)))
                // right rail
                .child(div().w(px(72.)).h_full().flex_shrink_0().relative().child(art::probe("rail")).children(crate::perf::mark("rail")).border_l_1().border_color(c("rule")).flex().flex_col().py_2().when(kscreen, |d| d.w(px(0.)).border_l_0().py_0().overflow_hidden()).when(!kscreen, |d| d.child(rail_sections(self.screens.rail_scroll.clone(), reveal, rail_pages, rail_tools, c("rail-rule"))))))
            // the one-time telemetry notice (screens/tmnotice.rs)
            .children({ let (n, shown) = (self.screens.tm_notice.clone(), self.screens.tm_shown.clone()); crate::screens::tmnotice::strip(cx, n.as_ref(), &shown, light) })
            // the one-time notice that the hub no longer pushes on its own (#1171)
            .children({ let n = self.screens.gp_notice.clone(); crate::screens::tmnotice::gp_strip(cx, n.as_ref(), light) })
            .children(crate::perf::mark("over"))
            // Settings over the whole window: its dialog is sized from the window (settings::fit), so it must not sit in the thread column
            .children(crate::screens::settings::view(cx, light, &self.screens.fields, &self.data["settings"], &self.screens.settings_focus, &self.screens.settings_ui, &self.local, &self.screens.acct, crate::screens::settings::fit(f32::from(window.viewport_size().width), f32::from(window.viewport_size().height)),&self.hubs,self.screens.offers.on_home.as_deref()))
            .children(crate::screens::improvements::view(cx, light, &self.screens.imp))
            .children(crate::screens::improvements::indicator(cx, light, &self.screens.imp))
            .children({ let title = crate::screens::badge::from_machine(&self.data["thread"]["mark"], &string(&self.data["thread"], "title")); crate::screens::term::view(cx, window, light, &self.local, &title, &self.screens.term) })
            .children(crate::screens::padview::view(cx, window, light, &self.local, &mut self.screens.pad, &mut ctl))
            .children(crate::screens::onboard::view(cx, light, &self.screens.fields, &self.data["tour"], &self.screens.acct, &self.screens.setup_focus, &self.screens.setup.scroll, f32::from(window.viewport_size().width), &self.error))
            // a 0.14 home coming across: over setup and everything else of the window
            // the keys sheet, over setup too (? and F1 work there)
            .children(crate::screens::keysheet::view(cx, light, self.screens.keysheet, &self.local))
            // the comment window over the KiCad pane (Ctrl+;)
            .children({ if let Some(k) = self.screens.kcomment.as_mut().filter(|k| k.focus_pending) { k.focus_pending = false; window.focus(&k.input.focus_handle(cx)); eprintln!("GPUI_LOG {}", serde_json::json!({"kcomment": "rendered", "at_ms": crate::screens::kcomment::epoch_ms() as u64})); } crate::screens::kcomment::view(self, cx, light, window.viewport_size()) })
            .children(crate::screens::migrate::view(cx, light, &self.data["migrate"]))
            // the GitHub star suggestion, while the hub says it is out: a small card in the upper right corner, at a pause (screens/star.rs `due`)
            .children(self.star_up(cx).then(|| crate::screens::star::card(cx, self.screens.star.clone().as_ref(), light)).flatten())
            // the right-click menu of a sidebar row, over everything
            .children(self.screens.ctx.clone().map(|m| { for it in &m.items { ctl.push(CtlEntry { id: format!("ctx:{}", it.label), label: it.label.clone(), ctl: Ctl::Node(it.action.clone(), it.value.clone()) }); } crate::screens::ctxmenu::render(cx, light, &m) }))
            .children(crate::perf::on().then(|| crate::perf::overlay(light, &crate::keys::hint(&self.local, "perf"))))
            .into_any_element();
        self.controls = ctl;
        // the selection's ends as ordinals of the blocks known now (the rows' part does the same when it draws)
        { let rows = &self.data["thread"]["entries"]; self.screens.reg.borrow_mut().resolve_by(|id| rows.as_array().and_then(|r| r.iter().position(|e| e["id"].as_str() == Some(id)))); }
        if crate::perf::on() { crate::perf::built(); }
        self.render_us = t_render.elapsed().as_micros() as u64;
        el
    }
}

/// A thread row being dragged (the sidebar's reorder: dropped on another row of its shelf), and a project header being dragged.
#[derive(Clone)] pub struct DragRow { pub id: String, pub label: String }
#[derive(Clone)] pub struct DragProj { pub id: String, pub label: String }
/// What follows the pointer while dragging: the label in a small box.
pub struct Ghost(pub String);
impl Render for Ghost { fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div().px_3().py_1().bg(gpui::rgb(0x2a2f3a)).border_1().border_color(gpui::rgb(0x4a5060)).text_color(gpui::rgb(0xe6e8ee)).text_size(crate::scale::tpx(12.)).child(self.0.clone()) } }

/// The width a project page's README is laid out to: the list's, in the page's 930 px column less its padding.
pub fn page_doc_w(l: &ListState) -> f32 { (f32::from(l.viewport_bounds().size.width).min(930.) - 32.).max(300.) }

/// Built-button evidence for one right-rail entry, collected where the button is constructed
/// under `BACKPLANE_GPUI_RECTS` (inert in normal clients).
#[derive(Clone, Debug, PartialEq, Eq)]
struct RailInvRow {
    id: String,
    label: String,
    group: String,
    state: String,
    disabled: bool,
    icon: String,
    icon_known: bool,
    tooltip_label: String,
    tooltip_hint: String,
    key: String,
    keycap_drawn: String,
    keycap_eligible: bool,
}

fn rail_inventory_json(rows: &[RailInvRow], pal_map: &std::collections::HashMap<String, (String, String)>) -> Vec<Value> {
    rows.iter().map(|r| {
        let pal = pal_map.get(&r.id);
        serde_json::json!({
            "id": r.id,
            "label": r.label,
            "group": r.group,
            "state": r.state,
            "disabled": r.disabled,
            "icon": r.icon,
            "icon_known": r.icon_known,
            "tooltip": r.tooltip_label,
            "tooltip_hint": r.tooltip_hint,
            "key": r.key,
            "keycap": r.keycap_drawn,
            "keycap_eligible": r.keycap_eligible,
            "palette_label": pal.map(|(l, _)| l.as_str()).unwrap_or(""),
            "palette_shortcut": pal.map(|(_, s)| s.as_str()).unwrap_or(""),
        })
    }).collect()
}

/// Emit `GPUI_METRIC {"event":"rail_inventory",...}` only when the built rail buttons or
/// palette commands changed (compares raw rows + palette map before serializing JSON).
fn emit_rail_inventory_if_changed(started: std::time::Instant, data: &Value, can_embed: bool, local: &crate::local::Local, rows: Vec<RailInvRow>) {
    static LAST: std::sync::Mutex<Option<(Vec<RailInvRow>, Vec<(String, String, String)>)>> = std::sync::Mutex::new(None);
    let pal_map = crate::screens::palette::rail_commands_map(data, can_embed, local);
    let mut pal_key: Vec<(String, String, String)> = pal_map.iter().map(|(k, (l, s))| (k.clone(), l.clone(), s.clone())).collect();
    pal_key.sort();
    let mut g = LAST.lock().unwrap();
    if g.as_ref().is_some_and(|(prev_r, prev_p)| *prev_r == rows && *prev_p == pal_key) {
        return;
    }
    let entries = rail_inventory_json(&rows, &pal_map);
    *g = Some((rows, pal_key));
    drop(g);
    crate::script::metric(started, "rail_inventory", serde_json::json!({"entries": entries}));
}

#[cfg(test)]
mod tests {
    #[test]
    fn translated_rail_labels_fit_as_a_group_at_both_text_scales() {
        // Fixed test font advances in em, independent of the machine's font installation.
        for (label, advance) in [("Simulation", 0.5_f32), ("Esquemático", 0.55), ("シミュレーション", 1.)] {
            let em = label.chars().count() as f32 * advance;
            for target in [11., 13.75] {
                let fits = |size| em * size <= 69.;
                let (size, ellipsis) = super::rail_label_fit(target, fits);
                assert!((9.5..=target).contains(&size));
                assert_eq!(ellipsis, !fits(9.5));
                if ellipsis { assert_eq!(size, 9.5); }
                else { assert!(fits(size)); assert!(size == target || !fits(size + 1./64.)); }
            }
        }
    }

    use super::{kicad_lit_id, kicad_rail_id, rail_inventory_json, rail_lit_id, rail_metric_change, rail_rows, viewer_row_on, RailInvRow, RailRow};
    use serde_json::{json, Value};
    #[test]
    fn rail_rows_follow_hub_order_content_and_empty_list() {
        let viewer = json!({"rail":[
            {"id":"files","label":"Project files","icon":"folder","group":"work","state":"on","key":"f"},
            {"id":"board","label":"Board","icon":"cpu","group":"design","state":"disabled","key":"b","why":"No board"}]});
        let rows = rail_rows(&viewer, false);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].id, rows[0].label, rows[0].state), ("files", "Project files", "on"));
        assert_eq!((rows[1].icon, rows[1].group, rows[1].key, rows[1].state, rows[1].why), ("cpu", "design", "b", "disabled", "No board"));
        assert!(rail_rows(&json!({"rail":[]}), true).is_empty());
        assert_eq!(rail_rows(&Value::Null, false).last().unwrap().id, "commands");
    }

    #[test]
    fn reveal_only_scrolls_as_far_as_the_selected_row_needs() {
        use gpui::px;
        let offset = |old, top, height| super::reveal_rail_row(px(old), px(top), px(48.), px(height));
        assert_eq!(offset(0., 396., 300.), px(144.));
        assert_eq!(offset(144., 180., 300.), px(144.));
        assert_eq!(offset(144., 48., 300.), px(48.));
        assert_eq!(offset(0., 396., 24.), px(396.));
    }
    #[test]
    fn short_rail_reserves_the_tools_before_allocating_scroll_space() {
        use gpui::px;
        for (height, pages) in [(720., 623.), (160., 63.), (97., 0.)] {
            assert_eq!(super::rail_layout(px(height), px(48.), 2), (px(pages), px(97.)));
        }
        assert_eq!(super::rail_layout(px(160.), px(48.), 0), (px(160.), px(0.)));
    }
    #[test]
    fn pending_press_lights_the_target_until_the_hub_replies() {
        let files = RailRow { id: "files", state: "on", ..Default::default() };
        let board = RailRow { id: "board", state: "off", ..Default::default() };
        assert!(viewer_row_on(&board, None, Some("board"), "files"));
        assert!(!viewer_row_on(&files, None, Some("board"), "files"));
        assert!(!viewer_row_on(&files, None, Some(""), "files"));
        assert!(viewer_row_on(&files, None, None, "board"));
        assert!(!viewer_row_on(&board, None, None, "board"));
    }

    #[test]
    fn kicad_frame_maps_to_its_rail_entry_and_leads_without_round_trip() {
        for (frame, rail) in [
            ("PCB", Some("board")),
            ("FOOTPRINT_EDITOR", Some("board")),
            ("SCH", Some("schematic")),
            ("SYMBOL_EDITOR", Some("schematic")),
            ("PCB_3D", Some("3d")),
            ("GERBVIEW", Some("gerbers")),
            ("SIMULATOR", Some("simulation")),
            ("UNKNOWN", None),
        ] {
            assert_eq!(kicad_rail_id(frame), rail, "{frame}");
        }
        let board = RailRow { id: "board", state: "on", ..Default::default() };
        let sch = RailRow { id: "schematic", state: "off", ..Default::default() };
        let files = RailRow { id: "files", state: "off", ..Default::default() };
        let split = RailRow { id: "split", state: "off", ..Default::default() };
        let sim = RailRow { id: "simulation", state: "off", ..Default::default() };
        // Strip switched to Schematic (or Symbol Editor) while hub's page is still Board (no pending press): Schematic lights, Board dims.
        let lit = kicad_rail_id("SCH");
        assert!(viewer_row_on(&sch, lit, None, "board"));
        assert!(!viewer_row_on(&board, lit, None, "board"));
        let sym = kicad_rail_id("SYMBOL_EDITOR");
        assert!(viewer_row_on(&sch, sym, None, "board"));
        assert!(!viewer_row_on(&board, sym, None, "board"));
        // Footprint Editor lights Board; Simulator lights Simulation with no pending press.
        assert!(viewer_row_on(&board, kicad_rail_id("FOOTPRINT_EDITOR"), None, "schematic"));
        assert!(viewer_row_on(&sim, kicad_rail_id("SIMULATOR"), None, "schematic"));
        assert!(!viewer_row_on(&sch, kicad_rail_id("SIMULATOR"), None, "schematic"));
        // Pending rail press always leads over the embedded KiCad pane's wanted frame:
        // 1) pressing Files or Split while pane is on Board/Schematic lights that target, never Board
        assert!(viewer_row_on(&files, Some("board"), Some("files"), "board"));
        assert!(!viewer_row_on(&board, Some("board"), Some("files"), "board"));
        assert!(viewer_row_on(&split, Some("board"), Some("split"), "schematic"));
        assert!(!viewer_row_on(&board, Some("board"), Some("split"), "schematic"));
        // 2) pressing Schematic while pane is on Board lights Schematic and unlights Board
        assert!(viewer_row_on(&sch, Some("board"), Some("schematic"), "board"));
        assert!(!viewer_row_on(&board, Some("board"), Some("schematic"), "board"));
        // 3) pressing the lit entry or X to close (pending = "") lights nothing
        assert!(!viewer_row_on(&board, Some("board"), Some(""), "board"));
        assert!(!viewer_row_on(&sch, Some("board"), Some(""), "board"));

        // Gates on kicad_lit_id:
        let no_gerbers = [
            RailRow { id: "board", state: "on", ..Default::default() },
            RailRow { id: "schematic", state: "disabled", ..Default::default() },
        ];
        // Linked thread (local = false) must never light the local pane's frame.
        assert_eq!(kicad_lit_id(&no_gerbers, false, true, false, Some("PCB")), None);
        // GerbView open with no Gerbers row in `rows` does not suppress the hub's lit row.
        assert_eq!(kicad_lit_id(&no_gerbers, true, true, false, Some("GERBVIEW")), None);
        // Disabled target row is not lit by the pane either.
        assert_eq!(kicad_lit_id(&no_gerbers, true, true, false, Some("SCH")), None);
        // Enabled target row on a local thread holding the editor is lit.
        assert_eq!(kicad_lit_id(&no_gerbers, true, true, false, Some("PCB")), Some("board"));

        // rail_lit_id end-to-end (used by both rail rendering and rail_press close check):
        let thread = json!({"viewer":{"open":"board","rail":[
            {"id":"board","state":"on"},{"id":"schematic","state":"off"},{"id":"files","state":"off"}]}});
        // Local thread with pane on SCH lights "schematic"; linked thread keeps hub's "board".
        assert_eq!(rail_lit_id(&thread, true, Some("SCH"), None).as_deref(), Some("schematic"));
        assert_eq!(rail_lit_id(&thread, false, Some("SCH"), None).as_deref(), Some("board"));
        // GerbView with no Gerbers row falls back to hub's "board" instead of lighting nothing.
        assert_eq!(rail_lit_id(&thread, true, Some("GERBVIEW"), None).as_deref(), Some("board"));
        // Pending press leads (leaving editor or closing).
        assert_eq!(rail_lit_id(&thread, true, Some("PCB"), Some("files")).as_deref(), Some("files"));
        assert_eq!(rail_lit_id(&thread, true, Some("PCB"), Some("")), None);
        // Fallback for older hubs / linked threads without `viewer.rail`.
        let legacy = json!({"viewer":{"open":"board"}});
        assert_eq!(rail_lit_id(&legacy, false, Some("SCH"), None).as_deref(), Some("board"));
        assert_eq!(rail_lit_id(&legacy, true, Some("SCH"), None).as_deref(), Some("schematic"));
    }

    #[test]
    fn rail_metric_emits_on_first_paint_and_on_change_only() {
        let mut last = None;
        let first = rail_metric_change(&mut last, &["Board"], &["Board", "Schematic", "Split"], &["Split"]);
        assert_eq!(first, Some(json!({"lit": ["Board"], "entries": ["Board", "Schematic", "Split"], "disabled": ["Split"]})));
        assert_eq!(rail_metric_change(&mut last, &["Board"], &["Board", "Schematic", "Split"], &["Split"]), None, "unchanged frame emits nothing");
        let switched = rail_metric_change(&mut last, &["Simulation"], &["Board", "Schematic", "Simulation", "Split"], &["Split"]);
        assert_eq!(switched, Some(json!({"lit": ["Simulation"], "entries": ["Board", "Schematic", "Simulation", "Split"], "disabled": ["Split"]})));
        assert_eq!(rail_metric_change(&mut last, &["Simulation"], &["Board", "Schematic", "Simulation", "Split"], &["Split"]), None);
    }
    #[test]
    fn rail_inventory_reports_drawn_icon_label_tooltip_keycap_and_palette() {
        let local = crate::local::Local::default();
        let data = serde_json::json!({"thread":{"viewer":{"rail":[
            {"id":"board","label":"Board","icon":"circuit-board","group":"design","state":"off","key":"b"},
            {"id":"gerbers","label":"Gerbers","icon":"layers","group":"design","state":"off","key":""},
            {"id":"split","label":"Split","icon":"columns","group":"design","state":"disabled","key":"v","why":"No schematic"}
        ]}}});
        let pal_map = crate::screens::palette::rail_commands_map(&data, false, &local);
        let rows = vec![
            RailInvRow { id: "board".into(), label: "Board".into(), group: "design".into(), state: "off".into(), disabled: false, icon: "circuit-board".into(), icon_known: true, tooltip_label: "Board".into(), tooltip_hint: "B".into(), key: "b".into(), keycap_drawn: String::new(), keycap_eligible: true },
            RailInvRow { id: "gerbers".into(), label: "Gerbers".into(), group: "design".into(), state: "off".into(), disabled: false, icon: "layers".into(), icon_known: true, tooltip_label: "Gerbers".into(), tooltip_hint: String::new(), key: String::new(), keycap_drawn: String::new(), keycap_eligible: false },
            RailInvRow { id: "split".into(), label: "Split".into(), group: "design".into(), state: "disabled".into(), disabled: true, icon: "columns".into(), icon_known: true, tooltip_label: "No schematic".into(), tooltip_hint: String::new(), key: "v".into(), keycap_drawn: String::new(), keycap_eligible: false },
        ];
        let inv = rail_inventory_json(&rows, &pal_map);
        assert_eq!(inv.len(), 3);
        assert_eq!(inv[0]["id"], "board");
        assert_eq!(inv[0]["tooltip"], "Board");
        assert_eq!(inv[0]["tooltip_hint"], "B");
        assert_eq!(inv[0]["keycap"], "");
        assert_eq!(inv[0]["palette_label"], "Board");
        assert_eq!(inv[0]["palette_shortcut"], "B");
        assert_eq!(inv[1]["tooltip_hint"], "");
        assert_eq!(inv[2]["disabled"], true);
        assert_eq!(inv[2]["tooltip"], "No schematic");
        assert_eq!(inv[2]["palette_label"], "");
    }
}

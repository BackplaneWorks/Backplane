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
pub struct FileRow { pub label: String, pub action: String, pub value: String, pub detail: String, pub kind: String, pub down: String }
#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine { pub k: u32, pub t: String, pub old: String, pub new: String }
#[derive(Clone, Debug, PartialEq)]
pub struct DiffFile { pub name: String, pub status: String, pub lines: Vec<DiffLine> }

pub const MAX_ROWS: usize = 1000;

/// Open a viewer kind, or close the viewer with "": the phones' idempotent `view` action.
pub fn view_kind(kind: &str) -> Command { Command::Action("view".into(), kind.to_string()) }
pub fn close() -> Command { view_kind("") }
/// A rail press (or its key) on a viewer kind: the kind the rail shows lit (the hub's page, or the one just pressed) shuts the viewer, as a press
/// on the web's shown tab did; any other kind opens. Still the idempotent `view` action either way, decided from what is on the screen.
pub fn rail_command(viewer: &Value, diff_on: bool, pending: Option<&str>, kind: &str) -> Command {
    let lit = !diff_on && (pending.is_some() || viewer["open"].as_str().map_or(false, |o| !o.is_empty())) && active_kind(viewer, pending) == kind;
    if lit && !kind.is_empty() { close() } else { view_kind(kind) }
}
/// Open in KiCad: the hub starts KiCad on the thread's design (kicad/open.rs), offered while the viewer has a board or schematic.
pub fn kicad_open() -> Command { Command::Action("kicad-open".into(), String::new()) }
/// The button's words. KiCad opens where the design is, on the hub's own machine: with a hub on another machine the button says whose screen.
pub fn kicad_open_label(hub_addr: Option<&str>, thread_machine: Option<&str>) -> String {
    // a thread of another machine (its mark names it) opens there, whichever hub the window is on
    if let Some(m) = thread_machine.filter(|m| !m.is_empty()) { return format!("Open in KiCad on {m}") }
    match hub_addr.map(|a| a.rsplit_once(':').map_or(a, |(h, _)| h)) {
        Some(h) if !h.is_empty() && !matches!(h, "127.0.0.1" | "localhost" | "[::1]") => format!("Open in KiCad on {}", h.split('.').next().unwrap_or(h)),
        _ => "Open in KiCad".into(),
    }
}
pub fn has_design(viewer: &Value) -> bool { tabs(viewer).iter().any(|t| matches!(t.value.as_str(), "board" | "schematic")) }

fn arr<'a>(v: &'a Value, k: &str) -> impl Iterator<Item = &'a Value> { v[k].as_array().into_iter().flatten() }
pub fn tabs(viewer: &Value) -> Vec<Tab> { arr(viewer, "choices").map(|c| Tab { label: string(c, "label"), value: string(c, "value"), on: c["on"].as_bool().unwrap_or(false) }).collect() }
pub fn crumbs(files: &Value) -> Vec<Crumb> { arr(files, "trail").map(|c| Crumb { label: string(c, "label"), action: string(c, "action"), value: string(c, "value") }).collect() }
pub fn file_rows(files: &Value) -> Vec<FileRow> { arr(files, "rows").map(|r| FileRow { label: string(r, "label"), action: string(r, "action"), value: string(r, "value"), detail: string(r, "detail"), kind: string(r, "kind"), down: string(r, "down") }).collect() }
pub fn diff_files(diff: &Value) -> Vec<DiffFile> {
    arr(diff, "files").map(|f| DiffFile { name: string(f, "name"), status: string(f, "status"),
        lines: arr(f, "lines").map(|l| DiffLine { k: l["k"].as_u64().unwrap_or(0) as u32, t: string(l, "t"), old: string(l, "o"), new: string(l, "n") }).collect() }).collect()
}
/// The sign and the kind of colour a diff line takes: 1 added, 2 removed, 4 a hunk header, the rest context.
pub fn diff_sign(k: u32) -> &'static str { match k { 1 => "+", 2 => "-", _ => " " } }

/// The kind the viewer shows: the hub's marked tab, or the one just pressed until the hub answers. A page with no tab of its own (Split,
/// Browser) is its open kind; a file opened from Files is Files'.
pub fn active_kind(viewer: &Value, pending: Option<&str>) -> String {
    pending.map(str::to_string).or_else(|| tabs(viewer).into_iter().find(|t| t.on).map(|t| t.value))
        .unwrap_or_else(|| match viewer["open"].as_str().unwrap_or("") { "image" | "pdf" | "text" => "files".into(), o => o.to_string() })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Part { pub label: String, pub value: String, pub on: bool }
/// The Mechanical page's parts (`viewer.mech.parts`): a press is `mech-part value`.
pub fn mech_parts(viewer: &Value) -> Vec<Part> { arr(&viewer["mech"], "parts").map(|p| Part { label: string(p, "label"), value: string(p, "value"), on: p["on"].as_bool().unwrap_or(false) }).collect() }

/// The 3D page may be shown only when the model it holds is the source the viewer asks for (`viewer.key`): one page serves every thread, so a
/// switched thread or a new part waits for its own frame instead of showing the last model.
pub fn solid_ready(want: &str, have: &str) -> bool { want.is_empty() || want == have }
/// Whether `view` draws the 3D page for this thread (3D, Mech or Split, the pane open, no diff over it): when not, the page is put away
/// (SolidPage::put_away: its pictures leave the GPU's atlas).
pub fn shows_solid(thread: &Value) -> bool { is_open(thread) && !thread["diff"].is_object() && matches!(thread["viewer"]["open"].as_str(), Some("3d" | "mech" | "split")) }

/// An agent's viewer_show (`kind`, `path`) as the page to open and, for a model, the part on it: Some((view kind, part)). A kind the viewer has no page for is ignored.
pub fn shown(kind: &str, path: &str) -> Option<(String, Option<String>)> {
    let l = path.to_ascii_lowercase();
    let model = [".step", ".stp", ".glb"].iter().any(|e| l.ends_with(e));
    let k = match kind { "board" | "pcb" => "board", "schematic" | "sch" => "schematic", "3d" if model => "mech", "3d" => "3d", "mech" | "split" | "browser" | "files" | "review" => kind,
        "" if l.ends_with(".kicad_pcb") => "board", "" if l.ends_with(".kicad_sch") => "schematic", "" if model => "mech", _ => return None };
    Some((k.to_string(), (k == "mech" && model).then(|| path.to_string())))
}

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
fn mech_chrome(cx: &mut Context<Backplane>, light: bool, viewer: &Value, key_empty: bool, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mech = &viewer["mech"];
    let mut col = div().flex().flex_col().gap_1().px_3().py_2().border_b_1().border_color(c("line"));
    let parts = mech_parts(viewer);
    if key_empty {
        let say = [string(mech, "say"), string(mech, "empty"), string(mech, "note")].into_iter().find(|s| !s.is_empty());
        if let Some(s) = say { col = col.child(div().text_size(px(12.)).text_color(c("dim")).child(s)); }
    }
    let mut row = div().flex().flex_wrap().items_center().gap_2();
    for (i, p) in parts.iter().enumerate() { row = row.child(btn(cx, light, format!("mech-part-{i}"), p.label.clone(), p.on, Command::Action("mech-part".into(), p.value.clone()))); }
    // (no Renders button: the renders page is not drawn here yet, and an offered button must lead somewhere)
    row = row.child(btn(cx, light, "mech-refresh".into(), "Refresh".into(), false, Command::Action("mech-refresh".into(), String::new())));
    // the pane's controls end the first line: the parts wrap under them
    col.child(div().flex().items_start().gap_2().child(div().flex_1().min_w_0().child(row)).child(actions)).into_any_element()
}

/// A row's icon by what it opens as.
pub fn file_icon(kind: &str) -> &'static str {
    match kind { "folder" => "folder", "board" => "circuit-board", "schematic" => "cpu", "3d" => "box", "image" => "image", "pdf" => "file-text", _ => "file" }
}
/// A file the viewer cannot open (listed only) is drawn dim, as the web does.
pub fn file_listed_only(kind: &str) -> bool { !matches!(kind, "folder" | "board" | "schematic" | "3d" | "image" | "pdf" | "text") }

/// diff.revert.confirm (i18n/en.msg)
const REVERT_ASK: &str = "Put the files back as they were when this thread began? Every change shown below is undone, committed ones and new files too. Commits stay in git.";

/// The Diff page's buttons as the harness's controls: (id, label, action, value); none while Diff is not on show.
pub fn diff_controls(diff: &Value) -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    if !diff.is_object() { return vec![] }
    let mut v = vec![("git-commit_push", "Commit", "git", "commit_push"), ("git-commit_push_pr", "PR", "git", "commit_push_pr"), ("revert-ask", "Revert", "revert", "ask")];
    if diff["reverting"] == true { v.push(("revert-confirm", "Revert", "revert", "confirm")); v.push(("revert-cancel", "Cancel", "revert-no", "")); }
    v
}

/// The room (px) the pane's controls take in a page's top right corner, left free by the 3D page's own floating tools.
pub const CORNER_ROOM: f32 = 150.0;
/// The pane's own controls, at the end of the page's toolbar row: where the page comes from (another machine's thread: "desktop: boards/x.kicad_pcb"),
/// Open in KiCad (while there is a board or schematic) and Close. They were the tab bar's; the rail picks the page now.
fn pane_actions(cx: &mut Context<Backplane>, light: bool, thread: &Value, target: Option<&super::attach::Target>, diff_on: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let viewer = &thread["viewer"];
    let mut row = div().flex().flex_shrink_0().items_center().gap_1().max_w_full();
    if let Some((_, o, hover)) = super::badge::place_text(&thread["mark"], "") {
        let t = super::badge::from_machine(&thread["mark"], &source(viewer, diff_on)); let col = c("dim");
        row = row.child(div().id("viewer-from").min_w_0().max_w(px(200.)).flex().items_center().gap(px(3.)).overflow_hidden().whitespace_nowrap().text_size(px(11.)).text_color(col).opacity(o)
            .tooltip(crate::keys::tip(&hover, "", light)).child(crate::art::icon("link", 11., col)).child(div().overflow_hidden().text_ellipsis().child(t)));
    }
    if has_design(viewer) { row = row.child(btn(cx, light, "kicad-open".into(), kicad_open_label(target.map(|t| t.addr.as_str()), thread["mark"]["name"].as_str()), false, kicad_open())); }
    row.child(div().id("viewer-close").px_1().py_1().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel"))).tooltip(crate::keys::tip("Close the viewer", "", light))
        .child(crate::art::icon("x", 14., c("dim"))).on_click(cx.listener(|this, _, _, cx| { this.screens.rail_pending = Some(String::new()); this.screens.rail_at = Some(std::time::Instant::now()); let _ = this.tx.try_send(close()); cx.notify(); })))
        .into_any_element()
}
/// The 3D page's corner: the Boards menu (a project with several boards) before the pane's own controls.
fn with_boards(cx: &mut Context<Backplane>, light: bool, viewer: &Value, pv: &super::plotview::PlotView, actions: AnyElement) -> AnyElement {
    match crate::viewers::boards::picker(cx, light, viewer, &pv.boards) { Some(b) => div().flex().items_center().gap_1().child(b).child(actions).into_any_element(), None => actions }
}
/// A page drawn to its edges with the pane's controls over its top right corner.
fn in_corner(page: AnyElement, actions: AnyElement) -> AnyElement {
    div().relative().size_full().child(page).child(div().absolute().top_1().right_2().flex().justify_end().max_w(px(CORNER_ROOM + 440.)).child(actions)).into_any_element()
}
/// A page that has nothing to show yet: what it says, and the pane's controls on the same row.
fn status_row(light: bool, say: &str, actions: Option<AnyElement>) -> AnyElement {
    div().flex().items_center().gap_2().px_3().py_2().child(div().flex_1().min_w_0().text_color(theme::color(light, "dim")).child(say.to_string())).children(actions).into_any_element()
}

fn diff_page(cx: &mut Context<Backplane>, light: bool, diff: &Value, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().text_size(px(12.));
    col = col.child(div().flex().gap_2().px_3().py_2().items_center().child(div().flex_1().text_color(c("dim")).child(string(diff, "summary")))
        .children([("Commit", "commit_push"), ("PR", "commit_push_pr")].into_iter().map(|(l, v)| btn(cx, light, format!("git-{v}"), l.to_string(), false, Command::Action("git".into(), v.into()))).collect::<Vec<_>>())
        .child(btn(cx, light, "revert-ask".into(), "Revert".into(), diff["reverting"] == true, Command::Action("revert".into(), "ask".into()))).child(actions));
    // Revert asks first (Ui.revert): the diff stays on show while the person decides what to discard
    if diff["reverting"] == true {
        col = col.child(div().flex().flex_col().gap_2().px_3().py_2().bg(c("raise")).border_y_1().border_color(c("bad"))
            .child(div().text_color(c("text")).child(REVERT_ASK))
            .child(div().flex().gap_2().child(btn(cx, light, "revert-confirm".into(), "Revert".into(), true, Command::Action("revert".into(), "confirm".into())))
                .child(btn(cx, light, "revert-cancel".into(), "Cancel".into(), false, Command::Action("revert-no".into(), String::new())))));
    }
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

/// One plot canvas: the frame it is given (or what the hub says instead) and, when `layers` is on, the hub's layer list beside it.
/// The viewer's boards (`viewer.boards`: label, path from the project's folder, on show).
/// What the pane shows, for its header beside the machine it comes from (`badge::from_machine`): the board on show, a file opened from Files,
/// the folder in Files, the Mech part, else the page's name.
pub fn source(viewer: &Value, diff_on: bool) -> String {
    if diff_on { return "Diff".into() }
    let open = viewer["open"].as_str().unwrap_or("");
    let on = boards(viewer).into_iter().find(|b| b.2).map(|b| b.1);
    // a key is `<n>|<root>|<file>`: a file opened from Files is its path
    let keyed = || string(viewer, "key").splitn(3, '|').nth(2).filter(|f| !f.is_empty() && !f.contains('|')).map(|f| f.rsplit('/').next().unwrap_or(f).to_string());
    let named = match open {
        "board" | "schematic" | "3d" => on.or_else(keyed),
        "files" => Some(crumbs(&viewer["files"]).iter().skip(1).map(|c| c.label.as_str()).collect::<Vec<_>>().join("/")).filter(|p| !p.is_empty()),
        "image" | "pdf" | "text" => Some(string(&viewer["file"], "name")).filter(|n| !n.is_empty()),
        "mech" => Some(string(&viewer["mech"], "rel")).filter(|n| !n.is_empty()),
        _ => None,
    };
    named.unwrap_or_else(|| tabs(viewer).into_iter().find(|t| t.on).map_or_else(|| open.to_string(), |t| t.label))
}
pub fn boards(viewer: &Value) -> Vec<(String, String, bool)> {
    viewer["boards"].as_array().into_iter().flatten().map(|b| (string(b, "label"), string(b, "value"), b["on"].as_bool().unwrap_or(false))).collect()
}
fn plot_pane(cx: &mut Context<Backplane>, light: bool, slot: usize, open: &str, viewer: &Value, frame: Option<&super::plot::Frame>, pv: &mut super::plotview::PlotView, layers: bool, sheets: &[super::plot::Sheet], actions: Option<AnyElement>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let loading = |actions| status_row(light, if open == "board" { "Loading the board…" } else { "Loading the schematic…" }, actions);
    // the design history bar stays under a board or schematic while a version loads or cannot be shown (screens::histbar)
    let hb = layers && matches!(open, "board" | "schematic"); let vkey = string(viewer, "key");
    let Some(frame) = frame else { return super::histbar::under(cx, light, pv, &vkey, hb, loading(actions)) };
    // the frame must be the source this viewer asks for: another project's board stays off the screen until its own plot arrives
    let mut want = string(viewer, "key");
    // a comparison on show is its own source: `diff\n<old>\n<live>` after the key; another page puts it away
    if let Some(cmp) = pv.compare.clone() { if cmp.base == want { want = format!("{want}{}", super::histbar::shown(&cmp)); } else { pv.compare = None; } }
    // (another version of the same file keeps the last one on the canvas until it comes, as 0.14 did: stepping does not blink to "Loading")
    if !want.is_empty() && !frame.key.is_empty() && frame.key != want && !super::histbar::same_file(&vkey, &frame.key, &want) { return super::histbar::under(cx, light, pv, &vkey, hb, loading(actions)) }
    if !frame.none.is_empty() { return super::histbar::under(cx, light, pv, &vkey, hb, status_row(light, &super::histbar::none_words(&frame.none), actions)); }
    if pv.focus.is_none() { pv.focus = Some(cx.focus_handle()); }
    let (w, h) = pv.bounds.map_or((480., 600.), |b| (f32::from(b.size.width), f32::from(b.size.height)));
    let view = super::plotview::ensure_view(pv, frame, w, h);
    let mut col = div().size_full().flex().flex_col();
    // the page's toolbar first: the pane's controls at its end (the design history bar is under the canvas, screens::histbar)
    let mut actions = actions;
    if let Some(a) = actions.take() { col = col.child(div().flex().justify_end().px_3().py_1().child(a)); }
    let note = string(viewer, "note");
    if !note.is_empty() { col = col.child(div().px_3().py_1().text_size(px(11.)).text_color(c("dim")).child(note)); }
    // Layers: the menu lists the layers this plot has; the hub's masks decide what the canvas draws
    let mut present = std::collections::BTreeMap::new();
    for ch in &frame.chunks { present.entry(ch.layer).or_insert(ch.color); }
    let strip = crate::viewers::layers::strip(viewer, &present);
    pv.present = strip.present;
    // Sheets: a schematic of several sheets lists them as a tree (indented by depth); a press shows that sheet, the root first
    if sheets.len() > 1 && open == "schematic" && layers {
        let on_key = frame.key.clone();
        let head = div().id("sheets-btn").px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).child(format!("Sheets ({})", sheets.len()))
            .on_click(cx.listener(|this, _, _, cx| { let pv = this.screens.pv(0); pv.sheets_open = !pv.sheets_open; cx.notify(); }));
        col = col.child(div().flex().px_3().py_1().child(head));
        if pv.sheets_open {
            let mut list = div().id("sheets-list").flex().flex_col().px_3().pb_1().text_size(px(12.));
            for (i, sh) in sheets.iter().enumerate() {
                let file = if i == 0 { String::new() } else { sh.file.clone() };
                let here = on_key.ends_with(&sh.file) && i != 0 || (i == 0 && on_key.ends_with('|'));
                list = list.child(div().id(SharedString::from(format!("sheet-{i}"))).pl(px(8. * sh.depth as f32)).py_px().cursor_pointer().text_color(if here { c("accent") } else { c("text") }).hover(|s| s.bg(c("raise"))).child(sh.name.clone())
                    .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-sheet".into(), file.clone())); this.screens.pv(0).sheets_open = false; cx.notify(); })));
            }
            col = col.child(list);
        }
    }
    if layers {
        let fit = div().id(SharedString::from(format!("plot-fit-{slot}"))).px_2().py_1().mt_1().ml_1().text_size(px(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).hover(|s| s.bg(c("panel"))).child("Fit")
            .on_click(cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.touched = false; pv.fit_wh = (0., 0.); cx.notify(); }));
        // the Boards menu leads the row (0.14 put it first in the viewer's bar), then Layers and Fit
        let boards = if slot == 0 { crate::viewers::boards::picker(cx, light, viewer, &pv.boards) } else { None };
        col = col.child(div().flex().items_center().min_w_0().children(boards.map(|b| div().pl_3().py_1().min_w_0().child(b))).child(crate::viewers::layers::strip_el(cx, light, &strip, &pv.layers_key)).child(fit));
    }
    let mut area = div().relative().flex().flex_col().flex_1().min_h_0().child(super::plotview::canvas_el(cx, slot, pv.focus.clone(), light, frame, view, crate::viewers::layers::masks(viewer)));
    if layers { if let Some(p) = crate::viewers::layers::panel_el(cx, light, &strip, pv.layers_open, pv.layers_cap.unwrap_or(280.), &pv.layers_scroll) { area = area.child(p); } }
    // the card of the piece last tapped (Insp.card): what it is, then key = value rows; a tap on nothing, or Close, puts it away
    // (Split's panes, slots 1 and 2, name it in their bar instead: a card there covered the drawing beside it)
    if let Some(card) = pv.card.clone().filter(|_| slot == 0) {
        let mut box_ = div().id(SharedString::from(format!("pick-card-{slot}"))).absolute().left_2().bottom_2().max_w(px(300.)).flex().flex_col().gap_1().p_2().text_size(px(12.)).border_1().border_color(c("line")).bg(c("panel")).occlude()
            .child(div().flex().gap_2().child(div().flex_1().font_weight(FontWeight::BOLD).child(card.title.clone()))
                .child(div().id(SharedString::from(format!("pick-close-{slot}"))).cursor_pointer().text_color(c("dim")).child("×").on_click(cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.card = None; pv.picked = None; cx.notify(); }))));
        for (k, v) in card.rows.iter().take(14) { box_ = box_.child(div().flex().gap_2().child(div().w(px(70.)).text_color(c("dim")).child(k.clone())).child(div().flex_1().min_w_0().child(v.clone()))); }
        area = area.child(box_);
    }
    col = col.child(area);
    if hb { col = col.child(super::histbar::bar_el(cx, light, pv, &vkey)); }
    col.into_any_element()
}

/// Board or schematic: the plot the hub sent for this file.
fn plot_page(cx: &mut Context<Backplane>, light: bool, open: &str, viewer: &Value, plot: &super::plot::Store, pv: &mut super::plotview::PlotView, actions: AnyElement) -> AnyElement {
    plot_pane(cx, light, 0, open, viewer, plot.frame.as_ref(), pv, true, &plot.sheets, Some(actions))
}

/// Split's panes by the words 0.14 kept (split.hide): "sch" the schematic, "pcb" the board, "3d" its 3D model.
pub const SPLIT_PANES: [(&str, &str); 3] = [("sch", "Schematic"), ("pcb", "Board"), ("3d", "3D")];
/// Whether pane `p` shows with the panes `hide` names put away.
pub fn split_shows(hide: &str, p: &str) -> bool { !hide.split_whitespace().any(|w| w == p) }
/// 0.14's Split.flip on a desktop: pane `p` put away or brought back, each on its own; the last pane shown stays (a press on it changes nothing).
pub fn split_flip(hide: &str, p: &str) -> String {
    if !SPLIT_PANES.iter().any(|(k, _)| *k == p) { return hide.to_string() }
    let mut on: Vec<&str> = SPLIT_PANES.iter().map(|x| x.0).filter(|q| split_shows(hide, q)).collect();
    if on.contains(&p) { if on.len() == 1 { return hide.to_string() } on.retain(|q| *q != p) } else { on.push(p) }
    SPLIT_PANES.iter().map(|x| x.0).filter(|q| !on.contains(q)).collect::<Vec<_>>().join(" ")
}
/// A divider's share for its first pane: the saved one (0: the default, 0.14's 1.15 : 1 for the schematic, halves for the board and 3D), kept so each pane
/// keeps 140 px for its head and some drawing (project-splitters.js `splitLimits`).
pub fn split_share(saved: f32, main: bool, extent: f32) -> f32 {
    let d = if main { 1.15 / 2.15 } else { 0.5 };
    let r = if saved > 0. && saved < 1. { saved } else { d };
    let min = if extent > 1. { (140. / extent).min(0.45) } else { 0.1 };
    r.clamp(min, 1. - min)
}
/// The share a drag to `at` (window px along the divider's axis) makes of a group starting at `origin` and `extent` long.
pub fn share_at(at: f32, origin: f32, extent: f32) -> f32 { if extent <= 1. { 0.5 } else { split_share(((at - origin) / extent).clamp(0.01, 0.99), true, extent) } }
/// The thickness of a divider (px): the 2 px line and room to take it.
const SPLIT_EDGE: f32 = 7.0;

/// This device's Split choices (local.rs), read once and kept here as the pane width is; and the two groups' bounds last painted
/// (0 the schematic beside the rest, 1 the board beside the 3D), for the dividers' drags and the panes' arrangement.
#[derive(Clone, Default)]
struct SplitPrefs { hide: String, main: f32, side: f32 }
static SPLIT: std::sync::Mutex<Option<SplitPrefs>> = std::sync::Mutex::new(None);
static SPLIT_BOX: std::sync::Mutex<[Option<Bounds<Pixels>>; 2]> = std::sync::Mutex::new([None, None]);
fn split_prefs() -> SplitPrefs {
    let mut g = SPLIT.lock().unwrap();
    g.get_or_insert_with(|| { let l = crate::local::Local::load(); SplitPrefs { hide: l.split_hide, main: l.split_main, side: l.split_side } }).clone()
}
fn split_set(this: &mut Backplane, f: impl FnOnce(&mut SplitPrefs), save: bool, cx: &mut Context<Backplane>) {
    let mut p = split_prefs(); f(&mut p);
    this.local.split_hide = p.hide.clone(); this.local.split_main = p.main; this.local.split_side = p.side;
    *SPLIT.lock().unwrap() = Some(p);
    if save { this.local.save(); }
    this.ui_epoch += 1; cx.notify();
}
/// The panes Split shows on this device (for the cross-probe and the harness).
pub fn split_visible() -> Vec<&'static str> { let h = split_prefs().hide; SPLIT_PANES.iter().map(|x| x.0).filter(|p| split_shows(&h, p)).collect() }
fn split_box(i: usize) -> Option<Bounds<Pixels>> { SPLIT_BOX.lock().unwrap()[i] }
/// An invisible layer that keeps a group's painted bounds.
fn box_probe(i: usize) -> AnyElement {
    // (a new size draws the window again: the viewer is a cached part (part.rs), and the arrangement and the shares are read from these bounds)
    canvas(move |b, window, _| { let mut g = SPLIT_BOX.lock().unwrap(); if g[i].map_or(true, |o| o.size != b.size) { g[i] = Some(b); window.refresh(); } else { g[i] = Some(b); } }, |_, _, _, _| {}).absolute().top_0().left_0().size_full().into_any_element()
}
struct SplitMain;
struct SplitSide;
/// A divider between two panes: drag to resize, double click for the default; `row` when the panes sit side by side (a vertical line).
fn split_edge(cx: &mut Context<Backplane>, light: bool, main: bool, row: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let (line, grip) = (c("line"), c("faint"));
    let id = if main { "split-resize-main" } else { "split-resize-side" };
    let mut d = div().id(id).flex_none().relative().bg(c("panel")).group(id);
    d = if row { d.w(px(SPLIT_EDGE)).h_full().cursor_col_resize() } else { d.h(px(SPLIT_EDGE)).w_full().cursor_row_resize() };
    let hl = theme::color(light, "accent");
    let mark = |d: Div| if row { d.absolute().top_0().bottom_0().left(px(SPLIT_EDGE / 2. - 1.)).w(px(2.)) } else { d.absolute().left_0().right_0().top(px(SPLIT_EDGE / 2. - 1.)).h(px(2.)) };
    let knob = |d: Div| if row { d.absolute().top(relative(0.5)).mt(px(-14.)).left(px(SPLIT_EDGE / 2. - 2.)).w(px(4.)).h(px(28.)) } else { d.absolute().left(relative(0.5)).ml(px(-14.)).top(px(SPLIT_EDGE / 2. - 2.)).h(px(4.)).w(px(28.)) };
    d = d.child(mark(div()).bg(line).group_hover(id, move |s| s.bg(hl))).child(knob(div()).rounded(px(2.)).bg(grip).opacity(0.7).group_hover(id, move |s| s.bg(hl).opacity(1.)));
    let reset = cx.listener(move |this, e: &ClickEvent, _, cx| { if e.click_count() >= 2 { split_set(this, |p| if main { p.main = 0. } else { p.side = 0. }, true, cx); } });
    let moved = move |this: &mut Backplane, pos: Point<Pixels>, cx: &mut Context<Backplane>| {
        let Some(b) = split_box(if main { 0 } else { 1 }) else { return };
        let (at, o, e) = if row { (pos.x, b.origin.x, b.size.width) } else { (pos.y, b.origin.y, b.size.height) };
        let r = share_at(f32::from(at), f32::from(o), f32::from(e));
        split_set(this, |p| if main { p.main = r } else { p.side = r }, false, cx);
    };
    let d = if main { d.on_drag(SplitMain, |_, _, _, cx| cx.new(|_| Ghost)).on_drag_move(cx.listener(move |this, e: &DragMoveEvent<SplitMain>, _, cx| moved(this, e.event.position, cx))) }
        else { d.on_drag(SplitSide, |_, _, _, cx| cx.new(|_| Ghost)).on_drag_move(cx.listener(move |this, e: &DragMoveEvent<SplitSide>, _, cx| moved(this, e.event.position, cx))) };
    d.on_click(reset).into_any_element()
}
/// A pane's head (0.14's combo-head): its name, and × to put it away.
fn split_head(cx: &mut Context<Backplane>, light: bool, pane: &'static str, name: &'static str) -> Div {
    let c = |n: &str| theme::color(light, n);
    div().flex_none().h(px(26.)).flex().items_center().gap_2().pl_2().pr_1().bg(c("panel")).border_b_1().border_color(c("line"))
        .child(div().flex_1().min_w_0().text_size(px(10.)).text_color(c("dim")).child(name.to_uppercase()))
        .child(div().id(SharedString::from(format!("split-hide-{pane}"))).p(px(3.)).cursor_pointer().hover(|s| s.bg(c("raise"))).tooltip(crate::keys::tip(&format!("Hide the {name}"), "", light))
            .child(crate::art::icon("x", 13., c("dim"))).on_click(cx.listener(move |this, _, _, cx| split_set(this, |p| p.hide = split_flip(&p.hide, pane), true, cx))))
}
/// The bar's pane toggles (lit while shown) and the probed piece as a chip (0.14's combo-picks and probe-chip): a tap on a part or net in any pane names it here
/// instead of a card over the drawing; × or Escape clears it in every pane.
fn split_bar(cx: &mut Context<Backplane>, light: bool, hide: &str, probed: Option<&super::plot::Card>, boards: Option<AnyElement>, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut picks = div().flex().flex_none().gap_1();
    for (pane, name) in SPLIT_PANES.iter() {
        let (pane, name, on) = (*pane, *name, split_shows(hide, pane));
        // (0.14's combo-pick: outlined in the accent while shown, dim while put away)
        let d = div().id(SharedString::from(format!("split-pick-{pane}"))).px_2().py(px(2.)).text_size(px(12.)).cursor_pointer().border_1().hover(|s| s.bg(c("raise")));
        let d = if on { d.border_color(c("accent")).text_color(c("text")) } else { d.border_color(c("line")).text_color(c("dim")) };
        picks = picks.child(d.tooltip(crate::keys::tip(&format!("{} the {name}", if on { "Hide" } else { "Show" }), "", light)).child(name)
            .on_click(cx.listener(move |this, _, _, cx| split_set(this, |p| p.hide = split_flip(&p.hide, pane), true, cx))));
    }
    // the bar: (the Boards menu,) the pane toggles, the probe, then the pane's own controls
    // (as tall as the thread's head beside it, so their lines meet)
    let mut row = div().id("split-bar").flex_none().h(px(super::bar::H)).flex().items_center().gap_2().px_2().border_b_1().border_color(c("line")).children(boards).child(div().flex_1()).child(picks);
    if let Some(card) = probed {
        let words = probe_words(card);
        let full = std::iter::once(card.title.clone()).chain(card.rows.iter().map(|(k, v)| format!("{k}: {v}"))).collect::<Vec<_>>().join("\n");
        row = row.child(div().id("split-probe").min_w_0().flex().items_center().gap_1().pl_2().pr(px(2.)).py_px().border_1().border_color(c("accent")).bg(c("raise")).text_size(px(12.))
            .tooltip(crate::keys::tip(&full, "", light))
            .child(div().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(words.join(" · ")))
            .child(div().id("split-probe-clear").flex_none().p(px(2.)).cursor_pointer().hover(|s| s.bg(c("panel"))).tooltip(crate::keys::tip("Clear", "Esc", light)).child(crate::art::icon("x", 12., c("dim")))
                .on_click(cx.listener(|this, _, _, cx| { for i in 0..3 { let pv = this.screens.pv(i); pv.picked = None; pv.card = None; pv.lit.clear(); } super::plotview::report_state(this); cx.notify(); }))));
    }
    row.child(actions).into_any_element()
}
/// What the probe chip says: the part's reference (else what was tapped), its value and its net, as 0.14's Probe.label named a probe by its ref or net.
pub fn probe_words(card: &super::plot::Card) -> Vec<String> {
    let get = |k: &str| card.rows.iter().find(|r| r.0 == k).map(|r| r.1.trim().to_string()).filter(|v| !v.is_empty());
    let mut words = vec![get("ref").unwrap_or_else(|| card.title.clone())];
    for k in ["value", "net"] { if let Some(v) = get(k) { if !words.contains(&v) { words.push(v) } } }
    words
}
/// Split (0.14's combined view): the schematic beside the board and its 3D model (stacked the other way on a pane taller than wide), each under a head that
/// names it and puts it away, the bar's toggles bring each back, and the dividers between them resize them (kept on this device). Every pane is a page of
/// `viewer.panes` (key, camera), all watched at once; a tap in the schematic or the board lights the part or net in both.
fn split_page(cx: &mut Context<Backplane>, light: bool, viewer: &Value, plot: &super::plot::Store, pvs: &mut [super::plotview::PlotView; 2], solid: Option<&Entity<crate::viewers::solid::SolidPage>>, have: &str, boards: Option<AnyElement>, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let panes: Vec<&Value> = viewer["panes"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if panes.len() < 3 { return status_row(light, "Opening…", Some(actions)) }
    let prefs = split_prefs();
    let (s_on, b_on, t_on) = (split_shows(&prefs.hide, "sch"), split_shows(&prefs.hide, "pcb"), split_shows(&prefs.hide, "3d"));
    let probed = pvs.iter().find_map(|p| p.card.clone());
    let bar = split_bar(cx, light, &prefs.hide, probed.as_ref(), boards, actions);
    // landscape (the pane wider than tall): the schematic on the left, the board over the 3D on the right; portrait: the schematic on top, the board beside the 3D
    let wide = split_box(0).map_or(true, |b| b.size.width >= b.size.height);
    let [sch_pv, brd_pv] = pvs;
    let pane = |head: Div, body: AnyElement| div().flex().flex_col().min_w_0().min_h_0().child(head).child(div().relative().flex_1().min_h_0().min_w_0().child(body));
    let sch = s_on.then(|| { let h = split_head(cx, light, "sch", "Schematic"); let b = plot_pane(cx, light, 1, "schematic", panes[0], plot.frame_for(&string(panes[0], "key")), sch_pv, false, &[], None); pane(h, b) });
    let brd = b_on.then(|| { let h = split_head(cx, light, "pcb", "Board"); let b = plot_pane(cx, light, 2, "board", panes[1], plot.frame_for(&string(panes[1], "key")), brd_pv, false, &[], None); pane(h, b) });
    let d3 = t_on.then(|| {
        let want3 = string(panes[2], "key");
        if let Some(s) = solid { if s.read(cx).reserve != 0. { s.update(cx, |p, cx| { p.reserve = 0.; cx.notify(); }); } }
        let model: AnyElement = match solid { Some(s) if solid_ready(&want3, have) && !have.is_empty() => div().size_full().child(s.clone()).into_any_element(), _ => status_row(light, "Loading the model…", None) };
        let h = split_head(cx, light, "3d", "3D"); pane(h, model)
    });
    // the side group: the board and the 3D, one beside (or over) the other with the divider between them
    let side_row = !wide;
    let sized = |d: Div, share: f32, row: bool| if row { d.flex_none().h_full().w(relative(share)) } else { d.flex_none().w_full().h(relative(share)) };
    let fill = |d: Div, row: bool| if row { d.flex_1().h_full() } else { d.flex_1().w_full() };
    let side_extent = split_box(1).map_or(0., |b| f32::from(if side_row { b.size.width } else { b.size.height }));
    let side: Option<Div> = match (brd, d3) {
        (Some(b), Some(t)) => { let e = split_edge(cx, light, false, side_row); Some(div().relative().flex().when(!side_row, |d| d.flex_col()).child(box_probe(1)).child(sized(b, split_share(prefs.side, false, side_extent), side_row)).child(e).child(fill(t, side_row))) }
        (Some(x), None) | (None, Some(x)) => Some(div().relative().flex().flex_col().child(fill(x, false))),
        (None, None) => None,
    };
    let main_row = wide;
    let main_extent = split_box(0).map_or(0., |b| f32::from(if main_row { b.size.width } else { b.size.height }));
    let body = div().id("split-panes").relative().flex_1().min_h_0().flex().when(!main_row, |d| d.flex_col()).bg(c("view")).child(box_probe(0));
    let body = match (sch, side) {
        (Some(s), Some(sd)) => { let e = split_edge(cx, light, true, main_row); body.child(sized(s, split_share(prefs.main, true, main_extent), main_row)).child(e).child(fill(sd, main_row).min_w_0().min_h_0()) }
        (Some(x), None) => body.child(fill(x, main_row)),
        (None, Some(x)) => body.child(fill(x, main_row).min_w_0().min_h_0()),
        (None, None) => body,
    };
    // a divider's drag ends where the pointer is let go: the shares are written to this device's file then (not on every move)
    let body = body.on_drop(cx.listener(|this, _: &SplitMain, _, cx| split_set(this, |_| {}, true, cx))).on_drop(cx.listener(|this, _: &SplitSide, _, cx| split_set(this, |_| {}, true, cx)));
    div().size_full().flex().flex_col().child(bar).child(body).into_any_element()
}

fn review_page(cx: &mut Context<Backplane>, reg: &Reg, light: bool, review: &Value, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().gap_2().p_3();
    let pages = div().flex_1().min_w_0().flex().flex_wrap().gap_2().children(arr(review, "tabs").enumerate().map(|(i, t)| btn(cx, light, format!("rv-{i}"), string(t, "label"), t["on"].as_bool().unwrap_or(false), Command::Action("rv-page".into(), string(t, "page")))).collect::<Vec<_>>());
    col = col.child(div().flex().items_start().gap_2().child(pages).child(actions));
    let title = string(review, "title");
    let page = arr(review, "tabs").find(|t| t["on"] == true).map(|t| string(t, "page")).unwrap_or_default();
    if !title.is_empty() { col = col.child(div().flex().items_center().gap_2().child(div().flex_1().font_weight(FontWeight::BOLD).text_size(px(15.)).child(title))
        .children(matches!(page.as_str(), "bom" | "checks").then(|| btn(cx, light, "rv-run".into(), "Refresh".into(), false, Command::Action("rv-run".into(), String::new()))))); }
    // the Checks page: why the gate fails, and the checklist's boxes (a press ticks one: rv-tick <index>)
    if review["checks"].is_object() {
        let line = string(&review["checks"], "line"); let gate = string(&review["checks"], "gate");
        if !line.is_empty() { col = col.child(div().text_color(c("dim")).child(line)); }
        if !gate.is_empty() { col = col.child(div().text_color(c("bad")).child(gate)); }
        for t in arr(&review["checks"], "tasks") {
            let (done, idx) = (t["done"] == true, t["index"].as_u64().unwrap_or(0));
            col = col.child(div().id(SharedString::from(format!("rv-task-{idx}"))).flex().gap_2().items_center().cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(div().w(px(24.)).font(crate::art::mono_font()).text_color(c(if done { "ok" } else { "dim" })).child(if done { "[x]" } else { "[ ]" })).child(div().flex_1().child(string(t, "text")))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("rv-tick".into(), idx.to_string())); cx.notify(); })));
        }
    }
    let blocks: Vec<Value> = review["blocks"].as_array().cloned().unwrap_or_default();
    if blocks.is_empty() { col = col.child(div().text_color(c("dim")).child(if review["busy"] == true { "Working…".to_string() } else { string(review, "note") })); }
    else { let parsed = md::parse(&blocks); for el in timeline::blocks(cx, reg, light, &parsed, "review") { col = col.child(el); } }
    col.into_any_element()
}

/// The panel (None while nothing is open). 520 px wide, between the thread and the rail.
pub fn page(cx: &mut Context<Backplane>, reg: &Reg, light: bool, files_ui: &super::filesui::FilesUi, images: &super::images::Images, target: Option<&super::attach::Target>, thread: &Value, plot: &super::plot::Store, pv: &mut super::plotview::PlotView, split_pv: &mut [super::plotview::PlotView; 2], solid: Option<&Entity<crate::viewers::solid::SolidPage>>, browser: Option<&Entity<crate::viewers::browser::BrowserPage>>) -> AnyElement {
    if !is_open(thread) { return Empty.into_any_element(); }
    let viewer = &thread["viewer"];
    let open = viewer["open"].as_str().unwrap_or("");
    let diff_on = thread["diff"].is_object();
    // no tab bar: the rail picks the page. The pane's own controls (where it comes from, Open in KiCad, Close) end the page's toolbar row,
    // or float in the top right corner of a page drawn to its edges (3D, Split, Browser)
    let acts = |cx: &mut Context<Backplane>| pane_actions(cx, light, thread, target, diff_on);
    let body: AnyElement = if diff_on { let a = acts(cx); diff_page(cx, light, &thread["diff"], a) }
        else if open == "files" && viewer["files"].is_object() { let a = acts(cx); super::filesui::files_page(cx, light, &viewer["files"], files_ui, a) }
        else if matches!(open, "image" | "pdf" | "text") && viewer["file"].is_object() { let a = acts(cx); super::filesui::preview_page(cx, reg, light, open, viewer, files_ui, images, target, a) }
        else if open == "review" && viewer["review"].is_object() { let a = acts(cx); review_page(cx, reg, light, &viewer["review"], a) }
        else if matches!(open, "board" | "schematic") { let a = acts(cx); plot_page(cx, light, open, viewer, plot, pv, a) }
        else if open == "split" { let have = solid.map(|s| s.read(cx).shown_key().to_string()).unwrap_or_default(); let a = acts(cx); let b = crate::viewers::boards::picker(cx, light, viewer, &pv.boards); split_page(cx, light, viewer, plot, split_pv, solid, &have, b, a) }
        else if matches!(open, "3d" | "mech") {
            let key_empty = string(viewer, "key").is_empty();
            let want = string(viewer, "key");
            // what the page HOLDS, not what was queued for it: a big model's decode, a frame that fails or is replaced in the worker's slot
            // the model shown before comes back at once when it is the one asked for (the page keeps one)
            if let Some(s) = solid { if !want.is_empty() && s.read(cx).shown_key() != want { s.update(cx, |p, cx| p.want(&want, cx)); } }
            let have = solid.map(|s| s.read(cx).shown_key().to_string()).unwrap_or_default();
            let ready = matches!(solid, Some(s) if solid_ready(&want, &have));
            // the 3D page's own tools float top left over the model: they wrap short of the corner the pane's controls take
            if let Some(s) = solid { let room = if open == "3d" && ready { CORNER_ROOM + if crate::viewers::boards::list(viewer).len() > 1 { 240. } else { 0. } } else { 0. }; if s.read(cx).reserve != room { s.update(cx, |p, cx| { p.reserve = room; cx.notify(); }); } }
            let model: AnyElement = if open == "mech" && key_empty { div().into_any_element() } else { match solid { Some(s) if ready => div().flex_1().min_h_0().child(s.clone()).into_any_element(), _ => status_row(light, "Loading the model…", None) } };
            if open == "mech" { let a = acts(cx); div().size_full().flex().flex_col().child(mech_chrome(cx, light, viewer, key_empty, a)).child(model).into_any_element() }
            else if ready { let a0 = acts(cx); let a = with_boards(cx, light, viewer, pv, a0); in_corner(div().size_full().flex().flex_col().child(model).into_any_element(), a) }
            else { let a = acts(cx); status_row(light, "Loading the model…", Some(a)) }
        }
        else if open == "browser" { let a = acts(cx); match browser { Some(b) => in_corner(div().size_full().child(b.clone()).into_any_element(), a), None => status_row(light, "The browser page needs a connected hub.", Some(a)) } }
        else { let a = acts(cx); status_row(light, "Opening…", Some(a)) };
    if matches!(open, "files" | "image" | "pdf" | "text") && !diff_on { div().id("viewer-body").size_full().child(body).into_any_element() } else { div().id("viewer-body").size_full().overflow_y_scroll().child(body).into_any_element() }
}
/// The pane around the page on show: its width, the left edge's drag and border; `page` (the page itself, `page()`, drawn as a part of the
/// window by ui.rs) fills the rest, so the pane's frame is known without building the page.
pub fn pane(cx: &mut Context<Backplane>, light: bool, thread: &Value, page: AnyElement) -> Option<AnyElement> {
    if !is_open(thread) { return None; }
    let c = |n: &str| theme::color(light, n);
    let open = thread["viewer"]["open"].as_str().unwrap_or("");
    // a Split holds three pages at once, so it takes the room of a desktop: its pane is at least that wide, whatever this device chose for the others
    let width = if open == "split" { (pane_width() as f32).max(960.) } else { pane_width() as f32 };
    // The pane gives way to the thread: it shrinks (down to its least) before the composer row is squeezed, so Attach and Send stay reachable
    Some(div().w(px(width)).max_w(relative(0.62)).min_w(px(crate::local::VIEWER_PX.0 as f32)).h_full().flex_shrink().relative().border_l_1().border_color(c("rule")).flex().flex_col()
        .child(resize_handle(cx)).child(page).into_any_element())
}

/// The pane's width: this device's choice (the left edge's drag), else the default; read once from the local file, then kept here.
static PANE_W: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);
pub fn pane_width() -> u32 {
    use std::sync::atomic::Ordering::Relaxed;
    if PANE_W.load(Relaxed) == u32::MAX { PANE_W.store(crate::local::Local::load().viewer_px, Relaxed); }
    match PANE_W.load(Relaxed) { 0 => crate::local::VIEWER_PX_DEFAULT, w => w }
}
/// The width a drag to window x `x` makes (the pane runs from there to the rail on the right), kept in range and leaving the thread at least THREAD_MIN wide
/// (its composer row, Attach and Send included, measured at 1400 px: about 550) beside the sidebar.
pub const THREAD_MIN: f32 = 560.0;
pub fn width_for(window_w: f32, x: f32, rail: f32, sidebar: f32) -> u32 {
    let most = ((window_w - rail - sidebar - THREAD_MIN) as i64).max(crate::local::VIEWER_PX.0 as i64).min(crate::local::VIEWER_PX.1 as i64);
    ((window_w - rail - x).round() as i64).clamp(crate::local::VIEWER_PX.0 as i64, most) as u32
}
struct ResizeDrag;
struct Ghost;
impl Render for Ghost { fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div() } }
fn set_pane(this: &mut Backplane, w: u32, cx: &mut Context<Backplane>) {
    PANE_W.store(w, std::sync::atomic::Ordering::Relaxed);
    this.local.viewer_px = w; this.local.save(); this.ui_epoch += 1; cx.notify();
}
/// The pane's left edge: drag to resize, double click for the default (the web's split).
fn resize_handle(cx: &mut Context<Backplane>) -> AnyElement {
    div().id("viewer-resize").absolute().left(px(-3.)).top_0().h_full().w(px(7.)).cursor_col_resize().hover(|d| d.bg(rgba(0x80808040)))
        .on_drag(ResizeDrag, |_, _, _, cx| cx.new(|_| Ghost))
        .on_drag_move(cx.listener(|this, e: &DragMoveEvent<ResizeDrag>, window, cx| {
            let w = width_for(f32::from(window.viewport_size().width), f32::from(e.event.position.x), 71.0, if this.sidebar_open { this.local.sidebar_px as f32 } else { 0.0 });
            set_pane(this, w, cx);
        }))
        .on_click(cx.listener(|this, e: &ClickEvent, _, cx| { if e.click_count() >= 2 { set_pane(this, 0, cx); } }))
        .into_any_element()
}

/// The watch a Mech part needs. Board, Schematic and 3D are watched by the commands of the action that opened them; a part opened by the Mech page (a STEP the
/// listing then picks) comes in after a hub reply, which carries no command, so the window asks for its model itself when the viewer's key moves.
/// `key` is the viewer's source "<kind>|<root>|<path>" (kind 0 board, 1 schematic, 2 3D).
pub fn watch_for(open: &str, key: &str, last: &str) -> Option<Value> {
    if open != "mech" || key.is_empty() || key == last { return None }
    let mut p = key.splitn(3, '|');
    let (k, root, path) = (p.next()?, p.next()?, p.next()?);
    let kind = match k { "0" => "board", "1" => "schematic", "2" => "3d", _ => return None };
    Some(serde_json::json!({"kind": kind, "root": root, "path": path, "tree": ""}))
}
static WATCHED: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
pub fn follow(tx: &tokio::sync::mpsc::Sender<Command>, thread: &Value) {
    let (open, key) = (thread["viewer"]["open"].as_str().unwrap_or(""), thread["viewer"]["key"].as_str().unwrap_or(""));
    let mut last = WATCHED.lock().unwrap();
    if open != "mech" { if !last.is_empty() { last.clear(); } return }
    if let Some(p) = watch_for(open, key, &last) {
        if tx.try_send(Command::Rpc("kicad.watch".into(), super::settings::Secret(p), "watch")).is_ok() { *last = key.to_string(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn split_panes_are_put_away_and_brought_back_one_at_a_time_and_the_last_one_stays() {
        assert!(split_shows("", "sch") && split_shows("", "pcb") && split_shows("", "3d"));
        assert_eq!(split_flip("", "pcb"), "pcb");
        assert_eq!(split_flip("pcb", "3d"), "pcb 3d");
        assert_eq!(split_flip("pcb 3d", "sch"), "pcb 3d", "the last pane shown stays");
        assert_eq!(split_flip("pcb 3d", "pcb"), "3d");
        assert_eq!(split_flip("3d", "3d"), "");
        assert_eq!(split_flip("pcb", "nope"), "pcb");
        assert!(!split_shows("sch 3d", "3d") && split_shows("sch 3d", "pcb"));
    }
    #[test] fn a_split_divider_keeps_each_pane_room_for_its_head() {
        assert!((split_share(0., true, 1000.) - 1.15 / 2.15).abs() < 1e-6, "the schematic's default share is 0.14's");
        assert_eq!(split_share(0., false, 1000.), 0.5);
        assert_eq!(split_share(0.05, true, 1000.), 0.14, "140 px of 1000 at least");
        assert_eq!(split_share(0.99, false, 1000.), 0.86);
        assert_eq!(split_share(0.3, true, 200.), 0.45, "a small pane: never less than 45 percent each way");
        assert!((share_at(600., 100., 1000.) - 0.5).abs() < 1e-6);
    }
    #[test] fn the_probe_chip_names_a_part_by_its_reference() {
        let c = super::super::plot::card("fp_poly;ref=C5;value=47uF 100V;net=GND").unwrap();
        assert_eq!(probe_words(&c), vec!["C5", "47uF 100V", "GND"]);
        let n = super::super::plot::card("track;net=/NRST").unwrap();
        assert_eq!(probe_words(&n), vec!["track", "/NRST"]);
    }
    #[test] fn open_in_kicad_names_the_machine_when_the_hub_is_another() {
        assert_eq!(kicad_open_label(Some("127.0.0.1:3797"), None), "Open in KiCad"); assert_eq!(kicad_open_label(None, None), "Open in KiCad"); assert_eq!(kicad_open_label(Some("127.0.0.1:3797"), Some("desktop")), "Open in KiCad on desktop"); assert_eq!(kicad_open_label(None, Some("")), "Open in KiCad");
        assert_eq!(kicad_open_label(Some("desktop.tail9e0cf2.ts.net:8451"), None), "Open in KiCad on desktop");
    }
    #[test] fn open_in_kicad_is_offered_with_a_board_or_a_schematic() {
        assert!(has_design(&serde_json::json!({"choices":[{"label":"Board","value":"board"},{"label":"Files","value":"files"}]})));
        assert!(has_design(&serde_json::json!({"choices":[{"label":"Schematic","value":"schematic"}]})));
        assert!(!has_design(&serde_json::json!({"choices":[{"label":"Files","value":"files"}]}))); assert!(!has_design(&serde_json::json!({})));
    }
    #[test] fn a_pane_names_what_it_shows() {
        let j = |s: &str| serde_json::from_str::<Value>(s).unwrap();
        assert_eq!(source(&j(r#"{"open":"board","key":"0|/p|","boards":[{"label":"boards/a","value":"boards/a.kicad_pcb","on":false},{"label":"boards/x","value":"boards/x.kicad_pcb","on":true}]}"#), false), "boards/x.kicad_pcb");
        assert_eq!(source(&j(r#"{"open":"board","key":"0|/p|/p/hw/y.kicad_pcb"}"#), false), "y.kicad_pcb", "a board opened from Files");
        assert_eq!(source(&j(r#"{"open":"schematic","key":"1|/p|","choices":[{"label":"Schematic","value":"schematic","on":true}]}"#), false), "Schematic", "the project's own: the page's name");
        assert_eq!(source(&j(r#"{"open":"files","files":{"trail":[{"label":"Project"},{"label":"boards"},{"label":"x"}]}}"#), false), "boards/x");
        assert_eq!(source(&j(r#"{"open":"files","files":{"trail":[{"label":"Project"}]},"choices":[{"label":"Files","value":"files","on":true}]}"#), false), "Files");
        assert_eq!(source(&j(r#"{"open":"image","file":{"name":"shot.png"}}"#), false), "shot.png");
        assert_eq!(source(&j(r#"{"open":"board"}"#), true), "Diff");
        assert!(matches!(kicad_open(), Command::Action(a, v) if a == "kicad-open" && v.is_empty()));
    }
    #[test] fn tabs_files_and_diff_parse_from_the_hubs_rows() {
        let v = j(r#"{"open":"files","choices":[{"label":"Board","value":"board","on":false},{"label":"Files","value":"files","on":true}],
            "files":{"trail":[{"label":"project","action":"fm-cd","value":""}],"rows":[{"label":"src","action":"fm-cd","value":"src","detail":"","kind":"folder","icon":"folder"},{"label":"a.rs","action":"fm-open","value":"a.rs","detail":"1 KB","kind":"text","icon":"file"}],"ready":true}}"#);
        assert_eq!(tabs(&v).iter().map(|t| (t.value.as_str(), t.on)).collect::<Vec<_>>(), [("board", false), ("files", true)]);
        assert_eq!(crumbs(&v["files"])[0].action, "fm-cd");
        let r = file_rows(&v["files"]); assert_eq!((r[0].kind.as_str(), r[1].detail.as_str()), ("folder", "1 KB"));
        let d = diff_files(&j(r#"{"summary":"1 file","files":[{"name":"a.rs","status":"M","lines":[{"k":4,"t":"@@ -1 +1 @@","o":"","n":""},{"k":2,"t":"old","o":"1","n":""},{"k":1,"t":"new","o":"","n":"1"}]}]}"#));
        assert_eq!(d[0].lines.len(), 3); assert_eq!((diff_sign(d[0].lines[1].k), diff_sign(d[0].lines[2].k), diff_sign(4)), ("-", "+", " "));
    }
    #[test] fn file_rows_take_the_icon_of_what_they_open_as_and_listed_only_ones_are_dim() {
        assert_eq!((file_icon("board"), file_icon("schematic"), file_icon("folder"), file_icon("pdf"), file_icon("text"), file_icon("")), ("circuit-board", "cpu", "folder", "file-text", "file", "file"));
        assert!(file_listed_only("") && file_listed_only("other") && !file_listed_only("text") && !file_listed_only("board") && !file_listed_only("folder"));
    }
    #[test] fn a_rail_press_on_the_lit_kind_shuts_the_viewer_and_any_other_opens_it() {
        let shut = |c: Command| matches!(c, Command::Action(a, v) if a == "view" && v.is_empty());
        let opens = |c: Command, k: &str| matches!(c, Command::Action(a, v) if a == "view" && v == k);
        let board = j(r#"{"open":"board","choices":[{"label":"Board","value":"board","on":true},{"label":"Files","value":"files","on":false}]}"#);
        assert!(shut(rail_command(&board, false, None, "board")), "Board on show, Board pressed: the viewer shuts");
        assert!(opens(rail_command(&board, false, None, "schematic"), "schematic"), "another kind opens");
        assert!(opens(rail_command(&board, true, None, "board"), "board"), "Diff covers the board: Board shows it again");
        let closed = j(r#"{"open":"","choices":[{"label":"Board","value":"board","on":false}]}"#);
        assert!(opens(rail_command(&closed, false, None, "board"), "board") && opens(rail_command(&Value::Null, false, None, "board"), "board"), "nothing open: it opens");
        // pressed and not answered yet: the rail already shows it lit, so a second press shuts it; a press after Close opens again
        assert!(shut(rail_command(&closed, false, Some("board"), "board")));
        assert!(opens(rail_command(&board, false, Some(""), "board"), "board"), "Close pressed, the hub not caught up: Board opens");
        // pages with no tab (Split, Browser) and a file opened from Files are lit and shut as well
        assert!(shut(rail_command(&j(r#"{"open":"split","choices":[{"label":"Board","value":"board","on":false}]}"#), false, None, "split")));
        assert!(shut(rail_command(&j(r#"{"open":"browser"}"#), false, None, "browser")));
        assert!(shut(rail_command(&j(r#"{"open":"image","choices":[{"label":"Files","value":"files","on":true}]}"#), false, None, "files")));
        assert_eq!(active_kind(&j(r#"{"open":"pdf"}"#), None), "files");
    }
    #[test] fn opening_and_closing_are_the_idempotent_view_action() {
        for c in [view_kind("board"), view_kind("board"), view_kind("schematic")] { match c { Command::Action(a, v) => { assert_eq!(a, "view"); assert!(v == "board" || v == "schematic"); } c => panic!("{c:?}") } }
        match close() { Command::Action(a, v) => assert_eq!((a.as_str(), v.as_str()), ("view", "")), c => panic!("{c:?}") }
    }
    #[test] fn the_3d_page_is_put_away_when_no_page_draws_it() {
        let t = |v: &str| j(v);
        assert!(shows_solid(&t(r#"{"viewer":{"open":"3d"}}"#)) && shows_solid(&t(r#"{"viewer":{"open":"mech"}}"#)) && shows_solid(&t(r#"{"viewer":{"open":"split"}}"#)));
        assert!(!shows_solid(&t(r#"{"viewer":{"open":"board"}}"#)), "another page");
        assert!(!shows_solid(&t(r#"{"viewer":{"open":""}}"#)) && !shows_solid(&Value::Null), "the pane closed, or no thread");
        assert!(!shows_solid(&t(r#"{"viewer":{"open":"3d"},"diff":{}}"#)), "a diff drawn over it");
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
    #[test] fn a_mech_part_is_watched_once_when_its_key_moves() {
        let key = "2|/p|/p/mech/board.step";
        assert_eq!(watch_for("mech", key, "").unwrap(), serde_json::json!({"kind": "3d", "root": "/p", "path": "/p/mech/board.step", "tree": ""}));
        assert!(watch_for("mech", key, key).is_none(), "asked once");
        assert!(watch_for("mech", "", "").is_none() && watch_for("board", key, "").is_none(), "no key, or a page the open action already watched");
    }
    #[test] fn a_drag_makes_the_pane_as_wide_as_the_room_right_of_the_pointer_within_its_range() {
        assert_eq!(width_for(1600.0, 900.0, 71.0, 264.0), 629); assert_eq!(width_for(1600.0, 1500.0, 71.0, 264.0), crate::local::VIEWER_PX.0);
        assert_eq!(width_for(1600.0, 0.0, 71.0, 264.0), 705, "the thread keeps 560"); assert_eq!(width_for(1600.0, 0.0, 71.0, 0.0), 969);
    }
    #[test] fn the_panel_is_open_for_a_page_or_a_diff_only() {
        assert!(is_open(&j(r#"{"viewer":{"open":"board"}}"#))); assert!(is_open(&j(r#"{"diff":{"summary":"","files":[]}}"#)));
        assert!(!is_open(&j(r#"{"viewer":{"open":""},"diff":null}"#))); assert!(!is_open(&Value::Null));
    }

    #[test] fn a_viewer_show_opens_the_page_its_kind_or_file_names() {
        assert_eq!(shown("board", ""), Some(("board".into(), None)));
        assert_eq!(shown("mech", "/p/mech/a.step"), Some(("mech".into(), Some("/p/mech/a.step".into()))));
        assert_eq!(shown("3d", "/p/mech/a.glb").map(|x| x.0), Some("mech".into()), "a model shown as 3D is Mech's part");
        assert_eq!(shown("", "/p/x.kicad_sch").map(|x| x.0), Some("schematic".into()));
        assert_eq!(shown("nonsense", "/p/readme.txt"), None);
    }
}

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

/// Open a viewer kind, or close the viewer with "": the phones' idempotent `view` action. The client never toggles the panel and never
/// guesses its state: pressing twice before the reply asks for the same thing twice.
pub fn view_kind(kind: &str) -> Command { Command::Action("view".into(), kind.to_string()) }
pub fn close() -> Command { view_kind("") }
/// Open in KiCad: the hub starts KiCad on the thread's design (kicad/open.rs), offered while the viewer has a board or schematic.
pub fn kicad_open() -> Command { Command::Action("kicad-open".into(), String::new()) }
/// The button's words. KiCad opens where the design is, on the hub's own machine: with a hub on another machine the button says whose screen.
pub fn kicad_open_label(hub_addr: Option<&str>) -> String {
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

/// A row's icon by what it opens as.
pub fn file_icon(kind: &str) -> &'static str {
    match kind { "folder" => "folder", "board" => "circuit-board", "schematic" => "cpu", "3d" => "box", "image" => "image", "pdf" => "file-text", _ => "file" }
}
/// A file the viewer cannot open (listed only) is drawn dim, as the web does.
pub fn file_listed_only(kind: &str) -> bool { !matches!(kind, "folder" | "board" | "schematic" | "3d" | "image" | "pdf" | "text") }

/// diff.revert.confirm (i18n/en.msg)
const REVERT_ASK: &str = "Put the files back as they were when this thread began? Every change shown below is undone, committed ones and new files too. Commits stay in git.";

fn diff_page(cx: &mut Context<Backplane>, light: bool, diff: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().text_size(px(12.));
    col = col.child(div().flex().gap_2().px_3().py_2().items_center().child(div().flex_1().text_color(c("dim")).child(string(diff, "summary")))
        .children([("Commit", "commit_push"), ("PR", "commit_push_pr")].into_iter().map(|(l, v)| btn(cx, light, format!("git-{v}"), l.to_string(), false, Command::Action("git".into(), v.into()))).collect::<Vec<_>>())
        .child(btn(cx, light, "revert-ask".into(), "Revert".into(), diff["reverting"] == true, Command::Action("revert".into(), "ask".into()))));
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
pub fn boards(viewer: &Value) -> Vec<(String, String, bool)> {
    viewer["boards"].as_array().into_iter().flatten().map(|b| (string(b, "label"), string(b, "value"), b["on"].as_bool().unwrap_or(false))).collect()
}
fn plot_pane(cx: &mut Context<Backplane>, light: bool, slot: usize, open: &str, viewer: &Value, frame: Option<&super::plot::Frame>, pv: &mut super::plotview::PlotView, layers: bool, sheets: &[super::plot::Sheet]) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let loading = || div().p_3().text_color(c("dim")).child(if open == "board" { "Loading the board…" } else { "Loading the schematic…" }).into_any_element();
    let Some(frame) = frame else { return loading() };
    // the frame must be the source this viewer asks for: another project's board stays off the screen until its own plot arrives
    let mut want = string(viewer, "key");
    // a comparison on show is its own source: `diff\n<old>\n<live>` after the key; another page puts it away
    if let Some(cmp) = pv.compare.clone() { if cmp.base == want { want = format!("{want}{}", super::histbar::shown(&cmp)); } else { pv.compare = None; } }
    if !want.is_empty() && !frame.key.is_empty() && frame.key != want { return loading() }
    if !frame.none.is_empty() { return div().p_3().text_color(c("dim")).child(frame.none.clone()).into_any_element(); }
    if pv.focus.is_none() { pv.focus = Some(cx.focus_handle()); }
    let (w, h) = pv.bounds.map_or((480., 600.), |b| (f32::from(b.size.width), f32::from(b.size.height)));
    let view = super::plotview::ensure_view(pv, frame, w, h);
    let mut col = div().size_full().flex().flex_col();
    let note = string(viewer, "note");
    if !note.is_empty() { col = col.child(div().px_3().py_1().text_size(px(11.)).text_color(c("dim")).child(note)); }
    // Boards: a project with several lists them as chips, the one on show marked; a press shows that board (and its schematic)
    let boards = boards(viewer);
    if boards.len() > 1 && matches!(open, "board" | "schematic" | "3d") {
        col = col.child(div().id("boards-row").flex().flex_wrap().gap_1().px_3().py_1().children(boards.into_iter().enumerate().map(|(i, (label, value, on))| {
            let d = div().id(SharedString::from(format!("board-{i}"))).px_2().py_1().text_size(px(12.)).cursor_pointer().border_1();
            let d = if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
            d.child(label).on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-board".into(), value.clone())); cx.notify(); })).into_any_element()
        }).collect::<Vec<_>>()));
    }
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
    // History and Compare (screens::histbar): steps, versions, Old/Diff/New
    if layers && matches!(open, "board" | "schematic") { col = col.child(super::histbar::bar_el(cx, light, pv)); }
    if layers {
        let fit = div().id(SharedString::from(format!("plot-fit-{slot}"))).px_2().py_1().mt_1().ml_1().text_size(px(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).hover(|s| s.bg(c("panel"))).child("Fit")
            .on_click(cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.touched = false; pv.fit_wh = (0., 0.); cx.notify(); }));
        col = col.child(div().flex().items_center().child(crate::viewers::layers::strip_el(cx, light, &strip, &pv.layers_key)).child(fit));
    }
    let mut area = div().relative().flex().flex_col().flex_1().min_h_0().child(super::plotview::canvas_el(cx, slot, pv.focus.clone(), light, frame, view, crate::viewers::layers::masks(viewer)));
    if layers { if let Some(p) = crate::viewers::layers::panel_el(cx, light, &strip, pv.layers_open, pv.layers_cap.unwrap_or(280.), &pv.layers_scroll) { area = area.child(p); } }
    // the card of the piece last tapped (Insp.card): what it is, then key = value rows; a tap on nothing, or Close, puts it away
    if let Some(card) = pv.card.clone() {
        let mut box_ = div().id(SharedString::from(format!("pick-card-{slot}"))).absolute().left_2().bottom_2().max_w(px(300.)).flex().flex_col().gap_1().p_2().text_size(px(12.)).border_1().border_color(c("line")).bg(c("panel")).occlude()
            .child(div().flex().gap_2().child(div().flex_1().font_weight(FontWeight::BOLD).child(card.title.clone()))
                .child(div().id(SharedString::from(format!("pick-close-{slot}"))).cursor_pointer().text_color(c("dim")).child("×").on_click(cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.card = None; pv.picked = None; cx.notify(); }))));
        for (k, v) in card.rows.iter().take(14) { box_ = box_.child(div().flex().gap_2().child(div().w(px(70.)).text_color(c("dim")).child(k.clone())).child(div().flex_1().min_w_0().child(v.clone()))); }
        area = area.child(box_);
    }
    col.child(area).into_any_element()
}

/// Board or schematic: the plot the hub sent for this file.
fn plot_page(cx: &mut Context<Backplane>, light: bool, open: &str, viewer: &Value, plot: &super::plot::Store, pv: &mut super::plotview::PlotView) -> AnyElement {
    plot_pane(cx, light, 0, open, viewer, plot.frame.as_ref(), pv, true, &plot.sheets)
}

/// Split: the schematic across the top, the board and its 3D model beneath it, each its own page of `viewer.panes` (key, camera), all watched at once.
fn split_page(cx: &mut Context<Backplane>, light: bool, viewer: &Value, plot: &super::plot::Store, pvs: &mut [super::plotview::PlotView; 2], solid: Option<&Entity<crate::viewers::solid::SolidPage>>, have: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let panes: Vec<&Value> = viewer["panes"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if panes.len() < 3 { return div().p_3().text_color(c("dim")).child("Opening…").into_any_element(); }
    let [sch_pv, brd_pv] = pvs;
    let tag = |label: &str| div().absolute().top_1().left_2().px_1().text_size(px(10.)).text_color(c("dim")).child(label.to_string());
    let sch = div().relative().w_full().h(relative(0.5)).border_b_1().border_color(c("line")).child(plot_pane(cx, light, 1, "schematic", panes[0], plot.frame_for(&string(panes[0], "key")), sch_pv, false, &[])).child(tag("Schematic"));
    let brd = div().relative().flex_1().min_w_0().h_full().border_r_1().border_color(c("line")).child(plot_pane(cx, light, 2, "board", panes[1], plot.frame_for(&string(panes[1], "key")), brd_pv, false, &[])).child(tag("Board"));
    let want3 = string(panes[2], "key");
    let model: AnyElement = match solid { Some(s) if solid_ready(&want3, have) && !have.is_empty() => div().size_full().child(s.clone()).into_any_element(), _ => div().p_3().text_color(c("dim")).child("Loading the model…").into_any_element() };
    let d3 = div().relative().flex_1().min_w_0().h_full().child(model).child(tag("3D"));
    div().size_full().flex().flex_col().child(sch).child(div().flex().flex_1().min_h_0().child(brd).child(d3)).into_any_element()
}

fn review_page(cx: &mut Context<Backplane>, reg: &Reg, light: bool, review: &Value) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().gap_2().p_3();
    col = col.child(div().flex().flex_wrap().gap_2().children(arr(review, "tabs").enumerate().map(|(i, t)| btn(cx, light, format!("rv-{i}"), string(t, "label"), t["on"].as_bool().unwrap_or(false), Command::Action("rv-page".into(), string(t, "page")))).collect::<Vec<_>>()));
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
pub fn view(cx: &mut Context<Backplane>, reg: &Reg, light: bool, files_ui: &super::filesui::FilesUi, images: &super::images::Images, target: Option<&super::attach::Target>, thread: &Value, plot: &super::plot::Store, pv: &mut super::plotview::PlotView, split_pv: &mut [super::plotview::PlotView; 2], solid: Option<&Entity<crate::viewers::solid::SolidPage>>, browser: Option<&Entity<crate::viewers::browser::BrowserPage>>) -> Option<AnyElement> {
    if !is_open(thread) { return None; }
    let c = |n: &str| theme::color(light, n);
    let viewer = &thread["viewer"];
    let open = viewer["open"].as_str().unwrap_or("");
    let diff_on = thread["diff"].is_object();
    let mut bar = div().h(px(34.)).flex_shrink_0().flex().items_center().gap_1().px_2().border_b_1().border_color(c("line"));
    for (i, t) in tabs(viewer).into_iter().enumerate() { bar = bar.child(btn(cx, light, format!("vtab-{i}"), t.label, t.on && !diff_on, view_kind(&t.value))); }
    if has_design(viewer) { bar = bar.child(div().flex_1()).child(btn(cx, light, "kicad-open".into(), kicad_open_label(target.map(|t| t.addr.as_str())), false, kicad_open())); } else { bar = bar.child(div().flex_1()); }
    bar = bar.child(div().id("viewer-close").px_2().cursor_pointer().text_color(c("dim")).hover(|s| s.text_color(c("text"))).child("Close")
        .on_click(cx.listener(|this, _, _, cx| { let _ = this.tx.try_send(close()); cx.notify(); })));
    let body: AnyElement = if diff_on { diff_page(cx, light, &thread["diff"]) }
        else if open == "files" && viewer["files"].is_object() { super::filesui::files_page(cx, light, &viewer["files"], files_ui) }
        else if matches!(open, "image" | "pdf" | "text") && viewer["file"].is_object() { super::filesui::preview_page(cx, reg, light, open, viewer, files_ui, images, target) }
        else if open == "review" && viewer["review"].is_object() { review_page(cx, reg, light, &viewer["review"]) }
        else if matches!(open, "board" | "schematic") { plot_page(cx, light, open, viewer, plot, pv) }
        else if open == "split" { let have = solid.map(|s| s.read(cx).key().to_string()).unwrap_or_default(); split_page(cx, light, viewer, plot, split_pv, solid, &have) }
        else if matches!(open, "3d" | "mech") {
            let key_empty = string(viewer, "key").is_empty();
            let want = string(viewer, "key");
            // what the page HOLDS, not what was queued for it: a big model's decode, a frame that fails or is replaced in the worker's slot
            let have = solid.map(|s| s.read(cx).key().to_string()).unwrap_or_default();
            let model: AnyElement = if open == "mech" && key_empty { div().into_any_element() } else { match solid { Some(s) if solid_ready(&want, &have) => div().flex_1().min_h_0().child(s.clone()).into_any_element(), _ => div().p_3().text_color(c("dim")).child("Loading the model…").into_any_element() } };
            if open == "mech" { div().size_full().flex().flex_col().child(mech_chrome(cx, light, viewer, key_empty)).child(model).into_any_element() } else { div().size_full().flex().flex_col().child(model).into_any_element() }
        }
        else if open == "browser" { match browser { Some(b) => div().size_full().child(b.clone()).into_any_element(), None => div().p_3().text_color(c("dim")).child("The browser page needs a hub on plain http://.").into_any_element() } }
        else { div().p_3().text_color(c("dim")).child("Opening…").into_any_element() };
    // a Split holds three pages at once, so it takes the room of a desktop: its pane is at least that wide, whatever this device chose for the others
    let width = if open == "split" { (pane_width() as f32).max(960.) } else { pane_width() as f32 };
    // The pane gives way to the thread: it shrinks (down to its least) before the composer row is squeezed, so Attach and Send stay reachable
    Some(div().w(px(width)).max_w(relative(0.62)).min_w(px(crate::local::VIEWER_PX.0 as f32)).h_full().flex_shrink().relative().border_l_1().border_color(c("line")).flex().flex_col()
        .child(resize_handle(cx))
        .child(bar).child(if matches!(open, "files" | "image" | "pdf" | "text") && !diff_on { div().id("viewer-body").flex_1().min_h_0().child(body) } else { div().id("viewer-body").flex_1().min_h_0().overflow_y_scroll().child(body) }).into_any_element())
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
    #[test] fn open_in_kicad_names_the_machine_when_the_hub_is_another() {
        assert_eq!(kicad_open_label(Some("127.0.0.1:3797")), "Open in KiCad"); assert_eq!(kicad_open_label(None), "Open in KiCad");
        assert_eq!(kicad_open_label(Some("desktop.tail9e0cf2.ts.net:8451")), "Open in KiCad on desktop");
    }
    #[test] fn open_in_kicad_is_offered_with_a_board_or_a_schematic() {
        assert!(has_design(&serde_json::json!({"choices":[{"label":"Board","value":"board"},{"label":"Files","value":"files"}]})));
        assert!(has_design(&serde_json::json!({"choices":[{"label":"Schematic","value":"schematic"}]})));
        assert!(!has_design(&serde_json::json!({"choices":[{"label":"Files","value":"files"}]}))); assert!(!has_design(&serde_json::json!({})));
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

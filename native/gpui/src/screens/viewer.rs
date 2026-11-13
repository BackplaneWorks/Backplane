//! The viewer panel beside the thread: the tabs the hub lists (`thread.viewer.choices`) and the page it has open.
//! Files (folder rows), Diff (the thread's changes) and Review (the report as Markdown, page chips) are drawn here from
//! the hub's rows; every press is the view action the row names. The board, schematic and 3D pages draw plot chunks
//! (screens::plot, next). Pure parsing below, tested; the element code only lays the rows out.
use super::{md, timeline::{self, Reg}};
use crate::{hub::Command, i18n::{tf, trs}, string, theme, Backplane};
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
    let cmd = if lit && !kind.is_empty() { close() } else { view_kind(kind) };
    if let Command::Action(_, k) = &cmd { PRESSED.with(|p| *p.borrow_mut() = Some((k.clone(), std::time::Instant::now()))); }
    cmd
}
/// The page a rail press just asked for ("" a close), until the hub's picture shows it (at most 2 s): the pane opens (or shuts) at the press, laid out
/// as that page will be, instead of after the hub's round trip (11-05: a viewer opens at the moment of the press).
/// (the window's own thread: presses and pictures are both handled there)
thread_local! { static PRESSED: std::cell::RefCell<Option<(String, std::time::Instant)>> = const { std::cell::RefCell::new(None) }; }
pub fn pressed(now_open: &str) -> Option<String> {
    PRESSED.with(|p| { let mut g = p.borrow_mut();
    let Some((k, at)) = g.clone() else { return None };
    // answered (the hub shows that page, or shut it), or too old to be waiting still
    if at.elapsed() > std::time::Duration::from_secs(2) || (!k.is_empty() && opened_as(now_open, &k)) || (k.is_empty() && now_open.is_empty()) { *g = None; return None }
    Some(k) })
}
/// Whether the hub's page `open` is what a press on `kind` asked for (a Files press may land on a file it opened).
fn opened_as(open: &str, kind: &str) -> bool { open == kind || (kind == "files" && matches!(open, "image" | "pdf" | "text")) }
/// Open in KiCad: the hub starts KiCad on the thread's design (kicad/open.rs), offered while the viewer has a board or schematic.
/// Open in KiCad names what the viewer draws (its key, `<kind>|<root>|<file>`; a split's first pane), so the hub opens exactly that.
pub fn kicad_open(viewer: &Value) -> Command { Command::Action("kicad-open".into(), shown_key(viewer)) }
pub fn shown_key(viewer: &Value) -> String {
    let k = string(viewer, "key");
    if !k.is_empty() && !k.starts_with("split|") { return k }
    arr(viewer, "panes").map(|p| string(p, "key")).find(|k| !k.is_empty()).unwrap_or(k)
}
/// The button's words. KiCad opens where the design is, on the hub's own machine: with a hub on another machine the button says whose screen.
pub fn kicad_open_label(hub_addr: Option<&str>, thread_machine: Option<&str>) -> String {
    // a thread of another machine (its mark names it) opens there, whichever hub the window is on
    if let Some(m) = thread_machine.filter(|m| !m.is_empty()) { return tf("gpui.c.kicad.open.on", &[("machine", m.into())]) }
    match hub_addr.map(|a| a.rsplit_once(':').map_or(a, |(h, _)| h)) {
        Some(h) if !h.is_empty() && !matches!(h, "127.0.0.1" | "localhost" | "[::1]") => tf("gpui.c.kicad.open.on", &[("machine", h.split('.').next().unwrap_or(h).into())]),
        _ => trs("Open in KiCad"),
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
        .unwrap_or_else(|| match viewer["open"].as_str().unwrap_or("") { "image" | "pdf" | "text" => "files".into(), "board" if viewer["gerbers"] == true => "gerbers".into(), o => o.to_string() })
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
/// Whether the pane is up: a page the hub has open, or a rail press it has not answered yet (`pane_page`): the pane opens and shuts at the press.
pub fn is_open(thread: &Value) -> bool { pane_page(thread).is_some() }
/// Whether the hub's picture has a page open (Diff included).
fn hub_open(thread: &Value) -> bool { thread["diff"].is_object() || thread["viewer"]["open"].as_str().map_or(false, |o| !o.is_empty()) }

/// A viewer page's bar, its first row: the thread header's height (head::ROW_H) and its structural rule (--rule), so the pane's line is the
/// one the sidebar's first row and the thread header draw across the window (the top bar's bottom + ROW_H). Its buttons are the header's
/// tools' height (head::BTN_H, `tall`), centred on that band.
pub fn bar(light: bool) -> Div {
    div().flex_shrink_0().h(px(super::head::ROW_H)).flex().items_center().gap_2().px_3().border_b_1().border_color(theme::color(light, "rule"))
}
/// The bar of a page whose buttons may not fit one line (Files, Diff, Review, Mech): one line on the header's band while they fit, else
/// it wraps and grows down from there (its first line stays on the band).
pub fn bar_wrap(light: bool) -> Div {
    div().flex_shrink_0().min_h(px(super::head::ROW_H)).flex().flex_wrap().items_center().gap_x_2().gap_y_1().px_3().py(px(3.5)).border_b_1().border_color(theme::color(light, "rule"))
}
/// A button of a viewer bar: the header's tool height, its content centred.
pub fn tall<E: Styled>(e: E) -> E { e.h(px(super::head::BTN_H)).flex().items_center() }

fn btn(cx: &mut Context<Backplane>, light: bool, id: String, label: String, on: bool, cmd: Command) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let d = tall(div().id(SharedString::from(id)).px_2().text_size(px(12.)).cursor_pointer().border_1());
    let d = if on { d.bg(c("accent")).text_color(c("on-accent")).border_color(c("accent")) } else { d.bg(c("raise")).border_color(c("line")).hover(|s| s.bg(c("panel"))) };
    let cmd = std::cell::RefCell::new(Some(cmd));
    d.child(label).on_click(cx.listener(move |this, _, _, cx| { if let Some(Command::Action(a, v)) = cmd.borrow().as_ref().map(|c| match c { Command::Action(a, v) => Command::Action(a.clone(), v.clone()), _ => Command::Page(String::new()) }) { let _ = this.tx.try_send(Command::Action(a, v)); } cx.notify(); })).into_any_element()
}

/// Mech: a chip per part (and Renders) over the part's 3D; with no part chosen the page says what there is (or why not) and lists them.
fn mech_chrome(cx: &mut Context<Backplane>, light: bool, viewer: &Value, key_empty: bool, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mech = &viewer["mech"];
    let parts = mech_parts(viewer);
    let say = if key_empty { [string(mech, "say"), string(mech, "empty"), string(mech, "note")].into_iter().find(|s| !s.is_empty()) } else { None };
    let mut row = div().flex().flex_wrap().items_center().gap_2();
    for (i, p) in parts.iter().enumerate() { row = row.child(btn(cx, light, format!("mech-part-{i}"), p.label.clone(), p.on, Command::Action("mech-part".into(), p.value.clone()))); }
    // (no Renders button: the renders page is not drawn here yet, and an offered button must lead somewhere)
    row = row.child(btn(cx, light, "mech-refresh".into(), trs("Refresh"), false, Command::Action("mech-refresh".into(), String::new())));
    // the bar: the parts, the pane's controls at the end of its first line (the parts wrap under them); what the page says under the bar
    let bar = bar_wrap(light).flex_nowrap().items_start().child(div().flex_1().min_w_0().child(row)).child(actions);
    div().flex().flex_col().child(bar).children(say.map(|s| div().px_3().py_2().text_size(px(12.)).text_color(c("dim")).child(trs(&s)))).into_any_element()
}

/// A row's icon by what it opens as.
pub fn file_icon(kind: &str) -> &'static str {
    match kind { "folder" => "folder", "board" => "circuit-board", "schematic" => "cpu", "3d" => "box", "image" => "image", "pdf" => "file-text", _ => "file" }
}
/// A file the viewer cannot open (listed only) is drawn dim, as the web does.
pub fn file_listed_only(kind: &str) -> bool { !matches!(kind, "folder" | "board" | "schematic" | "3d" | "image" | "pdf" | "text") }

/// diff.revert.confirm (i18n/en.msg)
/* i18n-ok: drawn through trs */ const REVERT_ASK: &str = "Put the files back as they were when this thread began? Every change shown below is undone, committed ones and new files too. Commits stay in git.";

/// Whether the Diff page has a change to act on. With none ("No changes since this thread began", or a folder that is no repository) there is
/// nothing to commit, push or put back: the buttons are not drawn (Commit then answered with git's "nothing to commit" and a failed push in the thread).
pub fn diff_changed(diff: &Value) -> bool { diff["files"].as_array().map_or(false, |f| !f.is_empty()) }

/// The Diff page's buttons as the harness's controls: (id, label, action, value); none while Diff is not on show.
pub fn diff_controls(diff: &Value) -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    if !diff.is_object() || !diff_changed(diff) { return vec![] }
    /* i18n-ok: the harness's control labels (ctl entries the e2e drivers press) */ let mut v = vec![("git-commit_push", "Commit & push", "git", "commit_push"), ("git-commit_push_pr", "PR", "git", "commit_push_pr"), ("revert-ask", "Revert", "revert", "ask")];
    /* i18n-ok: ctl labels */ if diff["reverting"] == true { v.push(("revert-confirm", "Revert", "revert", "confirm")); v.push(("revert-cancel", "Cancel", "revert-no", "")); }
    v
}

/// The room (px) the pane's controls take in a page's top right corner, left free by the 3D page's own floating tools.
pub const CORNER_ROOM: f32 = 150.0;
/// The pane's own controls, at the end of the page's toolbar row: where the page comes from (another machine's thread: "desktop: boards/x.kicad_pcb"),
/// Open in KiCad (while there is a board or schematic) and Close. They were the tab bar's; the rail picks the page now.
fn pane_actions(cx: &mut Context<Backplane>, light: bool, thread: &Value, target: Option<&super::attach::Target>, diff_on: bool, editing: bool) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let viewer = &thread["viewer"];
    let mut row = div().flex().flex_shrink_0().items_center().gap_1().max_w_full();
    if let Some((_, o, hover)) = super::badge::place_text(&thread["mark"], "") {
        let t = super::badge::from_machine(&thread["mark"], &source(viewer, diff_on)); let col = c("dim");
        row = row.child(div().id("viewer-from").min_w_0().max_w(px(200.)).flex().items_center().gap(px(3.)).overflow_hidden().whitespace_nowrap().text_size(px(11.)).text_color(col).opacity(o)
            .tooltip(crate::keys::tip(&hover, "", light)).child(crate::art::icon("link", 11., col)).child(div().overflow_hidden().text_ellipsis().child(t)));
    }
    // Edit / View: the real KiCad in this pane (on the hub's own machine), or back to Backplane's viewer
    // a Gerber set is no board to edit: no Edit, no Open in KiCad (the embedded GerbView comes with bp.13)
    let gerbers = viewer["gerbers"] == true;
    if has_design(viewer) && !gerbers && !diff_on && crate::viewers::kicad_pane::local(target.map(|t| t.addr.as_str()), thread) {
        let c2 = c(if editing { "accent" } else { "line" });
        /* i18n-ok: ctl labels and ui: ids are what drivers press */ let edit_div = crate::ui::with_rect(div().id("kicad-edit"), move || if editing { "ui:View".to_string() } else { "ui:Edit".to_string() });
        row = row.child(tall(edit_div.px_2().border_1().border_color(c2).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel"))).text_size(px(12.))
            .tooltip(crate::keys::tip(&if editing { trs("Back to Backplane's viewer (KiCad stays open)") } else { trs("Edit in the real KiCad, here in the pane") }, "", light))
            .child(if editing { trs("View") } else { trs("Edit") })
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| { if editing { this.kicad_view(cx); } else { this.kicad_edit(cx); } }))).into_any_element());
    }
    if has_design(viewer) && !gerbers && !editing { row = row.child(btn(cx, light, "kicad-open".into(), kicad_open_label(target.map(|t| t.addr.as_str()), thread["mark"]["name"].as_str()), false, kicad_open(viewer))); }
    row.child(div().id("viewer-close").size(px(super::head::BTN_H)).flex().items_center().justify_center().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel"))).tooltip(crate::keys::tip(&trs("Close the viewer"), "", light))
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
/// The 3D page before its first picture: the model's own ground (the viewer background setting, raster::Backdrop) with what it waits for in its middle, so the picture
/// replaces it in place instead of a line of text on the panel's ground.
fn model_loading(light: bool, say: &str) -> AnyElement {
    let b = crate::viewers::raster::Backdrop::current(); let hex = |c: [u8; 3]| rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32);
    div().size_full().flex_1().min_h_0().bg(gpui::linear_gradient(180., gpui::linear_color_stop(hex(b.top), 0.), gpui::linear_color_stop(hex(b.bottom), 1.))).flex().items_center().justify_center().text_color(theme::color(light, "faint")).child(say.to_string()).into_any_element()
}
/// A page that has nothing to show yet: what it says, and the pane's controls on the same row.
fn status_row(light: bool, say: &str, actions: Option<AnyElement>) -> AnyElement {
    // with the pane's controls it is the page's bar (on the header's line, as the page's own bar will be); without, a plain row
    let row = if actions.is_some() { bar(light) } else { div().flex().items_center().gap_2().px_3().py_2() };
    row.child(div().flex_1().min_w_0().text_color(theme::color(light, "dim")).child(say.to_string())).children(actions).into_any_element()
}

fn diff_page(cx: &mut Context<Backplane>, light: bool, diff: &Value, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().flex().flex_col().text_size(px(12.));
    col = col.child(bar_wrap(light).child(div().flex_1().min_w(px(180.)).text_color(c("dim")).child(trs(&string(diff, "summary"))))
        // the button says all it does: it commits and pushes (`commit_push`), as the palette's "Commit & push" and its tooltip say
        .when(diff_changed(diff), |d| d.children([(trs("Commit & push"), "commit_push"), ("PR".to_string(), "commit_push_pr")].into_iter().map(|(l, v)| btn(cx, light, format!("git-{v}"), l, false, Command::Action("git".into(), v.into()))).collect::<Vec<_>>())
            .child(btn(cx, light, "revert-ask".into(), trs("Revert"), diff["reverting"] == true, Command::Action("revert".into(), "ask".into()))))
        .child(actions));
    // Revert asks first (Ui.revert): the diff stays on show while the person decides what to discard
    if diff["reverting"] == true {
        col = col.child(div().flex().flex_col().gap_2().px_3().py_2().bg(c("raise")).border_y_1().border_color(c("bad"))
            .child(div().text_color(c("text")).child(trs(REVERT_ASK)))
            .child(div().flex().gap_2().child(btn(cx, light, "revert-confirm".into(), trs("Revert"), true, Command::Action("revert".into(), "confirm".into())))
                .child(btn(cx, light, "revert-cancel".into(), trs("Cancel"), false, Command::Action("revert-no".into(), String::new())))));
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
    if budget == 0 { col = col.child(div().px_3().py_1().text_color(c("dim")).child(trs("… the rest of the diff is not shown"))); }
    col.into_any_element()
}

/// One plot canvas: the frame it is given (or what the hub says instead) and, when `layers` is on, the hub's layer list beside it.
/// The viewer's boards (`viewer.boards`: label, path from the project's folder, on show).
/// What the pane shows, for its header beside the machine it comes from (`badge::from_machine`): the board on show, a file opened from Files,
/// the folder in Files, the Mech part, else the page's name.
pub fn source(viewer: &Value, diff_on: bool) -> String {
    if diff_on { return trs("Diff") }
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
    named.unwrap_or_else(|| tabs(viewer).into_iter().find(|t| t.on).map_or_else(|| open.to_string(), |t| trs(&t.label)))
}
pub fn boards(viewer: &Value) -> Vec<(String, String, bool)> {
    viewer["boards"].as_array().into_iter().flatten().map(|b| (string(b, "label"), string(b, "value"), b["on"].as_bool().unwrap_or(false))).collect()
}
/// A page whose plot has not come yet, laid out as it will be (its toolbar, the Boards/Layers/Fit row, the drawing's background) with what it waits for in
/// the middle: the plot then fills it in place, with no row arriving after it and nothing moving down.
fn plot_loading(cx: &mut Context<Backplane>, light: bool, slot: usize, viewer: &Value, pv: &mut super::plotview::PlotView, layers: bool, actions: Option<AnyElement>, say: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut col = div().size_full().flex().flex_col();
    // (as plot_pane lays it out: one bar, Boards/Layers/Fit then the pane's controls; the history bar goes under it, histbar::under)
    let ghost = |label: &str| tall(div().px_2().text_size(px(12.)).border_1().border_color(c("line")).text_color(c("faint")).child(trs(label)));
    let boards = if layers && slot == 0 { crate::viewers::boards::picker(cx, light, viewer, &pv.boards) } else { None };
    if let Some(a) = actions { col = col.child(bar(light).children(boards).when(layers, |d| d.child(ghost("Layers")).child(ghost("Fit"))).child(div().flex_1()).child(a)); }
    col.child(div().flex_1().min_h_0().bg(c("view")).flex().items_center().justify_center().text_color(c("faint")).child(say.to_string())).into_any_element()
}
fn plot_pane(cx: &mut Context<Backplane>, light: bool, slot: usize, open: &str, viewer: &Value, frame: Option<&super::plot::Frame>, pv: &mut super::plotview::PlotView, layers: bool, sheets: &[super::plot::Sheet], refs: &[(String, Vec<String>)], actions: Option<AnyElement>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let say = if open == "board" { trs("Loading the board…") } else { trs("Loading the schematic…") }; let say = say.as_str();
    // the design history bar stays under a board or schematic while a version loads or cannot be shown (screens::histbar)
    let hb = layers && matches!(open, "board" | "schematic"); let vkey = string(viewer, "key");
    let Some(frame) = frame else { let el = plot_loading(cx, light, slot, viewer, pv, layers, actions, say); return super::histbar::under(cx, light, pv, &vkey, hb, el) };
    // the frame must be the source this viewer asks for: another project's board stays off the screen until its own plot arrives
    let mut want = string(viewer, "key");
    // a comparison on show is its own source: `diff\n<old>\n<live>` after the key; another page puts it away
    if let Some(cmp) = pv.compare.clone() { if cmp.base == want { want = format!("{want}{}", super::histbar::shown(&cmp)); } else { pv.compare = None; } }
    // (another version of the same file keeps the last one on the canvas until it comes, as 0.14 did: stepping does not blink to "Loading")
    if !want.is_empty() && !frame.key.is_empty() && frame.key != want && !super::histbar::same_file(&vkey, &frame.key, &want) { let el = plot_loading(cx, light, slot, viewer, pv, layers, actions, say); return super::histbar::under(cx, light, pv, &vkey, hb, el) }
    if !frame.none.is_empty() { return super::histbar::under(cx, light, pv, &vkey, hb, status_row(light, &trs(&super::histbar::none_words(&frame.none)), actions)); }
    if pv.focus.is_none() { pv.focus = Some(cx.focus_handle()); }
    let (w, h) = pv.bounds.map_or((480., 600.), |b| (f32::from(b.size.width), f32::from(b.size.height)));
    let view = super::plotview::ensure_view(pv, frame, w, h);
    // the probe the hub keeps for this board, lit on this source too (after a page switch, a reopening, a new frame)
    super::plotview::probe_sync(cx, slot, pv, &frame.key, viewer["probe"].as_str().unwrap_or(""));
    let mut col = div().size_full().flex().flex_col();
    // the page's bar first (Boards, Layers, Fit, a schematic's Sheets, then the pane's controls at its end), what the page notes under it
    // (the design history bar is under the canvas, screens::histbar)
    let mut tools: Vec<AnyElement> = Vec::new();
    let note = string(viewer, "note");
    let note = (!note.is_empty()).then(|| div().px_3().py_1().text_size(px(11.)).text_color(c("dim")).child(trs(&note)));
    let mut sheets_list = None;
    // Layers: the menu lists the layers this plot has; the hub's masks decide what the canvas draws
    let mut present = std::collections::BTreeMap::new();
    for ch in &frame.chunks { present.entry(ch.layer).or_insert(ch.color); }
    let strip = crate::viewers::layers::strip(viewer, &present);
    pv.present = strip.present;
    // Sheets: a schematic of several sheets lists them as a tree (indented by depth); a press shows that sheet, the root first
    if sheets.len() > 1 && open == "schematic" && layers {
        let on_key = frame.key.clone();
        let head = tall(div().id("sheets-btn").px_2().text_size(px(12.)).cursor_pointer()).border_1().border_color(c("line")).bg(c("raise")).child(tf("gpui.c.sheets.count", &[("n", sheets.len().into())]))
            .on_click(cx.listener(|this, _, _, cx| { let pv = this.screens.pv(0); pv.sheets_open = !pv.sheets_open; cx.notify(); }));
        tools.push(head.into_any_element());
        if pv.sheets_open {
            let mut list = div().id("sheets-list").flex().flex_col().px_3().pb_1().text_size(px(12.));
            for (i, sh) in sheets.iter().enumerate() {
                let file = if i == 0 { String::new() } else { sh.file.clone() };
                let here = on_key.ends_with(&sh.file) && i != 0 || (i == 0 && on_key.ends_with('|'));
                list = list.child(div().id(SharedString::from(format!("sheet-{i}"))).pl(px(8. * sh.depth as f32)).py_px().cursor_pointer().text_color(if here { c("accent") } else { c("text") }).hover(|s| s.bg(c("raise"))).child(sh.name.clone())
                    .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-sheet".into(), file.clone())); this.screens.pv(0).sheets_open = false; cx.notify(); })));
            }
            sheets_list = Some(list);
        }
    }
    if layers {
        let fit = tall(div().id(SharedString::from(format!("plot-fit-{slot}"))).px_2().text_size(px(12.))).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).hover(|s| s.bg(c("panel"))).child(trs("Fit"))
            .on_click(cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.touched = false; pv.fit_wh = (0., 0.); cx.notify(); }));
        // the Boards menu leads the row (0.14 put it first in the viewer's bar), then Layers and Fit
        let boards = if slot == 0 { crate::viewers::boards::picker(cx, light, viewer, &pv.boards) } else { None };
        let lay = crate::viewers::layers::strip_el(cx, light, &strip, &pv.layers_key);
        tools.splice(0..0, boards.into_iter().chain([lay, fit.into_any_element()]));
        // a Gerber set: "vs board" (its copper, drills and outline against the board's: kicad/gerber/compare.rs), then where it is and how old (the plot's note)
        if viewer["gerbers"] == true {
            let on = viewer["gerbersVsBoard"] == true;
            tools.push(tall(crate::ui::with_rect(div().id("gerbers-vs-board"), || "ui:vs board".into()).px_2().text_size(px(12.))).cursor_pointer().border_1().border_color(c(if on { "accent" } else { "line" })).bg(c("raise")).hover(|s| s.bg(c("panel"))) // i18n-ok: a harness id
                .tooltip(crate::keys::tip(&trs("Mark where these Gerbers differ from the board: red on the board only, green in the Gerbers only"), "", light))
                .child(trs("vs board")).on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view".into(), if on { "gerbers".into() } else { "gerbers\tboard".into() })); cx.notify(); })).into_any_element());
            // the set in KiCad's own GerbView, where this runtime has it in process (embed-gerbview): Edit on the Board page, then the set
            let files: Vec<String> = viewer["gerberFiles"].as_array().map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect()).unwrap_or_default();
            if !files.is_empty() && crate::viewers::kicad_pane::GERBVIEW.load(std::sync::atomic::Ordering::Relaxed) {
                tools.push(tall(crate::ui::with_rect(div().id("gerbers-gerbview"), || "ui:Open in GerbView".into()).px_2().text_size(px(12.))).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).hover(|s| s.bg(c("panel"))) // i18n-ok: a harness id
                    .tooltip(crate::keys::tip(&trs("Open these Gerbers in KiCad's GerbView, here in the pane"), "", light))
                    .child(trs("Open in GerbView")).on_click(cx.listener(move |this, _, _, cx| this.open_in_gerbview(files.clone(), cx))).into_any_element());
            }
            // the marks told apart without colour too: the board's solid, the Gerbers' striped
            if on { tools.push(div().id("gerbers-legend").flex_shrink_0().flex().items_center().gap_1().text_size(px(11.)).text_color(c("dim"))
                .child(div().size(px(9.)).bg(rgb(0xe5484d))).child(trs("board only"))
                .child(div().ml_1().w(px(9.)).h(px(9.)).flex().flex_col().justify_between().children((0..3).map(|_| div().h(px(2.)).bg(rgb(0x30a46c))))).child(trs("Gerbers only")).into_any_element()); }
            if !frame.note.is_empty() { tools.push(div().id("gerbers-note").min_w_0().flex_shrink().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(px(11.)).text_color(c(if frame.warn { "warn" } else { "dim" })).child(frame.note.clone()).into_any_element()); }
        }
    }
    match actions {
        Some(a) => col = col.child(bar(light).children(tools).child(div().flex_1()).child(a)),
        None if !tools.is_empty() => col = col.child(div().flex().items_center().gap_2().px_3().py_1().children(tools)),
        None => {}
    }
    col = col.children(note).children(sheets_list);
    let mut area = div().relative().flex().flex_col().flex_1().min_h_0().child(super::plotview::canvas_el(cx, slot, pv.focus.clone(), light, frame, view, crate::viewers::layers::masks(viewer)));
    if layers { if let Some(p) = crate::viewers::layers::panel_el(cx, light, &strip, pv.layers_open, pv.layers_cap.unwrap_or(0.), &pv.layers_scroll) { area = area.child(p); } }
    // the card of the piece last tapped (Insp.card): what it is, then key = value rows; a tap on nothing, or Close, puts it away
    // a probed part placed on another sheet of the design: where it is, and a press to open that sheet (0.14's probe-away)
    if open == "schematic" {
        let r = viewer["probe"].as_str().and_then(super::plot::card).or_else(|| pv.card.clone()).and_then(|c| c.rows.iter().find(|x| x.0 == "ref" || x.0 == "symbol").map(|x| x.1.clone())).unwrap_or_default();
        if let Some(i) = super::plot::sheet_of(sheets, refs, &frame.key, &r) {
            let (say, file) = (if i == 0 { tf("gpui.c.probe.on.top", &[("ref", r.as_str().into())]) } else { tf("gpui.c.probe.on.sheet", &[("ref", r.as_str().into()), ("sheet", sheets[i].name.as_str().into())]) }, if i == 0 { String::new() } else { sheets[i].file.clone() });
            area = area.child(div().id(SharedString::from(format!("probe-away-{slot}"))).absolute().top_2().left_2().flex().items_center().gap_2().px_2().py_1().text_size(px(12.)).bg(c("panel")).border_1().border_color(c("accent")).occlude()
                .child(say)
                .child(div().id(SharedString::from(format!("probe-away-open-{slot}"))).px_2().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel"))).child(trs("Open it"))
                    .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-sheet".into(), file.clone())); cx.notify(); }))));
        }
    }
    // (Split's panes, slots 1 and 2, name it in their bar instead: a card there covered the drawing beside it)
    if let Some(card) = pv.card.clone().filter(|_| slot == 0) {
        // compact (0.14's probe chip spirit): the part or net in the head, then the few rows that say what it is; every row in the tooltip
        let words = probe_words(&card);
        let full = std::iter::once(trs(&card.title)).chain(card.rows.iter().map(|(k, v)| format!("{}: {v}", trs(k)))).collect::<Vec<_>>().join("\n");
        let mut box_ = div().id(SharedString::from(format!("pick-card-{slot}"))).absolute().left_2().bottom_2().max_w(px(280.)).flex().flex_col().gap(px(2.)).px_2().py_1().text_size(px(11.)).border_1().border_color(c("accent")).bg(c("panel")).occlude()
            .tooltip(crate::keys::tip(&full, "", light))
            .child(div().flex().items_center().gap_2().child(div().id(SharedString::from(format!("pick-name-{slot}"))).flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(px(12.)).font_weight(FontWeight::BOLD).cursor_pointer().child(words.join(" · ")).tooltip(crate::keys::tip(&trs("Bring it into view"), "", light))
                    .on_click(cx.listener(move |this, _, _, cx| { super::plotview::bring_into_view(this, slot); cx.notify(); })))
                /* i18n-ok: Esc is a key name */ .child(div().id(SharedString::from(format!("pick-close-{slot}"))).flex_none().p(px(2.)).cursor_pointer().hover(|s| s.bg(c("raise"))).tooltip(crate::keys::tip(&trs("Clear"), "Esc", light)).child(crate::art::icon("x", 12., c("dim")))
                    .on_click(cx.listener(move |this, _, _, cx| { let pv = this.screens.pv(slot); pv.card = None; pv.picked = None; pv.lit.clear(); super::plotview::keep_probe(this, None); cx.notify(); }))));
        for (k, v) in card_rows(&card) { box_ = box_.child(div().flex().gap_2().child(div().w(px(58.)).flex_none().text_color(c("dim")).child(trs(&k))).child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(v))); }
        area = area.child(box_);
    }
    col = col.child(area);
    if hb { col = col.child(super::histbar::bar_el(cx, light, pv, &vkey)); }
    col.into_any_element()
}

/// Board or schematic: the plot the hub sent for this file.
fn plot_page(cx: &mut Context<Backplane>, light: bool, open: &str, viewer: &Value, plot: &super::plot::Store, pv: &mut super::plotview::PlotView, actions: AnyElement) -> AnyElement {
    plot_pane(cx, light, 0, open, viewer, plot.frame.as_ref(), pv, true, &plot.sheets, &plot.sheet_refs, Some(actions))
}

/// Split's panes by the words 0.14 kept (split.hide): "sch" the schematic, "pcb" the board, "3d" its 3D model.
/* i18n-ok: drawn through trs (split_head, split_bar) */ pub const SPLIT_PANES: [(&str, &str); 3] = [("sch", "Schematic"), ("pcb", "Board"), ("3d", "3D")];
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
    if save { this.local.save(); } else { save_soon(cx); }
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
fn split_head(cx: &mut Context<Backplane>, light: bool, pane: &'static str, name: &'static str, more: Option<AnyElement>) -> Div {
    let c = |n: &str| theme::color(light, n);
    div().flex_none().h(px(26.)).flex().items_center().gap_2().pl_2().pr_1().bg(c("panel")).border_b_1().border_color(c("line"))
        .child(div().flex_none().text_size(px(10.)).text_color(c("dim")).child(trs(name).to_uppercase())).children(more).child(div().flex_1())
        .child(div().id(SharedString::from(format!("split-hide-{pane}"))).p(px(3.)).cursor_pointer().hover(|s| s.bg(c("raise"))).tooltip(crate::keys::tip(&tf("gpui.c.split.hide", &[("name", trs(name).into())]), "", light))
            .child(crate::art::icon("x", 13., c("dim"))).on_click(cx.listener(move |this, _, _, cx| split_set(this, |p| p.hide = split_flip(&p.hide, pane), true, cx))))
}
/// The schematic pane's Sheets menu: the sheet on show, and the design's sheets (indented by depth) under it while open; a press shows that sheet in the pane.
fn split_sheets(cx: &mut Context<Backplane>, light: bool, sheets: &[super::plot::Sheet], key: &str, open: bool) -> Option<AnyElement> {
    if sheets.len() < 2 { return None }
    let c = |n: &str| theme::color(light, n);
    let here = |i: usize, sh: &super::plot::Sheet| if i == 0 { key.ends_with('|') || !sheets.iter().skip(1).any(|x| key.ends_with(&x.file)) } else { key.ends_with(&sh.file) };
    let cur = sheets.iter().enumerate().find(|(i, sh)| here(*i, sh)).map(|(i, sh)| if i == 0 { trs("Top sheet") } else { sh.name.clone() }).unwrap_or_else(|| trs("Top sheet"));
    let btn = div().id("split-sheets").flex().items_center().gap_1().px_1().text_size(px(11.)).cursor_pointer().border_1().border_color(c(if open { "accent" } else { "line" })).hover(|s| s.bg(c("raise")))
        .child(div().max_w(px(220.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(cur)).child(crate::art::icon("chevron-down", 11., c("dim")))
        .on_click(cx.listener(|this, _, _, cx| { let pv = this.screens.pv(1); pv.sheets_open = !pv.sheets_open; cx.notify(); }));
    let mut wrap = div().relative().child(btn);
    if open {
        let mut list = div().id("split-sheets-list").mt(px(2.)).min_w(px(220.)).max_h(px(420.)).overflow_y_scroll().flex().flex_col().p_1().text_size(px(12.)).bg(c("panel")).border_1().border_color(c("line")).shadow_lg().occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| { this.screens.pv(1).sheets_open = false; cx.notify(); }));
        for (i, sh) in sheets.iter().enumerate() {
            let file = if i == 0 { String::new() } else { sh.file.clone() };
            let on = here(i, sh);
            list = list.child(div().id(SharedString::from(format!("split-sheet-{i}"))).pl(px(6. + 10. * sh.depth as f32)).pr_2().py(px(3.)).cursor_pointer().text_color(c(if on { "accent" } else { "text" })).hover(|s| s.bg(c("raise")))
                .child(if i == 0 { trs("Top sheet") } else { sh.name.clone() })
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("view-sheet".into(), file.clone())); this.screens.pv(1).sheets_open = false; cx.notify(); })));
        }
        wrap = wrap.child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(list)).with_priority(3));
    }
    Some(wrap.into_any_element())
}
/// The bar's pane toggles (lit while shown) and the probed piece as a chip (0.14's combo-picks and probe-chip): a tap on a part or net in any pane names it here
/// instead of a card over the drawing; × or Escape clears it in every pane.
fn split_bar(cx: &mut Context<Backplane>, light: bool, hide: &str, probed: Option<&super::plot::Card>, boards: Option<AnyElement>, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let mut picks = div().flex().min_w_0().overflow_hidden().gap_1(); // (in a narrow pane the toggles clip, the pane's own controls and its × stay)
    for (pane, name) in SPLIT_PANES.iter() {
        let (pane, name, on) = (*pane, *name, split_shows(hide, pane));
        // (0.14's combo-pick: outlined in the accent while shown, dim while put away)
        let d = div().id(SharedString::from(format!("split-pick-{pane}"))).px_2().py(px(2.)).text_size(px(12.)).cursor_pointer().border_1().hover(|s| s.bg(c("raise")));
        let d = if on { d.border_color(c("accent")).text_color(c("text")) } else { d.border_color(c("line")).text_color(c("dim")) };
        picks = picks.child(d.tooltip(crate::keys::tip(&tf(if on { "gpui.c.split.hide" } else { "gpui.c.split.show" }, &[("name", trs(name).into())]), "", light)).child(trs(name))
            .on_click(cx.listener(move |this, _, _, cx| split_set(this, |p| p.hide = split_flip(&p.hide, pane), true, cx))));
    }
    // the bar: (the Boards menu,) the pane toggles, the probe, then the pane's own controls
    // (as tall as the thread's head beside it, so their lines meet)
    // (the page's bar: on the header's line, as every viewer page's)
    let mut row = div().id("split-bar").flex_none().h(px(super::head::ROW_H)).flex().items_center().gap_2().px_2().border_b_1().border_color(c("rule")).children(boards).child(div().flex_1()).child(picks);
    if let Some(card) = probed {
        let words = probe_words(card);
        let full = std::iter::once(trs(&card.title)).chain(card.rows.iter().map(|(k, v)| format!("{}: {v}", trs(k)))).collect::<Vec<_>>().join("\n");
        row = row.child(div().id("split-probe").min_w_0().max_w(px(240.)).flex_shrink().flex().items_center().gap_1().pl_2().pr(px(2.)).py_px().border_1().border_color(c("accent")).bg(c("raise")).text_size(px(12.))
            .tooltip(crate::keys::tip(&full, "", light))
            .child(div().id("split-probe-name").min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().cursor_pointer().child(words.join(" · ")).tooltip(crate::keys::tip(&trs("Bring it into view"), "", light))
                .on_click(cx.listener(|this, _, _, cx| { for i in [1, 2] { super::plotview::bring_into_view(this, i); } cx.notify(); })))
            /* i18n-ok: Esc is a key name */ .child(div().id("split-probe-clear").flex_none().p(px(2.)).cursor_pointer().hover(|s| s.bg(c("panel"))).tooltip(crate::keys::tip(&trs("Clear"), "Esc", light)).child(crate::art::icon("x", 12., c("dim")))
                .on_click(cx.listener(|this, _, _, cx| { for i in 0..3 { let pv = this.screens.pv(i); pv.picked = None; pv.card = None; pv.lit.clear(); } super::plotview::keep_probe(this, None); super::plotview::report_state(this); cx.notify(); }))));
    }
    row.child(actions).into_any_element()
}
/// The card's rows beyond its head: what kind of piece (the info's title) and its footprint, layer and type, at most four (the ref, value and net are the head).
pub fn card_rows(card: &super::plot::Card) -> Vec<(String, String)> {
    let get = |k: &str| card.rows.iter().find(|r| r.0 == k).map(|r| r.1.trim().to_string()).filter(|v| !v.is_empty());
    let mut v = vec![];
    if card.rows.iter().any(|r| r.0 == "ref" && !r.1.is_empty()) { v.push(("piece".to_string(), card.title.clone())); }
    for k in ["footprint", "layer", "type", "shape"] { if let Some(x) = get(k) { v.push((k.to_string(), x)); } }
    v.truncate(4); v
}
/// What the probe chip says: the part's reference (else what was tapped), its value and its net, as 0.14's Probe.label named a probe by its ref or net.
pub fn probe_words(card: &super::plot::Card) -> Vec<String> {
    let get = |k: &str| card.rows.iter().find(|r| r.0 == k).map(|r| r.1.trim().to_string()).filter(|v| !v.is_empty());
    let mut words = vec![get("ref").or_else(|| get("symbol").map(|r| format!("{r} {}", trs(&card.title)))).unwrap_or_else(|| trs(&card.title))];
    for k in ["value", "net"] { if let Some(v) = get(k) { if !words.contains(&v) { words.push(v) } } }
    words
}
/// Split (0.14's combined view): the schematic beside the board and its 3D model (stacked the other way on a pane taller than wide), each under a head that
/// names it and puts it away, the bar's toggles bring each back, and the dividers between them resize them (kept on this device). Every pane is a page of
/// `viewer.panes` (key, camera), all watched at once; a tap in the schematic or the board lights the part or net in both.
fn split_page(cx: &mut Context<Backplane>, light: bool, viewer: &Value, plot: &super::plot::Store, pvs: &mut [super::plotview::PlotView; 2], solid: Option<&Entity<crate::viewers::solid::SolidPage>>, have: &str, boards: Option<AnyElement>, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let panes: Vec<&Value> = viewer["panes"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if panes.len() < 3 { return status_row(light, &trs("Opening…"), Some(actions)) }
    let prefs = split_prefs();
    let (s_on, b_on, t_on) = (split_shows(&prefs.hide, "sch"), split_shows(&prefs.hide, "pcb"), split_shows(&prefs.hide, "3d"));
    let probed = pvs.iter().find_map(|p| p.card.clone());
    let bar = split_bar(cx, light, &prefs.hide, probed.as_ref(), boards, actions);
    // landscape (the pane wider than tall): the schematic on the left, the board over the 3D on the right; portrait: the schematic on top, the board beside the 3D
    let wide = split_box(0).map_or(true, |b| b.size.width >= b.size.height);
    let [sch_pv, brd_pv] = pvs;
    let pane = |head: Div, body: AnyElement| div().flex().flex_col().min_w_0().min_h_0().child(head).child(div().relative().flex_1().min_h_0().min_w_0().child(body));
    // the schematic pane's sheets (0.14's combo-sheets): the sheet on show in its head, the hierarchy in a menu under it
    let sheets_el = split_sheets(cx, light, &plot.sheets, &string(panes[0], "key"), sch_pv.sheets_open);
    let sch = s_on.then(|| { /* i18n-ok: split_head draws the name through trs */ let h = split_head(cx, light, "sch", "Schematic", sheets_el); let b = plot_pane(cx, light, 1, "schematic", panes[0], plot.frame_for(&string(panes[0], "key")), sch_pv, false, &plot.sheets, &plot.sheet_refs, None); pane(h, b) });
    let brd = b_on.then(|| { /* i18n-ok: split_head draws the name through trs */ let h = split_head(cx, light, "pcb", "Board", None); let b = plot_pane(cx, light, 2, "board", panes[1], plot.frame_for(&string(panes[1], "key")), brd_pv, false, &[], &[], None); pane(h, b) });
    let d3 = t_on.then(|| {
        let want3 = string(panes[2], "key");
        if let Some(s) = solid { if s.read(cx).reserve != 0. { s.update(cx, |p, cx| { p.reserve = 0.; cx.notify(); }); } }
        let model: AnyElement = match solid { Some(s) if solid_ready(&want3, have) && !have.is_empty() => div().size_full().child(s.clone()).into_any_element(), _ => model_loading(light, &trs("Loading the model…")) };
        let h = split_head(cx, light, "3d", "3D", None); pane(h, model)
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
    let pages = div().flex_1().min_w_0().flex().flex_wrap().gap_2().gap_y_1().children(arr(review, "tabs").enumerate().map(|(i, t)| btn(cx, light, format!("rv-{i}"), trs(&string(t, "label")), t["on"].as_bool().unwrap_or(false), Command::Action("rv-page".into(), string(t, "page")))).collect::<Vec<_>>());
    // the tabs are the page's bar (the pane's controls at the end of its first line); the page under it
    let bar = bar_wrap(light).flex_nowrap().items_start().child(pages).child(actions);
    let title = string(review, "title");
    let page = arr(review, "tabs").find(|t| t["on"] == true).map(|t| string(t, "page")).unwrap_or_default();
    if !title.is_empty() { col = col.child(div().flex().items_center().gap_2().child(div().flex_1().font_weight(FontWeight::BOLD).text_size(px(15.)).child(trs(&title)))
        .children(matches!(page.as_str(), "bom" | "checks").then(|| btn(cx, light, "rv-run".into(), trs("Refresh"), false, Command::Action("rv-run".into(), String::new()))))); }
    // the Checks page: why the gate fails, and the checklist's boxes (a press ticks one: rv-tick <index>)
    if review["checks"].is_object() {
        let line = string(&review["checks"], "line"); let gate = string(&review["checks"], "gate");
        if !line.is_empty() { col = col.child(div().text_color(c("dim")).child(trs(&line))); }
        if !gate.is_empty() { col = col.child(div().text_color(c("bad")).child(trs(&gate))); }
        for t in arr(&review["checks"], "tasks") {
            let (done, idx) = (t["done"] == true, t["index"].as_u64().unwrap_or(0));
            col = col.child(div().id(SharedString::from(format!("rv-task-{idx}"))).flex().gap_2().items_center().cursor_pointer().hover(|s| s.bg(c("raise")))
                .child(div().w(px(24.)).font(crate::art::mono_font()).text_color(c(if done { "ok" } else { "dim" })).child(if done { "[x]" } else { "[ ]" })).child(div().flex_1().child(string(t, "text")))
                .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action("rv-tick".into(), idx.to_string())); cx.notify(); })));
        }
    }
    let blocks: Vec<Value> = review["blocks"].as_array().cloned().unwrap_or_default();
    if blocks.is_empty() { col = col.child(div().text_color(c("dim")).child(if review["busy"] == true { trs("Working…") } else { trs(&string(review, "note")) })); }
    else { let parsed = md::parse(&blocks); for el in timeline::blocks(cx, reg, light, &parsed, "review") { col = col.child(el); } }
    div().flex().flex_col().child(bar).child(col).into_any_element()
}

thread_local! { static BOARDS_ASKED: std::cell::RefCell<std::collections::HashSet<String>> = std::cell::RefCell::new(Default::default()); }
/// `view-boards` for this thread, once in this window's life (a project with one board never has a menu, so it is not asked again).
fn ask_boards_once(cx: &mut Context<Backplane>, thread: &str) {
    if thread.is_empty() || !BOARDS_ASKED.with(|a| a.borrow_mut().insert(thread.to_string())) { return }
    cx.spawn(async move |this, cx| { let _ = this.update(cx, |app, _| { let _ = app.tx.try_send(Command::Action("view-boards".into(), String::new())); }); }).detach();
}
/// The page the pane shows and whether it is a press the hub has not answered yet: a rail press shuts the pane, or opens it laid out as that page, at once;
/// None while nothing is open.
fn pane_page(thread: &Value) -> Option<(String, bool)> {
    let viewer = &thread["viewer"]; let diff_on = thread["diff"].is_object();
    let open = viewer["open"].as_str().unwrap_or("");
    let press = pressed(if diff_on { "diff" } else { open });
    if press.as_deref() == Some("") && !diff_on { return None }
    if hub_open(thread) { return Some((open.to_string(), false)) }
    press.filter(|k| !k.is_empty()).map(|k| (k, true))
}
/// The panel (None while nothing is open). 520 px wide, between the thread and the rail.
pub fn page(cx: &mut Context<Backplane>, reg: &Reg, light: bool, files_ui: &super::filesui::FilesUi, images: &super::images::Images, target: Option<&super::attach::Target>, thread: &Value, plot: &super::plot::Store, pv: &mut super::plotview::PlotView, split_pv: &mut [super::plotview::PlotView; 2], solid: Option<&Entity<crate::viewers::solid::SolidPage>>, browser: Option<&Entity<crate::viewers::browser::BrowserPage>>, editor: Option<&Entity<crate::viewers::kicad_pane::KicadPane>>) -> AnyElement {
    let Some((open, early)) = pane_page(thread) else { return Empty.into_any_element() };
    let open = open.as_str();
    // a drawing page shown without the project's boards (the window reconnected, or the page was restored as it was left): read them once
    if matches!(open, "board" | "schematic" | "3d" | "split") && !early && !thread["viewer"]["boards"].is_array() { ask_boards_once(cx, &string(thread, "id")); }
    let viewer = &thread["viewer"];
    let diff_on = thread["diff"].is_object();
    // no tab bar: the rail picks the page. The pane's own controls (where it comes from, Open in KiCad, Close) end the page's toolbar row,
    // or float in the top right corner of a page drawn to its edges (3D, Split, Browser)
    // the Gerbers page is the Board page drawing a Gerber set (viewer.gerbers): Backplane's own, never KiCad's, Edit on or not (lane K's
    // rerun 9: the rail lit Gerbers while the pane kept KiCad's PCB editor and never asked for the set's plot)
    let editing = editor.is_some() && matches!(open, "board" | "schematic" | "3d") && viewer["gerbers"] != true && !diff_on;
    let acts = |cx: &mut Context<Backplane>| pane_actions(cx, light, thread, target, diff_on, editing);
    let body: AnyElement = if editing {
        // the real KiCad, its rail page's editor over the pane (viewers/kicad_pane.rs); the pane's controls in its own row above it, never over KiCad's toolbar
        let pressed_open = pressed(open).filter(|k| matches!(k.as_str(), "board" | "schematic" | "3d")); // i18n-ok: rail page kinds
        let e = editor.unwrap().clone();
        if e.read(cx).needs_sync_open(open, pressed_open.as_deref()) {
            let ho = open.to_string();
            e.update(cx, |p, cx| p.sync_hub_open(&ho, pressed_open.as_deref(), cx));
        }
        if e.read(cx).light != light { e.update(cx, |p, cx| p.set_light(light, cx)); }
        let a = acts(cx);
        let (popped, full) = (KICAD_POPPED.load(std::sync::atomic::Ordering::Relaxed), KICAD_FULL.load(std::sync::atomic::Ordering::Relaxed));
        let strip = crate::viewers::kicad_pane::strip(cx, light, &e, false, full, |this: &mut Backplane, a, _, cx| this.kicad_strip_act(a, cx));
        let body = if popped {
            // the editor is in its own window: the pane says so and offers it back
            div().flex_1().min_h_0().flex().flex_col().items_center().justify_center().gap_2().text_color(theme::color(light, "faint")).child(trs("KiCad is open in its own window."))
                .child(div().id("kicad-popin").px_3().py_1().border_1().border_color(theme::color(light, "line")).bg(theme::color(light, "raise")).cursor_pointer().child(trs("Pop back in"))
                    .on_click(cx.listener(|this, _, _, cx| { let h = this.screens.kicad_window; this.kicad_popin(cx); if let Some(h) = h { let _ = h.update(cx, |_, w, _| w.remove_window()); } })))
                .into_any_element()
        } else { div().flex_1().min_h_0().child(e).into_any_element() };
        div().size_full().flex().flex_col().child(bar(light).justify_end().child(a)).when(!popped, |d| d.child(strip)).child(body).into_any_element()
    }
    else if early {
        let k = open;
        let a = acts(cx);
        match k { "board" | "schematic" => plot_loading(cx, light, 0, &Value::Null, pv, true, Some(a), &if k == "board" { trs("Loading the board…") } else { trs("Loading the schematic…") }),
            "3d" | "mech" | "split" => in_corner(model_loading(light, &trs("Loading…")), a), _ => status_row(light, &trs("Opening…"), Some(a)) }
    }
    else if diff_on { let a = acts(cx); diff_page(cx, light, &thread["diff"], a) }
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
            let model: AnyElement = if open == "mech" && key_empty { div().into_any_element() } else { match solid { Some(s) if ready => div().flex_1().min_h_0().child(s.clone()).into_any_element(), _ => model_loading(light, &trs("Loading the model…")) } };
            if open == "mech" { let a = acts(cx); div().size_full().flex().flex_col().child(mech_chrome(cx, light, viewer, key_empty, a)).child(model).into_any_element() }
            else if ready { let a0 = acts(cx); let a = with_boards(cx, light, viewer, pv, a0); in_corner(div().size_full().flex().flex_col().child(model).into_any_element(), a) }
            else { let a = acts(cx); in_corner(model_loading(light, &trs("Loading the model…")), a) }
        }
        else if open == "browser" { let a = acts(cx); match browser { Some(b) => in_corner(div().size_full().child(b.clone()).into_any_element(), a), None => status_row(light, &trs("The browser page needs a connected hub."), Some(a)) } }
        else { let a = acts(cx); status_row(light, &trs("Opening…"), Some(a)) };
    if matches!(open, "files" | "image" | "pdf" | "text") && !diff_on { div().id("viewer-body").size_full().child(body).into_any_element() } else { div().id("viewer-body").size_full().overflow_y_scroll().child(body).into_any_element() }
}
/// The pane around the page on show: its width, the left edge's drag and border; `page` (the page itself, `page()`, drawn as a part of the
/// window by ui.rs) fills the rest, so the pane's frame is known without building the page.
pub fn pane(cx: &mut Context<Backplane>, light: bool, thread: &Value, page: AnyElement) -> Option<AnyElement> {
    let (open, _) = pane_page(thread)?;
    let c = |n: &str| theme::color(light, n);
    let open = open.as_str();
    // a Split holds three pages at once, so it takes the room of a desktop: its pane is at least that wide, whatever this device chose for the others
    let width = if open == "split" { (pane_width() as f32).max(960.) } else { pane_width() as f32 };
    // the embedded editor's full size: the pane takes the room of the sidebar and the thread (ui.rs folds them for it)
    if KICAD_FULL.load(std::sync::atomic::Ordering::Relaxed) && matches!(open, "board" | "schematic" | "3d") {
        return Some(div().flex_1().min_w_0().h_full().relative().border_l_1().border_color(c("rule")).flex().flex_col().child(page).into_any_element())
    }
    // The pane gives way to the thread: it shrinks (down to its least) before the composer row is squeezed, so Attach and Send stay reachable
    Some(div().w(px(width)).max_w(relative(0.62)).min_w(px(PANE_LEAST.load(std::sync::atomic::Ordering::Relaxed) as f32)).h_full().flex_shrink().relative().border_l_1().border_color(c("rule")).flex().flex_col()
        .child(resize_handle(cx)).child(page).into_any_element())
}

/// The embedded editor this frame (set by ui.rs before the window is drawn, as the pane's least is): drawn at full size (only while one
/// is open), and drawn in a window of its own (the pane then says so).
pub static KICAD_FULL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static KICAD_POPPED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// The least the pane may shrink to on this frame (`fit`'s third answer, set by ui.rs before the window is drawn).
static PANE_LEAST: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(crate::local::VIEWER_PX.0);
pub fn set_least(px: f32) { PANE_LEAST.store(px.max(0.).round() as u32, std::sync::atomic::Ordering::Relaxed); }
/// The viewer's width when it was not dragged (`fit`'s fourth answer, set by ui.rs before the window is drawn).
static PANE_SHARE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(crate::local::VIEWER_PX_DEFAULT);
pub fn set_share(px: f32) { PANE_SHARE.store(px.max(0.).round() as u32, std::sync::atomic::Ordering::Relaxed); }
/// The rail's width and the folded sidebar's strip; the thread's least, the viewer's least and a fair viewer (0.14's core/panes.bend:
/// Panes.thread, Panes.least, Panes.fair).
pub const RAIL_W: f32 = 72.0;
pub const STRIP_W: f32 = 44.0;
pub const THREAD_LEAST: f32 = 360.0;
pub const VIEWER_LEAST: f32 = 320.0;
pub const PANE_FAIR: f32 = 480.0;
/// How a window `window_w` wide is shared, as 0.14 did (core/panes.bend), so the rail and the viewer's × never leave the window: the thread
/// keeps THREAD_LEAST and the viewer takes its share of the rest (11/21 of the room beside the sidebar) or its dragged width; opening the
/// viewer where the open sidebar would leave it less than a fair viewer folds the sidebar to its strip for as long as the window is that
/// narrow (`force`: the person unfolded it anyway; then the viewer gives way, down to nothing). Answers (the sidebar drawn open, the thread
/// column's least, the viewer pane's least, the viewer's undragged width).
pub fn fit(window_w: f32, side_w: f32, side_open: bool, force: bool, viewer: bool) -> (bool, f32, f32, f32) {
    let room = window_w - side_w - RAIL_W - THREAD_LEAST;
    let shown = side_open && (force || if viewer { room >= PANE_FAIR } else { room >= 0. });
    let side = if shown { side_w } else { STRIP_W };
    let avail = (window_w - side - RAIL_W).max(0.);
    let thread = THREAD_LEAST.min(avail);
    let least = if viewer { (avail - thread).clamp(0., VIEWER_LEAST) } else { 0. };
    (shown, thread, least, ((window_w - side).max(0.) * 11. / 21.).round())
}
impl Backplane {
    /// Fold or unfold the sidebar as it is shown (Ctrl+B, the fold chevron, the strip's chevron): folding clears `side_force`, unfolding a
    /// sidebar the window folded for want of room shows it anyway (until the window is wide enough again).
    pub(crate) fn side_flip(&mut self, cx: &mut Context<Self>) {
        if self.screens.side_shown { self.screens.side_force = false; if self.sidebar_open { self.sidebar_toggle(cx); } }
        else if !self.sidebar_open { self.screens.side_force = false; self.sidebar_toggle(cx); }
        else { self.screens.side_force = true; self.ui_epoch += 1; cx.notify(); }
    }
}
/// The pane's width: this device's choice (the left edge's drag), else the default; read once from the local file, then kept here.
static PANE_W: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);
pub fn pane_width() -> u32 {
    use std::sync::atomic::Ordering::Relaxed;
    if PANE_W.load(Relaxed) == u32::MAX { PANE_W.store(crate::local::Local::load().viewer_px, Relaxed); }
    match PANE_W.load(Relaxed) { 0 => PANE_SHARE.load(Relaxed), w => w }
}
/// The width a drag to window x `x` makes (the pane runs from there to the rail on the right), kept in range and leaving the thread at least
/// THREAD_LEAST wide beside the sidebar (0.14: Panes.room).
pub fn width_for(window_w: f32, x: f32, rail: f32, sidebar: f32) -> u32 {
    let most = ((window_w - rail - sidebar - THREAD_LEAST) as i64).max(crate::local::VIEWER_PX.0 as i64).min(crate::local::VIEWER_PX.1 as i64);
    ((window_w - rail - x).round() as i64).clamp(crate::local::VIEWER_PX.0 as i64, most) as u32
}
struct ResizeDrag;
struct Ghost;
impl Render for Ghost { fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div() } }
/// A width from the edge's drag: on screen at once; written to this device's file once the drag rests (300 ms), not on every move (a file write per
/// pointer move made the drag stutter).
static PANE_SAVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn set_pane(this: &mut Backplane, w: u32, cx: &mut Context<Backplane>) {
    use std::sync::atomic::Ordering::Relaxed;
    if PANE_W.load(Relaxed) == w && this.local.viewer_px == w { return }
    PANE_W.store(w, Relaxed);
    this.local.viewer_px = w; this.ui_epoch += 1; cx.notify();
    save_soon(cx);
}
/// This device's file written once a drag rests (300 ms), wherever the pointer is let go.
fn save_soon(cx: &mut Context<Backplane>) {
    use std::sync::atomic::Ordering::Relaxed;
    if !PANE_SAVE.swap(true, Relaxed) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(300)).await;
            PANE_SAVE.store(false, Relaxed);
            let _ = this.update(cx, |this, _| this.local.save());
        }).detach();
    }
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
    /// The pane's words in Japanese: whose machine KiCad opens on, a Split pane's toggle, a probed part on another sheet.
    #[test] fn the_panes_words_read_in_japanese() {
        crate::i18n::with_lang("ja", || {
            assert_eq!(kicad_open_label(Some("desktop.tail9e0cf2.ts.net:8451"), None), "desktopのKiCadで開く"); assert_eq!(kicad_open_label(None, None), "KiCadで開く");
            assert_eq!(crate::i18n::tf("gpui.c.split.hide", &[("name", crate::i18n::trs("Schematic").into())]), "回路図を隠す");
            assert_eq!(crate::i18n::tf("gpui.c.probe.on.sheet", &[("ref", "C39".into()), ("sheet", "power".into())]), "C39はシートpowerにあります");
            assert_eq!(crate::i18n::trs(REVERT_ASK).chars().next(), Some('フ'));
        });
    }
    #[test] fn a_narrow_window_keeps_the_rail_as_0_14_shared_the_panes() {
        let (side, rail) = (264., RAIL_W);
        for w in [700., 800., 900., 1100., 1176., 1280., 1600., 1920.] { for viewer in [false, true] { for force in [false, true] {
            let (shown, thread, pane, _) = fit(w, side, true, force, viewer);
            let used = if shown { side } else { STRIP_W } + rail + thread + pane;
            assert!(used <= w + 0.5, "{w} viewer={viewer} force={force}: {used} > {w}");
            assert_eq!(thread, THREAD_LEAST, "{w}: the thread keeps its least");
            if viewer && !force { assert!(pane > 0., "{w}: the viewer keeps some room"); }
        } } }
        assert_eq!(fit(1920., side, true, false, true), (true, THREAD_LEAST, VIEWER_LEAST, 867.), "wide: sidebar open, the viewer's share");
        assert_eq!(fit(1280., side, true, false, true).0, true, "a fair viewer still fits beside the open sidebar");
        assert_eq!(fit(1100., side, true, false, true), (false, THREAD_LEAST, VIEWER_LEAST, 553.), "less than a fair viewer: the sidebar folds to its strip");
        assert_eq!(fit(1100., side, true, false, false).0, true, "no viewer: nothing folds");
        assert_eq!(fit(700., side, true, false, true), (false, THREAD_LEAST, 224., 344.), "then the viewer gives way");
        assert_eq!(fit(900., side, true, true, true), (true, THREAD_LEAST, 204., 333.), "unfolded anyway: the viewer gives way, the rail stays");
        assert_eq!(fit(900., side, false, false, true).0, false, "folded stays folded");
        assert_eq!(width_for(1600., 400., 72., 264.), 904, "a drag may leave the thread its least (0.14's room), no more");
    }

    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    #[test] fn the_pane_is_up_at_the_press_and_down_at_the_press_that_shuts_it() {
        let shut = j(r#"{"viewer":{"open":""}}"#); let board = j(r#"{"viewer":{"open":"board","choices":[{"label":"Board","value":"board","on":true}]}}"#);
        assert!(!is_open(&shut));
        let _ = rail_command(&shut["viewer"], false, None, "board");
        assert!(is_open(&shut), "pressed: up before the hub answers");
        assert!(is_open(&board), "answered");
        let _ = rail_command(&board["viewer"], false, None, "board");
        assert!(!is_open(&board), "a press that shuts it: down before the hub answers");
        assert!(!is_open(&shut));
    }
    #[test] fn a_press_shows_its_page_until_the_hub_has_answered() {
        assert!(matches!(rail_command(&j(r#"{"open":""}"#), false, None, "board"), Command::Action(a, k) if a == "view" && k == "board"));
        assert_eq!(pressed(""), Some("board".to_string()), "pressed, not answered: the pane opens as Board at once");
        assert_eq!(pressed("board"), None, "the hub shows it: the press is done");
        let _ = rail_command(&j(r#"{"open":"board","choices":[{"label":"Board","value":"board","on":true}]}"#), false, None, "board");
        assert_eq!(pressed("board"), Some(String::new()), "a press on the lit kind shuts at once");
        assert_eq!(pressed(""), None);
        let _ = rail_command(&j(r#"{"open":""}"#), false, None, "files");
        assert_eq!(pressed("image"), None, "a file opened from Files answers a Files press");
    }
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
    #[test] fn a_part_on_another_sheet_is_found_by_its_reference() {
        let sheets = super::super::plot::sheet_rows("0\t0\tRoot\t/p/top.kicad_sch\n1\t1\tPower\t/p/power.kicad_sch");
        let refs = super::super::plot::sheet_refs("/p/top.kicad_sch\tJ1\n/p/power.kicad_sch\tC39\tU2");
        assert_eq!(super::super::plot::sheet_of(&sheets, &refs, "1|/p|", "C39"), Some(1));
        assert_eq!(super::super::plot::sheet_of(&sheets, &refs, "1|/p|/p/power.kicad_sch", "C39"), None, "on show: nothing to say");
        assert_eq!(super::super::plot::sheet_of(&sheets, &refs, "1|/p|/p/power.kicad_sch", "J1"), Some(0));
        assert_eq!(super::super::plot::sheet_of(&sheets, &refs, "1|/p|", "J1"), None);
        assert_eq!(super::super::plot::sheet_of(&sheets, &refs, "1|/p|", "Q9"), None);
    }
    #[test] fn the_probe_card_is_a_head_and_at_most_four_rows() {
        let c = super::super::plot::card("pad 1;net=GND;type=smd;shape=custom;ref=H9;value=M2 standoff;footprint=SMTSO;layer=F.Cu").unwrap();
        let r = card_rows(&c);
        assert_eq!(r.len(), 4); assert_eq!(r[0], ("piece".to_string(), "pad 1".to_string())); assert_eq!(r[1].0, "footprint");
    }
    #[test] fn the_probe_chip_names_a_part_by_its_reference() {
        let c = super::super::plot::card("fp_poly;ref=C5;value=47uF 100V;net=GND").unwrap();
        assert_eq!(probe_words(&c), vec!["C5", "47uF 100V", "GND"]);
        let p = super::super::plot::card("pin 3;name=EN;type=input;symbol=U2;net=/EN").unwrap();
        assert_eq!(probe_words(&p), vec!["U2 pin 3", "/EN"], "a pin names its part");
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
        assert!(matches!(kicad_open(&serde_json::json!({})), Command::Action(a, v) if a == "kicad-open" && v.is_empty()));
        assert!(matches!(kicad_open(&serde_json::json!({"key":"0|/p|/p/b.kicad_pcb"})), Command::Action(_, v) if v == "0|/p|/p/b.kicad_pcb"));
        assert_eq!(shown_key(&serde_json::json!({"key":"split|/p|","panes":[{"key":"1|/p|/p/a.kicad_sch"},{"key":"0|/p|"}]})), "1|/p|/p/a.kicad_sch");
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
        assert_eq!(width_for(1600.0, 0.0, 71.0, 264.0), 905, "the thread keeps 360 (0.14)"); assert_eq!(width_for(1600.0, 0.0, 71.0, 0.0), 1169);
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

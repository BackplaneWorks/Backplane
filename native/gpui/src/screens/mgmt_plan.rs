//! The Plan panel (screens/mgmt.rs hosts it; the hub's plan/panel.rs publishes `mgmt.plan.<digest>`): the routing plan per layer, each
//! corridor and plane region with its state in the floorplanner's language, net count, width/impedance, reference plane, routing progress,
//! and blocker details; Approve or Reject an item, a layer or all (`plan.approve`), Stop routing (`plan.stop`), Verify (`plan.verify`),
//! Hide done; a row press selects the item (`plan_sel`) and zooms its nets in KiCad (`plan.focus`). Pure rules first, tested.
use crate::{hub::Command, i18n::{tf, trs}, screens::{composer::{Composer, ComposerEvent}, mgmt_plan_canvas::{self, GridMode, ViewMode}, settings::Secret}, theme, ui::ic, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

static HIDE_DONE: AtomicBool = AtomicBool::new(false);
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
/// Whether an item counts as done (Hide done leaves it out).
pub fn done(i: &Value) -> bool { matches!(s(i, "state"), "routed" | "verified") }
/// The state's mark beside an item's name: ✓ approved, ✎ edited, n/m routing, ✓✓ verified, ⚠ n blocked.
pub fn mark(i: &Value) -> String {
    match s(i, "state") {
        "approved" => "✓".into(), "edited" => "✎".into(), "verified" => "✓✓".into(), "routed" => "100%".into(), "rejected" => "✗".into(), "failed" => "✗".into(), // i18n-ok: marks
        "routing" => match (i["routed"].as_u64(), i["total"].as_u64()) { (Some(r), Some(t)) if t > 0 => format!("{r}/{t}"), _ => "…".into() },
        "blocked" => format!("⚠ {}", i["problems"].as_u64().unwrap_or(0).max(1)),
        _ => String::new(),
    }
}
/// Translated status label and semantic color token for the Plan summary badge.
pub fn status_badge(d: &Value) -> (String, &'static str) {
    match s(d, "status") {
        "blocked" => (trs("Blocked"), "bad"),
        "routing" => (trs("Routing"), "accent"),
        "awaiting" => (trs("Awaiting review"), "warn"),
        "verified" => (trs("Verified"), "ok"),
        "routed" => (trs("Routed"), "ok"),
        "ready" => (trs("Approved"), "accent"),
        "partial" => (trs("Partial"), "warn"),
        "rejected" => (trs("Rejected"), "bad"),
        _ => {
            if d["blocked"].as_u64().unwrap_or(0) > 0 { (trs("Blocked"), "bad") }
            else if d["routing"] == true { (trs("Routing"), "accent") }
            else if d["awaiting"].as_u64().unwrap_or(0) > 0 { (trs("Awaiting review"), "warn") }
            else { (trs("Empty"), "dim") }
        }
    }
}
/// Ordered `(count, color_token)` segments for the Plan overview progress bar.
pub fn progress_segments(d: &Value) -> [(u64, &'static str); 5] {
    let mut done_n = d["done"].as_u64().unwrap_or(0);
    let mut approved_n = d["approved"].as_u64().unwrap_or(0);
    let awaiting_n = d["awaiting"].as_u64().unwrap_or(0);
    let blocked_n = d["blocked"].as_u64().unwrap_or(0);
    let mut routing_n = 0u64;
    if d["total"].is_null() {
        for l in d["layers"].as_array().into_iter().flatten() {
            for i in l["items"].as_array().into_iter().flatten() {
                match s(i, "state") {
                    "routed" | "verified" => done_n += 1,
                    "routing" => routing_n += 1,
                    "approved" => approved_n += 1,
                    _ => {}
                }
            }
        }
    } else if d["routing"] == true {
        routing_n = d["total"].as_u64().unwrap_or(0).saturating_sub(done_n + approved_n + awaiting_n + blocked_n);
    }
    [(done_n, "ok"), (routing_n, "accent"), (approved_n, "input"), (awaiting_n, "warn"), (blocked_n, "bad")]
}
/// The rows a layer shows (Hide done leaves out routed and verified items).
pub fn shown(l: &Value, hide_done: bool) -> Vec<Value> { l["items"].as_array().into_iter().flatten().filter(|i| !(hide_done && done(i))).cloned().collect() }
fn hex(c: &str) -> u32 { u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0x8a8a8a) }

pub(super) fn send(app: &mut Backplane, m: &str, mut p: Value, cx: &mut Context<Backplane>) {
    let pro = app.screens.mgmt.project.clone(); if pro.is_empty() { return }
    p["pro"] = json!(pro);
    eprintln!("GPUI_LOG {}", json!({"event": "plan_press", "method": m, "params": p}));
    let tag = if matches!(m, "plan.approve" | "plan.update") { "plan" } else { "mgmt" };
    let _ = app.tx.try_send(Command::Rpc(m.into(), Secret(p), tag));
    app.ui_epoch += 1; cx.notify();
}
pub fn approve(app: &mut Backplane, layer: &str, ids: &[String], yes: bool, cx: &mut Context<Backplane>) { *REFUSED.lock().unwrap() = None; send(app, "plan.approve", json!({"layer": layer, "ids": ids, "approve": yes}), cx) }
/// KiCad's refusal of the last Approve or Reject (the hub's words: a dialog KiCad shows, named with the way out), until the next press.
static REFUSED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// plan.approve's answer (tag "plan"): a refusal is said in the panel; an approval shows in the plan itself.
pub fn answered(app: &mut Backplane, ok: bool, why: &str, cx: &mut Context<Backplane>) {
    *REFUSED.lock().unwrap() = (!ok).then(|| if why.is_empty() { trs("KiCad refused") } else { why.to_string() });
    if !ok { app.screens.mgmt.plan_ui.optimistic.clear(); }
    app.ui_epoch += 1; cx.notify();
}
/// The e2e harness's controls (`mgmt:plan:<what>`).
pub fn controls(m: &super::mgmt::Mgmt) -> Vec<(String, String, String)> {
    if m.peek()["open"] != "plan" { return vec![] }
    let d = m.data("plan"); let mut v: Vec<(String, String, String)> = vec![
        ("mgmt:plan:on".into(), "On".into(), "plan:enable:on".into()), ("mgmt:plan:off".into(), "Off".into(), "plan:enable:off".into()), // i18n-ok: ctl ids
        ("mgmt:plan:view:canvas".into(), "Canvas".into(), "plan:view:canvas".into()), ("mgmt:plan:view:split".into(), "Split".into(), "plan:view:split".into()), ("mgmt:plan:view:list".into(), "List".into(), "plan:view:list".into()), // i18n-ok: ctl ids
        ("mgmt:plan:grid:pair".into(), "Side by side".into(), "plan:grid:pair".into()), ("mgmt:plan:grid:all".into(), "All layers".into(), "plan:grid:all".into()), ("mgmt:plan:grid:single".into(), "Single".into(), "plan:grid:single".into()), // i18n-ok: ctl ids
        ("mgmt:plan:expand".into(), "Expand".into(), "plan:expand".into()), ("mgmt:plan:fit".into(), "Fit".into(), "plan:fit".into()), // i18n-ok: ctl ids
    ];
    if d["awaiting"].as_u64().unwrap_or(0) > 0 { v.push(("mgmt:plan:approve-all".into(), "Approve all".into(), "plan:all".into())) } // i18n-ok: ctl ids
    if d["routing"] == true { v.push(("mgmt:plan:stop".into(), "Stop routing".into(), "plan:stop".into())) } // i18n-ok: ctl ids
    if d["live"] == true && d["done"].as_u64().unwrap_or(0) > 0 { v.push(("mgmt:plan:verify".into(), "Verify".into(), "plan:verify".into())) } // i18n-ok: ctl ids
    for l in d["layers"].as_array().into_iter().flatten() { for i in l["items"].as_array().into_iter().flatten() {
        let id = s(i, "id"); v.push((format!("mgmt:plan:row:{id}"), id.into(), format!("plan:focus:{id}")));
        if matches!(s(i, "state"), "proposed" | "edited") { v.push((format!("mgmt:plan:approve:{id}"), id.into(), format!("plan:approve:{id}"))); v.push((format!("mgmt:plan:reject:{id}"), id.into(), format!("plan:reject:{id}"))) }
    } }
    v
}
pub fn act(app: &mut Backplane, a: &str, cx: &mut Context<Backplane>) {
    if a == "all" { approve(app, "", &[], true, cx) }
    else if let Some(x) = a.strip_prefix("enable:") { enable(app, x == "on", cx) }
    else if a == "stop" { send(app, "plan.stop", json!({}), cx) }
    else if a == "verify" { send(app, "plan.verify", json!({}), cx) }
    else if a == "view:canvas" { app.screens.mgmt.plan_ui.view_mode = ViewMode::Canvas; cx.notify(); }
    else if a == "view:split" { app.screens.mgmt.plan_ui.view_mode = ViewMode::Split; cx.notify(); }
    else if a == "view:list" { app.screens.mgmt.plan_ui.view_mode = ViewMode::List; cx.notify(); }
    else if a == "grid:pair" { app.screens.mgmt.plan_ui.grid_mode = GridMode::Pair; cx.notify(); }
    else if a == "grid:all" { app.screens.mgmt.plan_ui.grid_mode = GridMode::All; cx.notify(); }
    else if a == "grid:single" { app.screens.mgmt.plan_ui.grid_mode = GridMode::Single; cx.notify(); }
    else if a == "expand" { toggle_expand(app, cx); }
    else if a == "fit" { app.screens.mgmt.plan_ui.fit_all(); cx.notify(); }
    else if let Some(id) = a.strip_prefix("approve:") { approve(app, "", &[id.to_string()], true, cx) }
    else if let Some(id) = a.strip_prefix("reject:") { approve(app, "", &[id.to_string()], false, cx) }
    else if let Some(id) = a.strip_prefix("focus:") {
        app.screens.mgmt.plan_sel = Some(id.to_string());
        send(app, "plan.focus", json!({"id": id}), cx);
    }
}

fn toggle_expand(app: &mut Backplane, cx: &mut Context<Backplane>) {
    let next = !app.screens.mgmt.plan_ui.expanded;
    app.screens.mgmt.plan_ui.expanded = next;
    let target = if next { mgmt_plan_canvas::EXPANDED_PANEL_W } else { mgmt_plan_canvas::COMPACT_PANEL_W };
    let mut st = app.screens.mgmt.state();
    if !st.is_null() {
        st["size"] = json!(target);
        super::mgmt::set(app, st, json!({"size": target}), cx);
    } else {
        app.ui_epoch += 1;
        cx.notify();
    }
}

/// The Boards menu's value for a board the hub names (absolute `pcb`, `rel` from the project's folder): the menu's own entry when it
/// lists it, else `rel`.
pub fn board_value(boards: &[crate::viewers::boards::Board], pcb: &str, rel: &str) -> String {
    boards.iter().find(|b| !b.value.is_empty() && (pcb.ends_with(&format!("/{}", b.value)) || b.value == rel)).map_or(rel.to_string(), |b| b.value.clone())
}
/// Open in the viewer a board whose plan awaits the person, with its Plan panel open.
fn open_board(app: &mut Backplane, pcb: &str, rel: &str, cx: &mut Context<Backplane>) {
    let value = board_value(&crate::viewers::boards::list(&app.data["thread"]["viewer"]), pcb, rel);
    eprintln!("GPUI_LOG {}", json!({"event": "plan_open_board", "pcb": pcb, "value": value}));
    let pro = format!("{}.kicad_pro", pcb.strip_suffix(".kicad_pcb").unwrap_or(pcb));
    let _ = app.tx.try_send(Command::Rpc("mgmt.panel".into(), Secret(json!({"project": pro, "open": "plan"})), "mgmt"));
    crate::viewers::boards::pick(app, value, cx);
}
/// The person's switch (`plan.enable`): Plan before routing, off unless turned on.
fn enable(app: &mut Backplane, on: bool, cx: &mut Context<Backplane>) { send(app, "plan.enable", json!({"on": on}), cx) }
fn chip(cx: &mut Context<Backplane>, light: bool, id: &str, label: String, on: bool, f: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n); let rect = id.to_string();
    crate::ui::with_rect(div().id(SharedString::from(id.to_string())), move || rect).flex_shrink_0().h(crate::scale::tpx(20.)).px_2().flex().items_center().text_size(crate::scale::tpx(11.)).cursor_pointer().border_1()
        .border_color(c(if on { "accent" } else { "line" }))
        .when(on, |d| d.bg(c("accent")).text_color(c("on-accent"))).when(!on, |d| d.bg(c("raise")).text_color(c("dim")).hover(|s| s.bg(c("line"))))
        .child(label).on_click(cx.listener(move |this, _, _, cx| { f(this, cx); cx.notify(); })).into_any_element()
}
/// Layers to render in the list section for the current view mode.
///
/// In `Split` mode the list mirrors only the layer cards currently visible on the canvas above;
/// in `List` mode it shows every planned layer.
pub fn list_layers_for_view(view_mode: ViewMode, all_layers: &[Value], visible_names: &[String]) -> Vec<Value> {
    if view_mode == ViewMode::Split && !visible_names.is_empty() {
        return all_layers
            .iter()
            .filter(|l| l["layer"].as_str().is_some_and(|n| visible_names.iter().any(|v| v == n)))
            .cloned()
            .collect();
    }
    all_layers.to_vec()
}

/// Compact header switch row + status summary + flush progress bar + cross-board approval banners.
fn heading(app: &mut Backplane, d: &Value, light: bool, cx: &mut Context<Backplane>) -> Vec<AnyElement> {
    let c = |n: &str| theme::color(light, n);
    let on = d["on"] == true;
    let empty = d["empty"] == true;
    let tip_box = if on {
        crate::keys::tip("Before routing a board with 4 or more copper layers or more than 60 nets, the agent proposes corridors and plane regions here. It routes only what you approve.", "", light)
    } else {
        crate::keys::tip("Agents route as you ask. Turn it on to have them propose a routing plan first and route only what you approve here.", "", light)
    };
    let mut row = div().flex().items_center().gap_2().px_2().py(px(5.))
        .child(ic(light, "layers", 12., if on { "accent" } else { "dim" }))
        .child(div().id("plan-toggle-lbl").flex_shrink_0().text_color(c("text")).tooltip(tip_box).child(trs("Plan before routing")));

    if !empty {
        let awaiting = d["awaiting"].as_u64().unwrap_or(0);
        let blocked = d["blocked"].as_u64().unwrap_or(0);
        let done_n = d["done"].as_u64().unwrap_or(0);
        let total_n = d["total"].as_u64().unwrap_or(0);
        let rev = d["revision"].as_u64().unwrap_or(0);
        let (status_lbl, status_tone) = status_badge(d);
        row = row
            .child(div().px_1().border_1().border_color(c(status_tone)).bg(c("raise")).text_size(crate::scale::tpx(10.)).text_color(c(status_tone)).font_weight(FontWeight::BOLD).child(status_lbl))
            .when(rev > 0, |x| x.child(div().text_size(crate::scale::tpx(10.)).text_color(c("dim")).child(format!("v{rev}"))))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_size(crate::scale::tpx(10.)).text_color(c(if awaiting > 0 { "warn" } else if blocked > 0 { "bad" } else { "dim" }))
                /* i18n-ok: formatted count */ .child(if total_n > 0 { format!("{done_n}/{total_n} {}", trs("done")) } else if awaiting > 0 { tf("gpui.mgmt.plan.awaiting", &[("n", awaiting.into())]) } else { trs("Nothing awaits your approval") }))
            .when(!d["live"].as_bool().unwrap_or(false), |x| x.child(div().px_1().border_1().border_color(c("line")).text_size(crate::scale::tpx(10.)).text_color(c("faint")).child(trs("saved plan"))));
    } else {
        row = row.child(div().flex_1());
    }

    row = row
        .child(chip(cx, light, "mgmt:plan:on", trs("On"), on, |a, cx| enable(a, true, cx)))
        .child(chip(cx, light, "mgmt:plan:off", trs("Off"), !on, |a, cx| enable(a, false, cx)));

    let mut hdr = div().flex_shrink_0().flex().flex_col().bg(c("panel")).border_b_1().border_color(c("rule")).text_size(crate::scale::tpx(11.)).child(row);
    if !empty {
        let segs = progress_segments(d);
        let seg_total: u64 = segs.iter().map(|(n, _)| *n).sum();
        if seg_total > 0 {
            hdr = hdr.child(div().w_full().h(px(3.)).bg(c("line")).flex()
                .children(segs.into_iter().filter(|(n, _)| *n > 0).map(|(n, tone)| {
                    let mut bar = div().h_full().bg(c(tone));
                    bar.style().flex_grow = Some(n as f32);
                    bar
                })));
        }
    }

    let mut v = vec![hdr.into_any_element()];
    let _ = app;
    for e in d["elsewhere"].as_array().cloned().unwrap_or_default() {
        let (pcb, rel) = (s(&e, "pcb").to_string(), s(&e, "rel").to_string());
        v.push(div().flex_shrink_0().flex().items_center().gap_2().px_2().py_1().border_b_1().border_color(c("rule")).bg(c("panel")).text_size(crate::scale::tpx(11.))
            .child(div().w(px(2.)).h(px(14.)).flex_shrink_0().bg(c("warn")))
            .child(div().flex_1().min_w_0().text_color(c("warn")).child(tf("gpui.mgmt.plan.elsewhere", &[("board", s(&e, "name").to_string().into())])))
            .child(btn(cx, light, format!("mgmt:plan:open:{}", s(&e, "name")), trs("Open board"), false, move |a, cx| open_board(a, &pcb, &rel, cx)))
            .into_any_element());
    }
    v
}

fn btn(cx: &mut Context<Backplane>, light: bool, id: String, label: String, primary: bool, on: impl Fn(&mut Backplane, &mut Context<Backplane>) + 'static) -> AnyElement {
    let c = |n: &str| theme::color(light, n); let rect = id.clone();
    let d = crate::ui::with_rect(div().id(SharedString::from(id)), move || rect).flex_shrink_0().h(crate::scale::tpx(21.)).px_2().flex().items_center().text_size(crate::scale::tpx(11.)).cursor_pointer().border_1();
    let d = if primary {
        d.bg(c("accent")).border_color(c("accent")).text_color(c("on-accent")).hover(|s| s.opacity(0.9))
    } else {
        d.bg(c("raise")).border_color(c("line")).text_color(c("text")).hover(|s| s.bg(c("line")))
    };
    d.child(label).on_click(cx.listener(move |this, _, _, cx| { on(this, cx); cx.notify(); })).into_any_element()
}

pub fn body(app: &mut Backplane, d: &Value, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    if d.is_null() { return div().p_3().text_size(crate::scale::tpx(12.)).text_color(c("faint")).child(trs("Reading the routing plan…")).into_any_element() }
    let head = heading(app, d, light, cx);
    if d["empty"] == true {
        return div().size_full().flex().flex_col().children(head)
            .child(div().p_3().flex().flex_col().gap_1()
                .child(div().text_size(crate::scale::tpx(12.)).text_color(c("dim")).child(trs("No routing plan for this board.")))
                .child(div().text_size(crate::scale::tpx(11.)).text_color(c("faint")).child(trs("Layers, corridors, and planes appear here once a plan is proposed."))))
            .into_any_element()
    }
    mgmt_plan_canvas::sync_revision_and_hydrate(app, d);
    let hide = HIDE_DONE.load(Ordering::Relaxed);
    let view_mode = app.screens.mgmt.plan_ui.view_mode;
    let grid_mode = app.screens.mgmt.plan_ui.grid_mode;
    let sync_zoom = app.screens.mgmt.plan_ui.sync_zoom;
    let expanded = app.screens.mgmt.plan_ui.expanded;
    let awaiting = d["awaiting"].as_u64().unwrap_or(0);
    let done_n = d["done"].as_u64().unwrap_or(0);
    let stackup_sum = s(d, "stackup_summary").to_string();
    let imps: Vec<String> = d["impedances"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    let has_stackup = !stackup_sum.is_empty() || !imps.is_empty();
    let stackup_open = app.screens.mgmt.plan_stackup_open;

    let mut act_row = div().flex_shrink_0().min_h(crate::scale::tpx(super::head::ROW_H)).flex().flex_wrap().items_center().gap(px(6.)).px_2().py_1().border_b_1().border_color(c("rule")).text_size(crate::scale::tpx(11.));
    if awaiting > 0 { act_row = act_row.child(btn(cx, light, "mgmt:plan:approve-all".into(), trs("Approve all"), true, |a, cx| approve(a, "", &[], true, cx))) }
    if d["routing"] == true { act_row = act_row.child(btn(cx, light, "mgmt:plan:stop".into(), trs("Stop routing"), false, |a, cx| send(a, "plan.stop", json!({}), cx))) }
    if d["live"] == true && done_n > 0 { act_row = act_row.child(btn(cx, light, "mgmt:plan:verify".into(), trs("Verify"), false, |a, cx| send(a, "plan.verify", json!({}), cx))) }
    act_row = act_row.child(div().flex_1());
    if has_stackup {
        act_row = act_row.child(chip(cx, light, "plan-stackup-toggle", format!("{} {}", trs("Stackup"), if stackup_open { "▾" } else { "▸" }), stackup_open, |a, cx| {
            a.screens.mgmt.plan_stackup_open = !a.screens.mgmt.plan_stackup_open;
            cx.notify();
        }));
    }
    act_row = act_row.child(btn(cx, light, "mgmt:plan:hide".into(), if hide { trs("Show done") } else { trs("Hide done") }, false, |_, cx| { HIDE_DONE.fetch_xor(true, Ordering::Relaxed); cx.notify(); }));

    let asks = d["asks"].as_array().cloned().unwrap_or_default();
    let ask_banner = (!asks.is_empty()).then(|| {
        let what = asks.iter().filter_map(|a| a["what"].as_str()).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ");
        let note = asks.first().and_then(|a| a["note"].as_str()).unwrap_or("");
        div().flex_shrink_0().flex().gap_2().px_2().py_1().border_b_1().border_color(c("rule")).bg(c("panel")).text_size(crate::scale::tpx(11.))
            .child(div().w(px(2.)).flex_shrink_0().bg(c("warn")))
            .child(div().flex_1().min_w_0().flex().flex_col()
                .child(div().text_color(c("warn")).child(if what.is_empty() { trs("Approval requested") } else { tf("gpui.mgmt.plan.requested", &[("what", what.into())]) }))
                .when(!note.is_empty(), |x| x.child(div().text_color(c("dim")).child(note.to_string()))))
            .into_any_element()
    });

    let stackup_box = (has_stackup && stackup_open).then(|| {
        div().flex_shrink_0().flex().flex_col().gap(px(2.)).px_3().py_2().border_b_1().border_color(c("rule")).bg(c("bg")).text_size(crate::scale::tpx(11.)).text_color(c("dim"))
            .when(!stackup_sum.is_empty(), |x| x.child(div().child(stackup_sum.clone())))
            /* i18n-ok: hub impedance spec */ .children(imps.into_iter().map(|imp| div().text_color(c("faint")).child(format!("· {imp}"))))
            .into_any_element()
    });

    let view_bar = div()
        .flex_shrink_0()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(4.))
        .px_2()
        .py(px(4.))
        .border_b_1()
        .border_color(c("rule"))
        .bg(c("panel"))
        .text_size(crate::scale::tpx(11.))
        .child(chip(cx, light, "mgmt:plan:view:canvas", trs("Canvas"), view_mode == ViewMode::Canvas, |a, cx| { a.screens.mgmt.plan_ui.view_mode = ViewMode::Canvas; cx.notify(); }))
        .child(chip(cx, light, "mgmt:plan:view:split", trs("Split"), view_mode == ViewMode::Split, |a, cx| { a.screens.mgmt.plan_ui.view_mode = ViewMode::Split; cx.notify(); }))
        .child(chip(cx, light, "mgmt:plan:view:list", trs("List"), view_mode == ViewMode::List, |a, cx| { a.screens.mgmt.plan_ui.view_mode = ViewMode::List; cx.notify(); }))
        .when(view_mode != ViewMode::List, |bar| {
            bar.child(div().w(px(1.)).h(px(12.)).bg(c("line")))
                .child(chip(cx, light, "mgmt:plan:grid:pair", trs("Side by side"), grid_mode == GridMode::Pair, |a, cx| { a.screens.mgmt.plan_ui.grid_mode = GridMode::Pair; cx.notify(); }))
                .child(chip(cx, light, "mgmt:plan:grid:all", trs("All layers"), grid_mode == GridMode::All, |a, cx| { a.screens.mgmt.plan_ui.grid_mode = GridMode::All; cx.notify(); }))
                .child(chip(cx, light, "mgmt:plan:grid:single", trs("Single"), grid_mode == GridMode::Single, |a, cx| { a.screens.mgmt.plan_ui.grid_mode = GridMode::Single; cx.notify(); }))
                .child(div().w(px(1.)).h(px(12.)).bg(c("line")))
                .child(chip(cx, light, "mgmt:plan:sync", trs("Link zoom"), sync_zoom, |a, cx| { a.screens.mgmt.plan_ui.sync_zoom = !a.screens.mgmt.plan_ui.sync_zoom; cx.notify(); }))
                .child(btn(cx, light, "mgmt:plan:fit".into(), trs("Fit"), false, |a, cx| { a.screens.mgmt.plan_ui.fit_all(); cx.notify(); }))
        })
        .child(div().flex_1())
        .child(btn(cx, light, "mgmt:plan:expand".into(), if expanded { trs("Compact") } else { trs("Expand") }, false, |a, cx| toggle_expand(a, cx)));

    let layers_merged = mgmt_plan_canvas::merge_optimistic_into_layers(&d["layers"], &app.screens.mgmt.plan_ui.optimistic);
    let all_layers_arr = layers_merged.as_array().cloned().unwrap_or_default();
    let copper_arr = d["copper_layers"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let canvas_layers = mgmt_plan_canvas::all_canvas_layers(&all_layers_arr, copper_arr);
    let shown_names: Vec<String> = mgmt_plan_canvas::visible_layers(&app.screens.mgmt.plan_ui, &canvas_layers)
        .iter()
        .filter_map(|l| l["layer"].as_str().map(str::to_string))
        .collect();
    let layer_bar = (view_mode != ViewMode::List && grid_mode != GridMode::All && canvas_layers.len() > 1).then(|| {
        let mut row = div().flex_shrink_0().flex().flex_wrap().items_center().gap(px(4.)).px_2().py(px(3.)).border_b_1().border_color(c("rule")).bg(c("bg")).text_size(crate::scale::tpx(10.));
        for l in &canvas_layers {
            let lname = s(l, "layer").to_string();
            let base_title = mgmt_plan_canvas::layer_display_title(&lname, copper_arr, &canvas_layers);
            let item_n = l["items"].as_array().map_or(0, Vec::len);
            let title = if item_n > 0 { format!("{base_title} · {item_n}") } else { base_title }; // i18n-ok: item count suffix
            let active = shown_names.contains(&lname);
            let empty_layer = item_n == 0;
            let lc = lname.clone();
            let pill = crate::ui::with_rect(div().id(SharedString::from(format!("mgmt:plan:layerpill:{lname}"))), {
                let id_s = format!("mgmt:plan:layerpill:{lname}");
                move || id_s.clone()
            })
            .flex_shrink_0()
            .h(crate::scale::tpx(20.))
            .px_2()
            .flex()
            .items_center()
            .text_size(crate::scale::tpx(11.))
            .cursor_pointer()
            .border_1()
            .border_color(c(if active { "accent" } else { "line" }))
            .when(active, |d| d.bg(c("accent")).text_color(c("on-accent")))
            .when(!active, |d| d.bg(c("raise")).text_color(c(if empty_layer { "faint" } else { "dim" })).hover(|s| s.bg(c("line"))))
            .when(empty_layer && !active, |d| d.opacity(0.65))
            .child(title)
            .on_click(cx.listener(move |a, _, _, cx| {
                mgmt_plan_canvas::pick_layer_in_grid(&mut a.screens.mgmt.plan_ui, lc.clone());
                cx.notify();
            }));
            row = row.child(pill);
        }
        row.into_any_element()
    });

    let move_layer_open = app.screens.mgmt.plan_ui.move_layer_open;
    let sel_inspector = app.screens.mgmt.plan_sel.as_ref().and_then(|sel_id| {
        let mut found: Option<(String, Value)> = None;
        for l in &all_layers_arr {
            for it in l["items"].as_array().into_iter().flatten() {
                if it["id"].as_str() == Some(sel_id.as_str()) {
                    found = Some((s(l, "layer").to_string(), it.clone()));
                    break;
                }
            }
            if found.is_some() { break }
        }
        let (cur_layer, item) = found?;
        let cur_layer_title = mgmt_plan_canvas::layer_display_title(&cur_layer, copper_arr, &canvas_layers);
        let id = sel_id.clone();
        let name = s(&item, "name").to_string();
        let stt = s(&item, "state").to_string();
        let locked = item["locked"] == true;
        let edited = item["edited"] == true || stt == "edited";
        let is_corridor = s(&item, "kind") == "corridor";
        let width_mm = item["width"].as_f64().unwrap_or(1.5);
        let col = if is_corridor {
            mgmt_plan_canvas::bus_color(&name, s(&item, "color"))
        } else {
            mgmt_plan_canvas::rail_color(&name, s(&item, "color"))
        };
        let copper_opts: Vec<String> = d["copper_layers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["layer"].as_str().map(str::to_string))
            .collect();
        let move_layers = if copper_opts.is_empty() {
            all_layers_arr.iter().filter_map(|l| l["layer"].as_str().map(str::to_string)).collect()
        } else {
            copper_opts
        };

        let id_lock = id.clone();
        let id_ok = id.clone();
        let id_no = id.clone();
        let id_wdown = id.clone();
        let id_wup = id.clone();

        let mut box_el = div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(4.))
            .px_2()
            .py(px(6.))
            .border_b_1()
            .border_color(c("rule"))
            .bg(c("panel"))
            .text_size(crate::scale::tpx(11.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .child(div().size(px(10.)).bg(rgb(col)))
                    .child(div().font_weight(FontWeight::BOLD).text_color(c("text")).child(name))
                    .child(div().text_color(c("dim")).child(cur_layer_title.clone()))
                    .when(edited, |x| x.child(div().px_1().border_1().border_color(c("warn")).text_size(crate::scale::tpx(10.)).text_color(c("warn")).child(trs("Edited"))))
                    .when(locked, |x| x.child(div().px_1().border_1().border_color(c("line")).text_size(crate::scale::tpx(10.)).text_color(c("dim")).child(trs("Locked"))))
                    .child(div().flex_1())
                    .child(btn(cx, light, "mgmt:plan:ins:lock".into(), if locked { trs("Unlock") } else { trs("Lock") }, false, move |a, cx| {
                        mgmt_plan_canvas::commit_item_fields(a, &id_lock, json!({"locked": !locked}), cx);
                    }))
                    .when(matches!(stt.as_str(), "proposed" | "edited"), |x| {
                        let a_id = id_ok.clone();
                        let r_id = id_no.clone();
                        x.child(btn(cx, light, "mgmt:plan:ins:ok".into(), trs("Approve"), true, move |a, cx| approve(a, "", &[a_id.clone()], true, cx)))
                            .child(btn(cx, light, "mgmt:plan:ins:no".into(), trs("Reject"), false, move |a, cx| approve(a, "", &[r_id.clone()], false, cx)))
                    })
                    .child(btn(cx, light, "mgmt:plan:ins:note".into(), trs("Add note"), false, |a, cx| {
                        a.screens.mgmt.plan_ui.note_open = !a.screens.mgmt.plan_ui.note_open;
                        cx.notify();
                    }))
                    .child(div().id("plan-ins-close").cursor_pointer().px_1().text_color(c("dim")).hover(|s| s.text_color(c("text"))).child("\u{d7}")
                        .on_click(cx.listener(|a, _, _, cx| {
                            a.screens.mgmt.plan_sel = None;
                            a.screens.mgmt.plan_ui.selected_vertex = None;
                            a.screens.mgmt.plan_ui.move_layer_open = false;
                            cx.notify();
                        }))),
            );

        if !locked {
            let mut steer_row = div().flex().flex_wrap().items_center().gap(px(5.)).text_size(crate::scale::tpx(10.));
            if is_corridor {
                let wd_dn = (((width_mm - 0.25).clamp(0.15, 25.0)) * 100.0).round() / 100.0;
                let wd_up = (((width_mm + 0.25).clamp(0.15, 25.0)) * 100.0).round() / 100.0;
                steer_row = steer_row
                    .child(div().text_color(c("dim")).child(trs("Width")))
                    .child(btn(cx, light, "mgmt:plan:ins:w-".into(), "-".into(), false, move |a, cx| {
                        mgmt_plan_canvas::commit_item_fields(a, &id_wdown, json!({"width": wd_dn}), cx);
                    }))
                    /* i18n-ok: metric width readout */
                    .child(div().px_1().font(crate::art::mono_font()).text_color(c("text")).child(format!("{width_mm:.2}mm")))
                    .child(btn(cx, light, "mgmt:plan:ins:w+".into(), "+".into(), false, move |a, cx| {
                        mgmt_plan_canvas::commit_item_fields(a, &id_wup, json!({"width": wd_up}), cx);
                    }))
                    .child(div().w(px(1.)).h(px(12.)).bg(c("line")));
            }
            if !move_layers.is_empty() {
                steer_row = steer_row
                    .child(div().text_color(c("dim")).child(trs("Layer")))
                    .child(chip(cx, light, "mgmt:plan:ins:mvtoggle", format!("{cur_layer_title} {}", if move_layer_open { "▴" } else { "▾" }), move_layer_open, |a, cx| { // i18n-ok: canonical layer title and chevron
                        a.screens.mgmt.plan_ui.move_layer_open = !a.screens.mgmt.plan_ui.move_layer_open;
                        cx.notify();
                    }));
            }
            box_el = box_el.child(steer_row);
            if move_layer_open && !move_layers.is_empty() {
                let mut mv_row = div().flex().flex_wrap().items_center().gap(px(4.)).pt(px(2.)).text_size(crate::scale::tpx(10.));
                for ml in move_layers {
                    let is_cur = ml == cur_layer;
                    let ml_title = mgmt_plan_canvas::layer_display_title(&ml, copper_arr, &canvas_layers);
                    let id_mv = id.clone();
                    let ml_c = ml.clone();
                    mv_row = mv_row.child(chip(cx, light, &format!("mgmt:plan:ins:mv:{ml}"), ml_title, is_cur, move |a, cx| {
                        a.screens.mgmt.plan_ui.move_layer_open = false;
                        if !is_cur {
                            mgmt_plan_canvas::commit_item_fields(a, &id_mv, json!({"layer": ml_c.clone()}), cx);
                            mgmt_plan_canvas::pick_layer_in_grid(&mut a.screens.mgmt.plan_ui, ml_c.clone());
                        }
                        cx.notify();
                    }));
                }
                box_el = box_el.child(mv_row);
            }
        }

        if app.screens.mgmt.plan_ui.note_open {
            if app.screens.mgmt.plan_ui.note_input.is_none() {
                let f = cx.new(|cx| {
                    let mut comp = Composer::with_context(cx, "AskInput");
                    comp.set_placeholder(&trs("Steering note for router…"));
                    comp.max_lines = 1.0;
                    comp
                });
                cx.subscribe(&f, |this, e, ev: &ComposerEvent, cx| {
                    if matches!(ev, ComposerEvent::Submit) {
                        let txt = e.read(cx).content.to_string().trim().to_string();
                        if !txt.is_empty() {
                            if let Some(target_id) = this.screens.mgmt.plan_sel.clone() {
                                let mut payload = json!({"ops": [{"op": "note", "target": target_id, "text": txt}]});
                                if let Some(b) = this.screens.mgmt.data("plan")["revision"].as_u64() {
                                    payload["base"] = json!(b);
                                }
                                send(this, "plan.update", payload, cx);
                            }
                            e.update(cx, |comp, cx| comp.set(String::new(), cx));
                            this.screens.mgmt.plan_ui.note_open = false;
                            cx.notify();
                        }
                    }
                }).detach();
                app.screens.mgmt.plan_ui.note_input = Some(f);
            }
            if let Some(field) = app.screens.mgmt.plan_ui.note_input.clone() {
                let f_sub = field.clone();
                box_el = box_el.child(
                    div().flex().items_center().gap_1()
                        .child(div().flex_1().min_w_0().bg(c("bg")).border_1().border_color(c("line")).px_1().child(field))
                        .child(btn(cx, light, "mgmt:plan:ins:notesend".into(), trs("Add note"), true, move |this, cx| {
                            let txt = f_sub.read(cx).content.to_string().trim().to_string();
                            if !txt.is_empty() {
                                if let Some(target_id) = this.screens.mgmt.plan_sel.clone() {
                                    let mut payload = json!({"ops": [{"op": "note", "target": target_id, "text": txt}]});
                                    if let Some(b) = this.screens.mgmt.data("plan")["revision"].as_u64() {
                                        payload["base"] = json!(b);
                                    }
                                    send(this, "plan.update", payload, cx);
                                }
                                f_sub.update(cx, |comp, cx| comp.set(String::new(), cx));
                                this.screens.mgmt.plan_ui.note_open = false;
                                cx.notify();
                            }
                        })),
                );
            }
        }

        if view_mode != ViewMode::List && !locked {
            box_el = box_el.child(div().text_size(crate::scale::tpx(10.)).text_color(c("faint")).child(trs("Drag waypoints to steer · Click + on edge to insert · Right-click vertex to remove")));
        }

        Some(box_el.into_any_element())
    });

    let mut list = div().id("plan-rows").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().text_size(crate::scale::tpx(12.));
    let sel = app.screens.mgmt.plan_sel.clone();
    let list_layers = list_layers_for_view(view_mode, &all_layers_arr, &shown_names);
    let list_layer_count = list_layers.len();
    if view_mode == ViewMode::Split && list_layers.is_empty() {
        list = list.child(
            div()
                .px_2()
                .py_2()
                .text_size(crate::scale::tpx(11.))
                .text_color(c("faint"))
                .child(trs("No corridors or planes on this layer")),
        );
    }
    for l in list_layers {
        let layer = s(&l, "layer").to_string();
        let layer_title = mgmt_plan_canvas::layer_display_title(&layer, copper_arr, &canvas_layers);
        let total = l["items"].as_array().map_or(0, Vec::len);
        let done_layer = l["done"].as_u64().unwrap_or(0);
        let stack = s(&l, "stackup").to_string();
        let role = s(&l, "role").to_string();
        let lid = layer.clone(); let lid2 = layer.clone();
        if view_mode == ViewMode::Split {
            if list_layer_count > 1 {
                let is_power = role == "power" || role == "plane";
                list = list.child(
                    div().flex_shrink_0().flex().items_center().gap_2().px_2().py(px(3.)).bg(c("panel")).border_b_1().border_color(c("line")).text_size(crate::scale::tpx(10.))
                        .child(div().w(px(3.)).h(px(12.)).bg(c(if is_power { "warn" } else { "accent" })))
                        .child(div().flex_shrink_0().font_weight(FontWeight::BOLD).text_color(c("text")).child(layer_title))
                        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("dim"))
                            .child(tf("gpui.mgmt.plan.done.of", &[("done", done_layer.into()), ("total", total.into())])))
                );
            }
        } else {
            let mut head_row = div().flex_shrink_0().flex().flex_col().px_2().py_1().bg(c("panel")).border_b_1().border_color(c("line")).text_size(crate::scale::tpx(11.))
                .child({
                    let mut top = div().flex().items_center().gap_2()
                        .child(div().flex_shrink_0().font_weight(FontWeight::BOLD).text_color(c("text")).child(layer_title))
                        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("dim"))
                            /* i18n-ok: layer count */ .child(format!("{}{}/{} {}", if role.is_empty() { String::new() } else { format!("{role} · ") }, done_layer, total, trs("done"))));
                    if s(&l, "status") != "approved" && total > 0 {
                        top = top.child(btn(cx, light, format!("mgmt:plan:layer-ok:{layer}"), trs("Approve layer"), false, move |a, cx| approve(a, &lid, &[], true, cx)))
                            .child(btn(cx, light, format!("mgmt:plan:layer-no:{layer}"), trs("Reject"), false, move |a, cx| approve(a, &lid2, &[], false, cx)));
                    }
                    top
                });
            if !stack.is_empty() {
                head_row = head_row.child(div().text_size(crate::scale::tpx(10.)).text_color(c("faint")).overflow_hidden().text_ellipsis().whitespace_nowrap().child(stack));
            }
            list = list.child(head_row);
        }
        for i in shown(&l, hide) {
            let id = s(&i, "id").to_string(); let col = hex(s(&i, "color")); let stt = s(&i, "state").to_string();
            let selected = sel.as_deref() == Some(id.as_str());
            let swatch = match stt.as_str() {
                "proposed" => div().w(px(14.)).h(px(10.)).flex_shrink_0().bg(rgba((col << 8) | 0x40)).border_1().border_dashed().border_color(rgb(col)),
                "edited" => div().w(px(14.)).h(px(10.)).flex_shrink_0().bg(rgba((col << 8) | 0x60)).border_1().border_color(c("warn")),
                "routed" | "verified" => div().w(px(14.)).h(px(10.)).flex_shrink_0().bg(rgba((col << 8) | 0x30)).border_1().border_color(c("ok")),
                "blocked" => div().w(px(14.)).h(px(10.)).flex_shrink_0().bg(rgba((col << 8) | 0x60)).border_2().border_color(c("bad")),
                "rejected" | "failed" => div().w(px(14.)).h(px(10.)).flex_shrink_0().bg(c("raise")).border_1().border_color(c("line")),
                _ => div().w(px(14.)).h(px(10.)).flex_shrink_0().bg(rgb(col)).border_1().border_color(rgb(col)),
            };
            let name = format!("{}{}", s(&i, "name"), if i["locked"] == true { " 🔒" } else { "" }); // i18n-ok: a mark
            let mut detail = if s(&i, "kind") == "plane" {
                i["reference"].as_str().filter(|r| !r.is_empty()).map(|r| tf("gpui.mgmt.plan.reference", &[("layer", r.to_string().into())])).unwrap_or_default()
            } else {
                tf("gpui.mgmt.plan.nets", &[("n", i["nets"].as_u64().unwrap_or(0).into())])
            };
            if let Some(w) = i["width"].as_f64() { detail = format!("{detail} · {w:.2}mm"); } // i18n-ok: width in mm
            if let Some(z) = i["impedance"].as_str().filter(|x| !x.is_empty()) { detail = format!("{detail} · {z}"); } // i18n-ok: impedance value
            let tone = match stt.as_str() { "blocked" | "failed" => "bad", "edited" => "warn", "rejected" | "routed" | "verified" => "faint", _ => "text" };
            let rid = id.clone();
            let rid_focus = id.clone();
            let problem = s(&i, "problem_text").to_string();
            let (rt, tot) = (i["routed"].as_u64().unwrap_or(0), i["total"].as_u64().unwrap_or(0));

            let mut top_line = div().flex().items_center().gap_2()
                .child(swatch)
                .child(div().flex_1().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(c(tone)).child(name))
                .child(div().flex_shrink_0().w(crate::scale::tpx(44.)).text_size(crate::scale::tpx(11.)).text_color(c(match stt.as_str() { "blocked" | "failed" => "bad", "verified" | "routed" => "ok", "approved" => "accent", _ => "dim" })).child(mark(&i)));
            if matches!(stt.as_str(), "proposed" | "edited") {
                let (a, b) = (id.clone(), id.clone());
                top_line = top_line
                    .child(btn(cx, light, format!("mgmt:plan:approve:{id}"), "✓".into(), false, move |x, cx| approve(x, "", &[a.clone()], true, cx))) // i18n-ok: a mark
                    .child(btn(cx, light, format!("mgmt:plan:reject:{id}"), "✗".into(), false, move |x, cx| approve(x, "", &[b.clone()], false, cx))); // i18n-ok: a mark
            }

            let row = crate::ui::with_rect(div().id(SharedString::from(format!("plan-row-{id}"))), move || format!("mgmt:plan:row:{rid}"))
                .flex_shrink_0().flex().flex_row().border_b_1().border_color(c("line"))
                .when(selected, |d| d.bg(c("raise")))
                .cursor_pointer().hover(|x| x.bg(c("panel")))
                .child(div().w(px(2.)).flex_shrink_0().bg(if selected { c("accent") } else { gpui::transparent_black().into() }))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(px(3.)).pl(px(10.)).pr_2().py(px(4.))
                    .child(top_line)
                    .when(!detail.is_empty(), |d| d.child(div().pl(px(22.)).text_size(crate::scale::tpx(11.)).text_color(c("dim")).child(detail)))
                    .when(stt == "routing" && tot > 0, |d| {
                        let frac = (rt as f32 / tot as f32).clamp(0.0, 1.0);
                        d.child(div().w_full().h(px(2.)).bg(c("line")).child(div().h_full().w(relative(frac)).bg(c("accent"))))
                    })
                    .when(!problem.is_empty(), |d| d.child(div().pl(px(22.)).text_size(crate::scale::tpx(11.)).text_color(c("bad")).child(problem))))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.screens.mgmt.plan_sel = Some(rid_focus.clone());
                    send(this, "plan.focus", json!({"id": rid_focus}), cx);
                }));
            list = list.child(row);
        }
    }
    let note = |t: String, tone: &str| div().flex_shrink_0().px_2().py_1().border_b_1().border_color(c("rule")).text_size(crate::scale::tpx(11.)).text_color(c(tone)).child(trs(&t)).into_any_element();
    let keys = d["keys_note"].as_str().map(|t| note(t.to_string(), "dim"));
    let refused = REFUSED.lock().unwrap().clone().map(|t| {
        div().flex_shrink_0().flex().items_center().gap_2().px_2().py_1().border_b_1().border_color(c("rule")).bg(c("panel")).text_size(crate::scale::tpx(11.)).text_color(c("bad"))
            .child(div().flex_1().min_w_0().child(trs(&t)))
            .child(div().id("plan-refused-x").cursor_pointer().px_1().text_color(c("dim")).hover(|s| s.text_color(c("text"))).child("\u{d7}")
                .on_click(cx.listener(|_, _, _, cx| { *REFUSED.lock().unwrap() = None; cx.notify(); })))
            .into_any_element()
    });
    let drawn = (d["draw"]["bands"].as_array().map_or(false, |a| !a.is_empty()) || d["draw"]["regions"].as_array().map_or(false, |a| !a.is_empty()))
        .then(|| div().flex_shrink_0().px_2().py(px(3.)).border_t_1().border_color(c("rule")).text_size(crate::scale::tpx(10.)).text_color(c("faint")).child(trs("The plan's corridors and planes are drawn on the board while this panel is open. Close it to hide them.")).into_any_element());
    let canvas_el = (view_mode != ViewMode::List).then(|| mgmt_plan_canvas::view(app, d, light, cx));
    let main_body = match view_mode {
        ViewMode::Canvas => div().flex_1().min_h_0().children(canvas_el).into_any_element(),
        ViewMode::Split => div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(div().flex_1().min_h(px(210.)).border_b_1().border_color(c("rule")).children(canvas_el))
            .child(div().h(px(190.)).flex_shrink_0().min_h(px(120.)).flex().flex_col().child(list))
            .into_any_element(),
        ViewMode::List => list.into_any_element(),
    };
    div().size_full().flex().flex_col().children(head).child(act_row).child(view_bar).children(layer_bar).children(ask_banner).children(refused).children(keys).children(stackup_box).children(sel_inspector).child(main_body).children(drawn).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn marks_and_hide_done() {
        assert_eq!(mark(&json!({"state": "routing", "routed": 9, "total": 14})), "9/14"); assert_eq!(mark(&json!({"state": "approved"})), "✓");
        assert_eq!(mark(&json!({"state": "blocked", "problems": 2})), "⚠ 2"); assert_eq!(mark(&json!({"state": "proposed"})), "");
        let l = json!({"items": [{"id": "a", "state": "routed"}, {"id": "b", "state": "proposed"}, {"id": "c", "state": "verified"}]});
        assert_eq!(shown(&l, true).iter().map(|i| s(i, "id").to_string()).collect::<Vec<_>>(), ["b"]); assert_eq!(shown(&l, false).len(), 3);
        assert_eq!(hex("#e8a33d"), 0xe8a33d);
        assert!(super::super::mgmt::VIEWS.iter().any(|v| v.id == "plan"), "the host draws the Plan panel");
        let b = |v: &str| crate::viewers::boards::Board { label: String::new(), value: v.into(), on: false, default: false };
        assert_eq!(board_value(&[b("Nodes/A/A.kicad_pcb"), b("Nodes/A_new/A_new.kicad_pcb")], "/p/Nodes/A_new/A_new.kicad_pcb", "Nodes/A_new/A_new.kicad_pcb"), "Nodes/A_new/A_new.kicad_pcb");
        assert_eq!(board_value(&[], "/p/x/B.kicad_pcb", "x/B.kicad_pcb"), "x/B.kicad_pcb", "the menu not read yet: the hub's path from the project folder");
    }
    #[test] fn status_badge_and_progress_segments_follow_plan_counts() {
        assert_eq!(status_badge(&json!({"status": "blocked"})).1, "bad");
        assert_eq!(status_badge(&json!({"status": "verified"})).1, "ok");
        assert_eq!(status_badge(&json!({"status": "awaiting"})).1, "warn");
        let segs = progress_segments(&json!({"total": 6, "done": 2, "approved": 1, "awaiting": 2, "blocked": 1, "routing": false}));
        assert_eq!(segs, [(2, "ok"), (0, "accent"), (1, "input"), (2, "warn"), (1, "bad")]);
        let icons: serde_json::Map<String, Value> = serde_json::from_str(include_str!("../../../../assets/icons.json")).unwrap();
        for name in ["layers", "chevron-down", "chevron-right"] { assert!(icons.contains_key(name), "missing icon {name}"); }
    }
    #[test] fn split_mode_list_layers_follow_visible_canvas_and_titles_canonical() {
        let all = vec![
            json!({"layer": "F.Cu", "title": "L1 · TOP"}),
            json!({"layer": "In1.Cu", "title": "L2 · GND1"}),
            json!({"layer": "In5.Cu", "title": "L6 · SIG1"}),
        ];
        let shown = vec!["In5.Cu".to_string()];
        let split_one = list_layers_for_view(ViewMode::Split, &all, &shown);
        assert_eq!(split_one.len(), 1);
        assert_eq!(split_one[0]["layer"], "In5.Cu");
        assert!(list_layers_for_view(ViewMode::Split, &all, &["In2.Cu".to_string()]).is_empty());
        assert_eq!(list_layers_for_view(ViewMode::List, &all, &shown).len(), 3);
        assert_eq!(mgmt_plan_canvas::layer_display_title("In5.Cu", &[], &all), "L6 · SIG1");
        assert_eq!(mgmt_plan_canvas::layer_display_title("In9.Cu", &[], &all), "In9.Cu");
    }
}

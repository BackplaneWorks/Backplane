//! Management panels beside the design (the hub's kicad/mgmt.rs; docs/MANAGEMENT.md): a slim tab strip along the bottom of the viewer pane
//! (the embedded KiCad or Backplane's own viewers), and the open panel at the pane's right side or bottom. Which panel is open, its side and size
//! are the hub's per project (info `mgmt.state.<digest>`); a press shows at once and the hub's state then confirms it. Each panel's data is the hub's
//! info `mgmt.<panel>.<digest>` (with `badge: {n, tone}` drawn on its tab), so the phones draw the same.
//!
//! A panel's client side is one `View` in `VIEWS`: its id (the hub panel's) and `body`, which draws the panel from its data.
use crate::{hub::Command, i18n::trs, screens::settings::Secret, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{collections::HashMap, time::{Duration, Instant}};

/// A panel's client side: drawn from the hub's `mgmt.<id>.<digest>` value (`data`; Null until it arrives).
pub struct View { pub id: &'static str, pub body: fn(&mut Backplane, &Value, bool, &mut Context<Backplane>) -> AnyElement }
pub const VIEWS: &[View] = &[View { id: "bom", body: super::mgmt_bom::body }, View { id: "fab", body: super::mgmt_fab::body }, View { id: "checks", body: super::mgmt_checks::body }, View { id: "life", body: super::mgmt_life::body }, View { id: "plan", body: super::mgmt_plan::body }];

/// The `mgmt.watch` answer's tag (Command::Ask).
pub const WATCH_TAG: u64 = 0x6d67_6d74;
/// The strip's height, the drag handle's thickness, and the room the design keeps beside an open panel.
pub const STRIP_H: f32 = 24.0;
pub const HANDLE: f32 = 5.0;
pub const KEEP: f32 = super::viewer::VIEWER_LEAST;
pub const MIN: f32 = 160.0;

#[derive(Default)]
pub struct Mgmt {
    // The screenshot driver reads the state of the panel Review actually rendered,
    // including filtered lists. A subsequent render of the dock has its own state.
    pub review_snapshot: Option<(&'static str, String)>,
    panel_placeholder: Option<(&'static str, String)>,
    /// every `mgmt.*` info the hub published, by key (JSON values)
    pub infos: HashMap<String, Value>,
    /// the project shown and its digest (from `mgmt.watch`'s answer); the project last asked about
    pub project: String, pub digest: String, asked: String,
    /// the state a press asked for, until the hub's says the same (at most 2 s)
    pending: Option<(Value, Instant)>,
    /// a drag of the panel's edge: where it started (pointer px, size), the size drawn now, and a counter so only the last move saves
    drag: Option<(f32, f32)>, live: Option<f32>, drag_n: u64,
    /// an error the hub gave for a panel's request
    pub error: String,
    /// the open panel's width as last drawn (the BOM's columns fit it)
    pub panel_w: f32,
    pub checks: super::mgmt_checks::State,
    pub bom: super::mgmt_bom::Ui,
    pub life: super::mgmt_life::Ui,
    /// a plan review link pressed (screens/linkroute.rs `Route::Plan`): its board (from the project folder), when, and whether that board was picked
    pub plan_next: Option<(String, Instant, bool)>,
}
/// What a pressed plan review link does now, on the page drawn: open the Plan panel (the board shown is the plan's, or its board could not be shown
/// within a few seconds: the panel then names the board whose plan waits), show its board first, or wait for the page.
#[derive(Debug, PartialEq)]
pub enum PlanStep { Open, Pick(String), Wait }
pub fn plan_step(rel: &str, project: &str, boards: &[(String, bool)], picked: bool, waited: Duration, loaded: bool) -> PlanStep {
    let pro = format!("{}.kicad_pro", rel.strip_suffix(".kicad_pcb").unwrap_or(rel));
    if !loaded { return if waited > Duration::from_secs(8) { PlanStep::Open } else { PlanStep::Wait } }
    if project.ends_with(&format!("/{pro}")) || project == pro { return PlanStep::Open }
    if !picked { if let Some((v, _)) = boards.iter().find(|(v, on)| v == rel && !on) { return PlanStep::Pick(v.clone()) } }
    if waited > Duration::from_secs(6) || (!picked && !boards.iter().any(|(v, _)| v == rel)) { PlanStep::Open } else { PlanStep::Wait }
}

/// The pages a panel sits beside: the design's (board, schematic, 3D, split; the embedded editor is on these too).
pub fn shows(open: &str) -> bool { matches!(open, "board" | "schematic" | "3d" | "split") }
/// The project file the panels are for: the design the viewer shows, None while the pane shows something else (or a diff is over it).
pub fn project_for(thread: &Value) -> Option<String> {
    let v = &thread["viewer"];
    if thread["diff"].is_object() || !shows(v["open"].as_str().unwrap_or("")) || !super::viewer::has_design(v) { return None }
    if let Some(pro) = v["designPanels"]["bom"]["project"].as_str() { return Some(pro.to_string()) }
    crate::viewers::kicad_pane::project_of(&super::viewer::shown_key(v)).filter(|p| !p.is_empty())
}
/// The state a press on a tab asks for: that panel, or none when it is the one open (the hub's mgmt::apply does the same).
pub fn toggled(st: &Value, id: &str) -> Value { let mut s = st.clone(); s["open"] = json!(if st["open"] == id { "" } else { id }); s }
/// Whether the hub's state has caught up with a press (the open panel, side and size it asked for).
pub fn caught_up(hub: &Value, want: &Value) -> bool { hub["open"] == want["open"] && hub["side"] == want["side"] && hub["size"] == want["size"] }
/// The room an open panel at the design's side asks of the pane (0: none, shut, or docked at the bottom): the pane grows by it at the thread's
/// cost (screens/viewer.rs `with_panel`), so opening BOM, Checks or Fab does not squeeze the board to a strip.
pub fn side_room(thread: &Value, m: &Mgmt) -> f32 {
    if project_for(thread).is_none() { return 0. }
    let st = m.peek();
    if st["panels"].as_array().map_or(true, |p| p.is_empty()) || st["open"].as_str().map_or(true, str::is_empty) || st["side"] == "bottom" { return 0. }
    m.live.unwrap_or(st["size"].as_f64().unwrap_or(400.) as f32).max(MIN)
}
/// The panel's drawn size: its size, kept within the room the pane leaves after the design's least.
pub fn fit(size: f32, avail: f32) -> f32 { size.min(avail - KEEP).max(MIN.min(avail * 0.5)).round() }
/// A drag of the edge: the new size from where it started. The panel grows as the edge moves away from the pane's right (or bottom) edge.
pub fn dragged(start_pos: f32, start_size: f32, pos: f32) -> f32 { (start_size + (start_pos - pos)).max(MIN) }
/// A tab's badge text and tone: "3" in warn or bad, none at 0.
pub fn badge(data: &Value) -> Option<(String, &'static str)> {
    let n = data["badge"]["n"].as_u64().unwrap_or(0);
    (n > 0).then(|| (if n > 99 { "99+".to_string() } else { n.to_string() }, if data["badge"]["tone"] == "bad" { "bad" } else { "warn" }))
}

impl Mgmt {
    pub fn info(&mut self, key: &str, text: &str) -> bool {
        if text.is_empty() { return self.infos.remove(key).is_some() }
        let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        if self.infos.get(key) == Some(&v) { return false }
        self.infos.insert(key.to_string(), v); true
    }
    /// The hub's state for the shown project (Null until it answered).
    pub fn hub_state(&self) -> Value { self.infos.get(&format!("mgmt.state.{}", self.digest)).cloned().unwrap_or(Value::Null) }
    /// The state drawn: what a press asked for while the hub has not caught up, else the hub's.
    pub fn peek(&self) -> Value {
        let hub = self.hub_state();
        match &self.pending { Some((want, at)) if !caught_up(&hub, want) && at.elapsed() < Duration::from_secs(2) => want.clone(), _ => hub }
    }
    pub fn state(&mut self) -> Value {
        let hub = self.hub_state();
        if let Some((want, at)) = &self.pending { if !caught_up(&hub, want) && at.elapsed() < Duration::from_secs(2) { return want.clone() } }
        self.pending = None; hub
    }
    pub fn data(&self, id: &str) -> Value { self.infos.get(&format!("mgmt.{id}.{}", self.digest)).cloned().unwrap_or(Value::Null) }
}

/// The shown project changed: ask the hub (its state, and its panels start publishing for it). Again for a new project only.
fn want(app: &mut Backplane, project: &str) {
    if app.screens.mgmt.asked == project { return }
    app.screens.mgmt.asked = project.to_string();
    if app.screens.mgmt.project != project { app.screens.mgmt.digest.clear(); app.screens.mgmt.bom = Default::default(); app.screens.mgmt.checks.reset(); app.screens.mgmt.error.clear(); }
    let _ = app.tx.try_send(Command::Ask("mgmt.watch".into(), json!({"project": project}), WATCH_TAG));
}
/// `mgmt.watch`'s answer: the project's digest (what its info keys end in).
pub fn answered(app: &mut Backplane, id: u64, reply: &Value, cx: &mut Context<Backplane>) -> bool {
    if id == super::mgmt_bom::EDIT_TAG { super::mgmt_bom::edited(app, reply, cx); return true }
    if id == super::mgmt_life::PART_TAG { super::mgmt_life::answered(app, reply, cx); return true }
    if id == super::mgmt_bom::ALTS_TAG { super::mgmt_bom::alts_answered(app, reply, cx); return true }
    if id != WATCH_TAG { return false }
    let r = if reply["result"].is_object() { &reply["result"] } else { reply };
    if reply["ok"] == false { app.screens.mgmt.error = crate::hub::rpc_error(reply); }
    if let Some(d) = r["digest"].as_str() { app.screens.mgmt.digest = d.to_string(); app.screens.mgmt.project = r["project"].as_str().unwrap_or(&app.screens.mgmt.asked).to_string(); }
    if let (Some(d), true) = (r["digest"].as_str(), r["state"].is_object()) { app.screens.mgmt.infos.entry(format!("mgmt.state.{d}")).or_insert(r["state"].clone()); }
    if let (Some(d), Some(data)) = (r["digest"].as_str(), r["data"].as_object()) { for (id, v) in data { app.screens.mgmt.infos.entry(format!("mgmt.{id}.{d}")).or_insert(v.clone()); } }
    cx.notify(); true
}
/// A change of the panel state, sent to the hub and drawn at once.
pub fn set(app: &mut Backplane, want: Value, patch: Value, cx: &mut Context<Backplane>) {
    let m = &mut app.screens.mgmt;
    if m.project.is_empty() { return }
    m.pending = Some((want, Instant::now()));
    let mut p = patch; p["project"] = json!(m.project.clone());
    let _ = app.tx.try_send(Command::Rpc("mgmt.panel".into(), Secret(p), "mgmt"));
    app.ui_epoch += 1; cx.notify();
}
/// Escape in Backplane: a panel open beside the design shuts first (the viewer, and the editor in it, only on the next Escape: a person
/// pressing Escape twice to leave a KiCad tool lost the whole viewer when the keyboard was on the panel). Whether it did.
pub fn escape(app: &mut Backplane, cx: &mut Context<Backplane>) -> bool {
    if project_for(&app.data["thread"]).is_none() { return false }
    let st = app.screens.mgmt.state();
    let Some(open) = st["open"].as_str().filter(|o| !o.is_empty()).map(str::to_string) else { return false };
    toggle(app, &open, cx); true
}
/// A tab pressed (or its key): opens that panel, or shuts it.
pub fn toggle(app: &mut Backplane, id: &str, cx: &mut Context<Backplane>) {
    let st = app.screens.mgmt.state();
    let known = st["panels"].as_array().map_or(false, |ps| ps.iter().any(|p| p["id"] == id));
    eprintln!("GPUI_LOG {}", json!({"mgmt_toggle": id, "known": known, "open": st["open"]}));
    if !known { return }
    set(app, toggled(&st, id), json!({"toggle": id}), cx);
}
pub fn dock(app: &mut Backplane, side: &str, cx: &mut Context<Backplane>) {
    let mut st = app.screens.mgmt.state(); if st.is_null() { return }
    st["side"] = json!(side); st["size"] = json!(if side == "bottom" { 260 } else { 400 });
    set(app, st, json!({"side": side}), cx);
}
/// The e2e harness's controls: a tab per panel, and dock/close while one is open.
pub fn controls(thread: &Value, m: &Mgmt) -> Vec<(String, String, String)> {
    if project_for(thread).is_none() { return vec![] }
    let st = m.peek();
    let mut v: Vec<(String, String, String)> = st["panels"].as_array().into_iter().flatten().map(|p| (format!("mgmt:{}", p["id"].as_str().unwrap_or("")), p["label"].as_str().unwrap_or("").to_string(), format!("toggle:{}", p["id"].as_str().unwrap_or("")))).collect();
    if st["open"].as_str().map_or(false, |o| !o.is_empty()) { /* i18n-ok: ctl ids and labels the drivers press */ v.extend([("mgmt:right".into(), "Right".into(), "dock:right".into()), ("mgmt:bottom".into(), "Bottom".into(), "dock:bottom".into())]); }
    v.extend(super::mgmt_bom::controls(m));
    v.extend(super::mgmt_life::controls(m));
    v.extend(super::mgmt_fab::controls(m));
    v.extend(super::mgmt_plan::controls(m));
    v.extend(super::mgmt_checks::controls(m));
    v
}
/// A press of one of `controls` (or a key): `toggle:<id>`, `dock:<side>`, or a panel's own (`bom:...`).
pub fn act(app: &mut Backplane, a: &str, cx: &mut Context<Backplane>) {
    if let Some(id) = a.strip_prefix("toggle:") { toggle(app, id, cx) }
    else if let Some(s) = a.strip_prefix("dock:") { dock(app, s, cx) }
    else if let Some(b) = a.strip_prefix("bom:") { super::mgmt_bom::act(app, b, cx) }
    else if let Some(b) = a.strip_prefix("fab:") { super::mgmt_fab::act(app, b, cx) }
    else if let Some(b) = a.strip_prefix("plan:") { super::mgmt_plan::act(app, b, cx) }
    else if let Some(b) = a.strip_prefix("checks:") { super::mgmt_checks::act(app, b, cx) }
    else if let Some(b) = a.strip_prefix("life:") { super::mgmt_life::act(app, b, cx) }
}

/// Review mounts the same element and borrows the same scroll and selection
/// state as the dock. A comparison has its own read-only report.
pub fn review_panel(app: &mut Backplane, cx: &mut Context<Backplane>) -> Option<AnyElement> {
    app.screens.mgmt.review_snapshot = None;
    let viewer = &app.data["thread"]["viewer"];
    if viewer["open"] != "review" || viewer["review"]["empty"].is_object() { return None }
    let id = viewer["review"]["tabs"].as_array()?.iter().find(|tab| tab["on"] == true)?["page"].as_str()?.to_owned();
    let scope = viewer["designPanels"].get(&id)?.clone();
    if !scope["versions"].is_array() { want(app, scope["project"].as_str()?) }
    app.screens.mgmt.panel_w = super::viewer::pane_width() as f32;
    app.screens.mgmt.panel_placeholder = None;
    let panel = shared_panel(app, &id, &scope, PanelLocation::Review, app.light(), cx);
    if crate::art::rects_on() {
        app.screens.mgmt.review_snapshot = Some(app.screens.mgmt.panel_placeholder.take().unwrap_or(("ready", String::new())));
    }
    Some(panel)
}

pub(super) fn panel_state(app: &mut Backplane, id: &str, light: bool, line: String, loading: bool) -> AnyElement {
    use super::panestate::{self, PaneState};
    if crate::art::rects_on() {
        app.screens.mgmt.panel_placeholder = Some((if loading { "loading" } else { "empty" }, line.clone()));
    }
    let state = if loading {
        let secs = app.screens.plots[0].loading.track_now(&format!("panel:{id}:{}", app.screens.mgmt.digest));
        PaneState::loading(panestate::loading_line(&line, secs, 0))
    } else { PaneState::empty(line, String::new()) };
    panestate::view(light, &state, None)
}

enum PanelLocation { Review, Dock { bottom: bool } }

fn shared_panel(app: &mut Backplane, id: &str, scope: &Value, location: PanelLocation, light: bool, cx: &mut Context<Backplane>) -> AnyElement {
    use crate::i18n::tf;
    let working = tf("history.workingcopy", &[]);
    let historical = scope["versions"].is_array();
    let key = scope["title"].as_str().unwrap_or("");
    let title = if matches!(location, PanelLocation::Review) {
        tf(if id == "bom" { "review.bom" } else { "review.checks" }, &[])
    } else if historical {
        let label = |n: usize| scope["versions"][n].as_str().filter(|s| !s.is_empty()).unwrap_or(&working).to_owned();
        tf(key, &[("old", label(0).into()), ("new", label(1).into())])
    } else if id == "bom" { tf(key, &[]) }
    else { tf(key, &[("version", working.into())]) };
    let method = scope["refresh"].as_str().unwrap_or("").to_owned();
    let panel = id.to_owned();
    let refresh = div().id("design-panel-refresh").px_2().cursor_pointer().child(trs("Refresh"))
        .on_click(cx.listener(move |app, _, _, _| {
            if method == "rv-run" {
                let _ = app.tx.try_send(Command::Action(method.clone(), String::new()));
            } else if !app.screens.mgmt.digest.is_empty() {
                let args = json!({"project":app.screens.mgmt.project, "panel":panel});
                let _ = app.tx.try_send(Command::Rpc(method.clone(), Secret(args), "mgmt"));
            }
        }));
    let head = div().flex_shrink_0().flex().items_center().px_2().py_1().text_size(crate::scale::tpx(12.))
        .child(div().flex_1().font_weight(FontWeight::BOLD).child(title)).child(refresh);
    let body = if historical {
        let review = &app.data["thread"]["viewer"]["review"];
        let nodes = review["blocks"].as_array().map(Vec::as_slice).unwrap_or_default();
        if review["busy"] == true || nodes.is_empty() {
            let busy = review["busy"] == true;
            let line = trs(review["note"].as_str().unwrap_or(""));
            panel_state(app, "comparison-bom", light, line, busy)
        } else {
            let parsed = super::md::parse(nodes);
            div().id("comparison-bom").p_3().flex_1().min_h_0().overflow_y_scroll()
                .children(super::timeline::blocks(cx, &app.screens.reg, light, &parsed, "comparison-bom")).into_any_element()
        }
    } else {
        let data = app.screens.mgmt.data(id);
        if id == "checks" { super::mgmt_checks::body(app, &data, light, cx) }
        else { super::mgmt_bom::panel(app, &data, matches!(location, PanelLocation::Dock { bottom: true }), light, cx) }
    };
    div().flex_1().min_h_0().flex().flex_col().overflow_hidden().child(head).when(!app.screens.mgmt.error.is_empty(), |d| d.child(div().text_color(theme::color(light, "bad")).child(app.screens.mgmt.error.clone()))).child(body).into_any_element()
}

struct EdgeDrag;
struct Ghost;
impl Render for Ghost { fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement { div() } }

/// The pane's page with the panels: the strip along the bottom, the open panel at the right or the bottom. `page` is returned as it is
/// when the pane shows no design or the hub has no panels.
pub fn host(app: &mut Backplane, page: AnyElement, window: &mut Window, cx: &mut Context<Backplane>) -> AnyElement {
    let Some(project) = project_for(&app.data["thread"]) else { return page };
    want(app, &project);
    // a plan review link pressed: its board, then the Plan panel (`plan_step`)
    if let Some((rel, at, picked)) = app.screens.mgmt.plan_next.clone() {
        let st = app.screens.mgmt.state();
        let loaded = st["panels"].as_array().map_or(false, |p| p.iter().any(|x| x["id"] == "plan"));
        let boards: Vec<(String, bool)> = crate::viewers::boards::list(&app.data["thread"]["viewer"]).into_iter().map(|b| (b.value, b.on)).collect();
        match plan_step(&rel, &project, &boards, picked, at.elapsed(), loaded) {
            PlanStep::Open => { app.screens.mgmt.plan_next = None; if loaded && st["open"] != "plan" { set(app, toggled(&st, "plan"), json!({"open": "plan"}), cx); } }
            PlanStep::Pick(v) => { app.screens.mgmt.plan_next = Some((rel, at, true)); crate::viewers::boards::pick(app, v, cx); }
            // looked at again in a second (the board's page or list may come with no other change to draw); a few seconds at most
            PlanStep::Wait => { cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_secs(1)).await; let _ = this.update(cx, |_, cx| cx.notify()); }).detach(); }
        }
    }
    let st = app.screens.mgmt.state();
    let panels: Vec<(String, String)> = st["panels"].as_array().into_iter().flatten().map(|p| (p["id"].as_str().unwrap_or("").to_string(), p["label"].as_str().unwrap_or("").to_string())).collect();
    if panels.is_empty() { return page }
    let light = app.light();
    let c = |n: &str| theme::color(light, n);
    let open = st["open"].as_str().unwrap_or("").to_string();
    let bottom = st["side"] == "bottom";
    // tabs: the panel's name and its badge (the hub's count of what needs doing); the open one underlined
    let mut strip = div().id("mgmt-strip").flex_shrink_0().h(crate::scale::tpx(STRIP_H)).flex().items_center().gap(px(2.)).px_2().bg(c("panel")).border_t_1().border_color(c("rule")).text_size(crate::scale::tpx(11.));
    for (id, label) in &panels {
        let on = *id == open; let data = app.screens.mgmt.data(id); let idc = id.clone();
        let tab = crate::ui::with_rect(div().id(SharedString::from(format!("mgmt-tab-{id}"))), { let id = id.clone(); move || format!("mgmt:{id}") })
            .h_full().px_2().flex().items_center().gap_1().cursor_pointer().text_color(c(if on { "text" } else { "dim" })).hover(|s| s.bg(c("raise")))
            .when(on, |d| d.border_t_2().border_color(c("accent")))
            .child(trs(label))
            .children(badge(&data).map(|(n, tone)| div().px(px(4.)).h(px(14.)).flex().items_center().text_size(crate::scale::tpx(10.)).text_color(c(tone)).border_1().border_color(c(tone)).child(n)))
            // on the press, as the editor's own tabs beside it (kicad_pane.rs strip): the panel is there as the button goes down, not a
            // click's length later (~50-100 ms of a person's click, all of the 100-145 ms real_use read for a panel opened by its tab)
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| { cx.stop_propagation(); toggle(this, &idc, cx) }));
        strip = strip.child(tab);
    }
    strip = strip.child(div().flex_1());
    if !app.screens.mgmt.error.is_empty() { strip = strip.child(div().max_w(px(260.)).overflow_hidden().whitespace_nowrap().text_ellipsis().text_color(c("bad")).child(app.screens.mgmt.error.clone())); }
    if !open.is_empty() {
        let (icon, side, tip) = if bottom { ("panel-right", "right", trs("Dock at the right")) } else { ("chevron-down", "bottom", trs("Dock at the bottom")) };
        strip = strip.child(crate::ui::with_rect(div().id("mgmt-dock"), move || format!("mgmt:{side}")).size(px(20.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c("raise"))).tooltip(crate::keys::tip(&tip, "", light))
            .child(crate::art::icon(icon, 13., c("dim"))).on_click(cx.listener(move |this, _, _, cx| dock(this, side, cx))))
            .child(div().id("mgmt-close").size(px(20.)).flex().items_center().justify_center().cursor_pointer().hover(|s| s.bg(c("raise"))).tooltip(crate::keys::tip(&trs("Close the panel"), "", light))
                .child(crate::art::icon("x", 13., c("dim"))).on_click({ let o = open.clone(); cx.listener(move |this, _, _, cx| toggle(this, &o, cx)) }));
    }
    let page = div().flex_1().min_w_0().min_h_0().relative().overflow_hidden().child(page);
    if open.is_empty() { return div().size_full().flex().flex_col().child(page).child(strip).into_any_element() }
    // the panel: its view drawn from the hub's data, or nothing yet
    let data = app.screens.mgmt.data(&open);
    let vp = window.viewport_size();
    let asked = app.screens.mgmt.live.unwrap_or(st["size"].as_f64().unwrap_or(400.) as f32);
    // at the side the pane has grown for the panel (viewer::pane_px, the same sum the pane's frame is made with)
    let avail = if bottom { f32::from(vp.height) * 0.8 } else { super::viewer::pane_px(app.data["thread"]["viewer"]["open"].as_str().unwrap_or(""), asked.max(MIN)) };
    let size = fit(asked, avail);
    app.screens.mgmt.panel_w = if bottom { super::viewer::pane_width() as f32 } else { size };
    let scope = app.data["thread"]["viewer"]["designPanels"][&open].clone();
    let body = if scope.is_object() { shared_panel(app, &open, &scope, PanelLocation::Dock { bottom }, light, cx) } else { match VIEWS.iter().find(|v| v.id == open) { Some(v) => (v.body)(app, &data, light, cx), None => div().into_any_element() } };
    let handle = div().id("mgmt-edge").flex_shrink_0().bg(c("rule")).hover(|s| s.bg(c("accent")))
        .map(|d| if bottom { d.h(px(1.)).w_full().cursor_row_resize() } else { d.w(px(1.)).h_full().cursor_col_resize() })
        .child(div().absolute().map(|d| if bottom { d.left_0().right_0().top(px(-2.)).h(px(HANDLE)) } else { d.top_0().bottom_0().left(px(-2.)).w(px(HANDLE)) }))
        .relative()
        .on_drag(EdgeDrag, |_, _, _, cx| cx.new(|_| Ghost))
        .on_drag_move(cx.listener(move |this, e: &DragMoveEvent<EdgeDrag>, _, cx| {
            let pos = f32::from(if bottom { e.event.position.y } else { e.event.position.x });
            let m = &mut this.screens.mgmt;
            let (p0, s0) = *m.drag.get_or_insert((pos, size));
            let now = dragged(p0, s0, pos); m.live = Some(now); m.drag_n += 1; let n = m.drag_n;
            cx.notify();
            // the size is saved once the edge has rested for a moment (a drag's end), not at every move
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_millis(350)).await;
                let _ = this.update(cx, |this, cx| {
                    if this.screens.mgmt.drag_n != n { return }
                    let m = &mut this.screens.mgmt; m.drag = None; let Some(z) = m.live.take() else { return };
                    let mut st = m.state(); st["size"] = json!(z.round() as u64);
                    set(this, st, json!({"size": z.round() as u64}), cx);
                });
            }).detach();
        }));
    let panel = div().id("mgmt-panel").flex_shrink_0().bg(c("bg")).overflow_hidden().flex().flex_col().map(|d| if bottom { d.h(px(size)).w_full() } else { d.w(px(size)).h_full() }).child(body);
    let area = if bottom { div().flex_1().min_h_0().flex().flex_col().child(page).child(handle).child(panel) } else { div().flex_1().min_h_0().flex().flex_row().child(page).child(handle).child(panel) };
    div().size_full().flex().flex_col().child(area).child(strip).into_any_element()
}

#[cfg(test)]
mod tests {
    #[test] fn a_plan_review_link_shows_its_board_then_opens_the_plan() {
        use std::time::Duration as D;
        let b = |v: &str, on: bool| (v.to_string(), on);
        let boards = vec![b("A/A.kicad_pcb", true), b("N/B.kicad_pcb", false)];
        assert_eq!(super::plan_step("N/B.kicad_pcb", "/p/A/A.kicad_pro", &boards, false, D::from_millis(10), true), super::PlanStep::Pick("N/B.kicad_pcb".into()));
        assert_eq!(super::plan_step("N/B.kicad_pcb", "/p/A/A.kicad_pro", &boards, true, D::from_secs(1), true), super::PlanStep::Wait, "picked: wait for its page");
        assert_eq!(super::plan_step("N/B.kicad_pcb", "/p/N/B.kicad_pro", &boards, true, D::from_secs(1), true), super::PlanStep::Open);
        assert_eq!(super::plan_step("N/B.kicad_pcb", "/p/A/A.kicad_pro", &boards, true, D::from_secs(7), true), super::PlanStep::Open, "never longer than a few seconds");
        assert_eq!(super::plan_step("X/X.kicad_pcb", "/p/A/A.kicad_pro", &boards, false, D::from_millis(10), true), super::PlanStep::Open, "a board not listed: the Plan panel here (it names the board)");
        assert_eq!(super::plan_step("A/A.kicad_pcb", "/p/A/A.kicad_pro", &[], false, D::from_millis(10), false), super::PlanStep::Wait, "the panels not known yet");
    }
    use super::*;
    use core::prelude::v1::test;
    #[test] fn panels_sit_beside_a_design_only() {
        let t = |open: &str, key: &str| json!({"viewer": {"open": open, "key": key, "choices": [{"label": "Board", "value": "board"}]}});
        assert_eq!(project_for(&t("board", "0|/p|x.kicad_pcb")).as_deref(), Some("/p/x.kicad_pcb"));
        assert_eq!(project_for(&t("files", "0|/p|x.kicad_pcb")), None);
        let mut d = t("board", "0|/p|x.kicad_pcb"); d["diff"] = json!({}); assert_eq!(project_for(&d), None);
        assert_eq!(project_for(&json!({"viewer": {"open": "board", "key": "0|/p|x.kicad_pcb", "choices": []}})), None, "no design among the tabs");
    }
    #[test] fn a_press_shows_at_once_until_the_hub_agrees() {
        let hub = json!({"open": "", "side": "right", "size": 400});
        let want = toggled(&hub, "bom"); assert_eq!(want["open"], "bom"); assert_eq!(toggled(&want, "bom")["open"], "");
        assert!(!caught_up(&hub, &want)); assert!(caught_up(&want, &want));
        let mut m = Mgmt::default(); m.digest = "d".into(); m.info("mgmt.state.d", &hub.to_string());
        m.pending = Some((want.clone(), Instant::now())); assert_eq!(m.state()["open"], "bom");
        m.info("mgmt.state.d", &want.to_string()); assert_eq!(m.state()["open"], "bom"); assert!(m.pending.is_none());
        m.pending = Some((toggled(&want, "life"), Instant::now() - Duration::from_secs(3))); assert_eq!(m.state()["open"], "bom", "a press the hub never answered gives way");
    }
    #[test] fn sizes_keep_room_for_the_design() {
        assert_eq!(fit(400., 1000.), 400.); assert_eq!(fit(900., 1000.), 680.); assert_eq!(fit(50., 1000.), 160.); assert_eq!(fit(400., 200.), 100.);
        assert_eq!(dragged(800., 400., 700.), 500.); assert_eq!(dragged(800., 400., 1200.), MIN);
    }
    #[test] fn a_side_panel_asks_the_pane_for_its_room() {
        let t = json!({"viewer": {"open": "board", "key": "0|/p|x.kicad_pcb", "choices": [{"label": "Board", "value": "board"}]}});
        let st = |open: &str, side: &str| json!({"open": open, "side": side, "size": 420, "panels": [{"id": "bom", "label": "BOM"}]});
        let mut m = Mgmt::default(); m.digest = "d".into();
        assert_eq!(side_room(&t, &m), 0., "the hub has not answered");
        m.info("mgmt.state.d", &st("", "right").to_string()); assert_eq!(side_room(&t, &m), 0., "shut");
        m.info("mgmt.state.d", &st("bom", "right").to_string()); assert_eq!(side_room(&t, &m), 420.);
        m.live = Some(500.); assert_eq!(side_room(&t, &m), 500., "its edge under a drag"); m.live = None;
        m.info("mgmt.state.d", &st("bom", "bottom").to_string()); assert_eq!(side_room(&t, &m), 0., "at the bottom it takes no width");
        m.info("mgmt.state.d", &st("bom", "right").to_string()); assert_eq!(side_room(&json!({"viewer": {"open": "files"}}), &m), 0., "no design on show");
    }
    #[test] fn badges() {
        assert_eq!(badge(&json!({"badge": {"n": 3, "tone": "warn"}})), Some(("3".into(), "warn"))); assert_eq!(badge(&json!({"badge": {"n": 0}})), None);
        assert_eq!(badge(&json!({"badge": {"n": 120, "tone": "bad"}})), Some(("99+".into(), "bad"))); assert_eq!(badge(&Value::Null), None);
    }
}

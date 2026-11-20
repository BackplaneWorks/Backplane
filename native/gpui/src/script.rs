//! Metrics contract and scripted input. See README.md.
use crate::{Backplane, screens::pane_header::Press as HeaderPress};
use std::{cell::RefCell, collections::BTreeMap};
use gpui::*;
use serde_json::{json, Value};
use std::{collections::VecDeque, path::Path, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

pub fn metric(started: Instant, event: &str, mut fields: Value) {
    fields["event"] = json!(event);
    fields["t_ms"] = json!((started.elapsed().as_secs_f64() * 1000.0 * 1000.0).round() / 1000.0);
    fields["epoch_ms"] = json!(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64));
    eprintln!("GPUI_METRIC {}", fields);
}

#[derive(Clone, Debug)]
/* i18n-ok: scripted e2e input and its metrics, not drawn */ pub enum Step { PairFill(String, String, String), /** keystrokes typed as the person would ("ctrl-j e c h o enter"), through the window's bindings */ Keys(String), Cef(Value), Browser(Value), /** {"kicad":{"report":true}}: the embedded editor's report */ Kicad(Value), Files(String), /** {"mgmt":"bom:set:mpn=X"}: a management panel's press or edit (screens/mgmt.rs `act`) */ Mgmt(String), ScrollState(String), Home(usize), Select(usize), Tap(f32, f32, usize), Sheets(bool), Compare(Option<usize>), Hist(Option<i64>), HistMode(u8), HistPlay, ViewShow(String, String), Popup(String), Type(String), Send, Attach(String), Action(String, String), Field(String, String), Wait(u64), Record(Option<String>), Click(String), /** {"press":id}: the click, not waiting for the hub to answer it (a person watching the window meanwhile: the tab-loading e2e) */ Press(String), /** {"click":id,"within":ms}: the click, retried every 250 ms until the control exists or ms runs out (a view still drawing) */ ClickWithin(String, u64), Imp(String), Read(String), View(String), Layers(bool, Option<f32>, f32), Expect(String), Pan(f32, f32), Zoom(f32, f32, f32), Scroll(f32), NetSleep(f64), Mark(String), /** {"shot":"<name>","settle":"<kind>"?,"timeout_ms":N?}: wait until settled (if requested) and painted, emit shot-ready, wait for <BACKPLANE_GPUI_SHOT_DIR>/<name>.done */ Shot { name: String, settle: Option<String>, timeout_ms: u64 }, /** {"wait_settled":"<kind>","tag":"<tag>","timeout_ms":N?}: wait until the page settles and emit a `settled` metric with its viewer state */ WaitSettled { kind: String, tag: String, timeout_ms: u64 }, Fit, Unsupported(String) }
/// What a click on a registered control does; the e2e harness's controls inventory and `click` step use these.
#[derive(Clone, Debug)]
pub enum Ctl { Header(String), /** the composer chip's account menu, and Settings' Accounts page */ AccountMenu, AccountsPage, /** the sidebar hub picker */ Hub(crate::screens::hubpick::Act), PairGo, PairYes, PairCancel, None, Pad, Dictate, OpenPalette, ToggleTerminal, Rail(String), PickFiles, /** the ▾ beside the thread's name: its menu */ ThreadMenu, ToggleSidebar, ToggleContext, Select(String), Home(String), Fold(String), Send, Skill(String), Page(String), /** a hub page of the Settings nav */ SettingsPage(String), Node(String, String), RenameSave(String), Imp(String), FocusSearch, /** a bots-page press (screens::bots::press): its kind and value */ Bot(String, String), /** a chat terminal's card: focus, expand, pop, stop; its key */ Term(String, String), /** a subagent's own view (its key; "" goes back) */ Sub(String), /** a card's button that sends the hub an action (a halt card's Resume...): `Command::Action` */ Go(String, String), /** the viewer's Edit (true: the real KiCad in the pane) and View (false) */ KicadEdit(bool), /** the editor strip: a tab (its kind), full, popout, popin */ KicadAct(String), /** a management panel's press (screens/mgmt.rs `act`) */ Mgmt(String) }
#[derive(Clone, Debug)]
pub struct CtlEntry { pub id: String, pub label: String, pub ctl: Ctl }
#[derive(Clone, Debug)]
pub enum Goal { Frame, Changed(u64), Reconnected, Home, Thread(String), Popup, Typed(String), Sent(usize), Sleep, Shot { name: String, settle: Option<String>, dir: std::path::PathBuf, timeout_ms: u64, polling: bool }, WaitSettled { kind: String, tag: String, timeout_ms: u64, polling: bool } }
pub struct Pending { pub label: String, pub started: Instant, pub goal: Goal, pub armed: bool, pub seq: u64 }

thread_local! {
    /// SolidPage (key, painted) snapshot taken when a script press/click runs, so `viewer_settled` requires a newly painted frame for a new model.
    static PRESS_SOLID: std::cell::RefCell<(String, u64)> = const { std::cell::RefCell::new((String::new(), 0)) };
}

struct HeaderAction {
    label: String,
    folded: bool,
    rect_id: String,
    press: Option<HeaderPress>,
}

#[derive(Default)]
struct HeaderRegistry {
    present: bool,
    window: Option<AnyWindowHandle>,
    actions: BTreeMap<String, HeaderAction>,
}

thread_local! {
    static HEADER: RefCell<HeaderRegistry> = RefCell::default();
}

fn header_alias(id: &str) -> Option<&'static str> {
    // i18n-ok: stable script control IDs
    Some(match id {
        "kicad-edit" => "ui:Edit",
        "kicad-open" => "ui:Open in KiCad", // i18n-ok: stable script control IDs
        "altium-open" => "ui:Open in Altium", // i18n-ok: stable script control IDs
        "gerbers-vs-board" => "ui:vs board", // i18n-ok: stable script control IDs
        "gerbers-gerbview" => "ui:Open in GerbView", // i18n-ok: stable script control IDs
        "follow-agent" => "follow-agent",
        "follow-resume" => "follow-resume",
        _ => return None,
    })
}

pub(crate) fn header_present() {
    HEADER.with_borrow_mut(|registry| registry.present = true);
}

impl HeaderRegistry {
    fn replaces(&self, id: &str) -> bool {
        // i18n-ok: compatibility IDs shared with the older viewer toolbar.
        self.actions.contains_key(id) || self.present && matches!(id,
            "ui:Edit" | "ui:View" | "ui:Open in KiCad" | "ui:Open in Altium" | "ui:Open in GerbView") // i18n-ok: stable script control IDs
    }
}

/// Called by the measured header with the same action that its button runs.
pub(crate) fn header_action(page: &str, id: &str, label: &str, folded: bool, press: Option<HeaderPress>) {
    HEADER.with_borrow_mut(|registry| {
        if registry.window.is_none() { return; }
        let key = format!("pane-tool:{page}:{id}");
        let rect_id = if folded { format!("pane-more:{page}") } else { key.clone() };
        for key in header_alias(id).map(str::to_owned).into_iter().chain(std::iter::once(key)) {
            registry.actions.insert(key, HeaderAction {
                label: label.into(), folded, rect_id: rect_id.clone(), press: press.clone(),
            });
        }
    });
}

/// Shot marker names must be lowercase ASCII slugs (`[a-z0-9-]+`) so they are safe filenames in `BACKPLANE_GPUI_SHOT_DIR`.
pub fn valid_shot_name(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Steps: {"select":N} {"home":N} {"popup":"/"} {"type":"text"} {"send":true} {"wait":ms}; e2e hooks: {"record":"start","name":"x"} {"record":"stop"}
/// {"click":"<control id>"} {"scroll":px} {"net_sleep":secs}; {"open_viewer":"files"} {"files":"select:N|open:N|key:down|filter:text|new:name|rename:N:name|delete:N|confirm_delete|menu:N|upload:PATH|download:N|zip|copy:N|up|back|cancel"} {"files_download":N|"name"}; {"open_settings":true} opens Settings (a click on `ui:Settings`); {"paste_image"} answers ok:false, error:not_implemented (the e2e pastes a real clipboard with Ctrl+V instead).
pub fn load(path: &Path) -> VecDeque<Step> {
    let list: Vec<Value> = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    list.iter().filter_map(|v| {
        if let Some(a) = v["pair_fill"].as_array() { let s = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string(); Some(Step::PairFill(s(0), s(1), s(2))) }
        else if let Some(n) = v["home"].as_u64() { Some(Step::Home(n as usize)) }
        else if let Some(n) = v["select"].as_u64() { Some(Step::Select(n as usize)) }
        else if let Some(a) = v["viewshow"].as_array() { Some(Step::ViewShow(a.first().and_then(|x| x.as_str()).unwrap_or("").into(), a.get(1).and_then(|x| x.as_str()).unwrap_or("").into())) }
        else if v.get("hist").is_some() { Some(Step::Hist(v["hist"].as_i64())) }
        else if let Some(m) = v["histmode"].as_u64() { Some(Step::HistMode(m as u8)) }
        else if v["histplay"].as_bool() == Some(true) { Some(Step::HistPlay) }
        else if v.get("compare").is_some() { Some(Step::Compare(v["compare"].as_u64().map(|n| n as usize))) }
        else if let Some(on) = v["sheets"].as_bool() { Some(Step::Sheets(on)) }
        else if let Some(a) = v["tap"].as_array() { Some(Step::Tap(a.first().and_then(Value::as_f64).unwrap_or(0.5) as f32, a.get(1).and_then(Value::as_f64).unwrap_or(0.5) as f32, a.get(2).and_then(Value::as_u64).unwrap_or(0) as usize)) }
        else if let Some(t) = v["popup"].as_str() { Some(Step::Popup(t.into())) }
        else if let Some(t) = v["type"].as_str() { Some(Step::Type(t.into())) }
        else if let Some(t) = v["keys"].as_str() { Some(Step::Keys(t.into())) }
        else if v["send"].as_bool() == Some(true) { Some(Step::Send) }
        else if let Some(p) = v["attach"].as_str() { Some(Step::Attach(p.into())) }
        else if let Some(a) = v["field"].as_array() { Some(Step::Field(a.first().and_then(|x| x.as_str()).unwrap_or("").into(), a.get(1).and_then(|x| x.as_str()).unwrap_or("").into())) }
        else if let Some(a) = v["action"].as_array() { Some(Step::Action(a.first().and_then(|x| x.as_str()).unwrap_or("").into(), a.get(1).and_then(|x| x.as_str()).unwrap_or("").into())) }
        else if let Some(ms) = v["wait"].as_u64() { Some(Step::Wait(ms)) }
        else if let Some(r) = v["record"].as_str() { Some(Step::Record(if r == "start" { Some(v["name"].as_str().unwrap_or("rec").to_string()) } else { None })) }
        else if let (Some(id), Some(ms)) = (v["click"].as_str(), v["within"].as_u64()) { Some(Step::ClickWithin(id.into(), ms)) }
        else if let Some(id) = v["click"].as_str() { Some(Step::Click(id.into())) }
        else if let Some(id) = v["press"].as_str() { Some(Step::Press(id.into())) }
        else if let Some(op) = v["imp"].as_str() { Some(Step::Imp(op.into())) }
        else if let Some(what) = v["read"].as_str() { Some(Step::Read(what.into())) }
        else if let Some(on) = v["layers"].as_bool() { Some(Step::Layers(on, v["max_h"].as_f64().map(|x| x as f32), v["scroll"].as_f64().unwrap_or(0.0) as f32)) }
        else if let Some(name) = v["mark"].as_str() { Some(Step::Mark(name.into())) }
        else if let Some(name) = v["shot"].as_str() { Some(Step::Shot { name: name.into(), settle: v["settle"].as_str().map(String::from), timeout_ms: v["timeout_ms"].as_u64().unwrap_or(15_000) }) }
        else if let Some(kind) = v["wait_settled"].as_str() { Some(Step::WaitSettled { kind: kind.into(), tag: v["tag"].as_str().unwrap_or(kind).into(), timeout_ms: v["timeout_ms"].as_u64().unwrap_or(15_000) }) }
        else if v["fit"].as_bool() == Some(true) { Some(Step::Fit) }
        else if let Some(what) = v["expect"].as_str() { Some(Step::Expect(what.into())) }
        else if v["cef"].is_object() { Some(Step::Cef(v["cef"].clone())) }
        else if v["browser"].is_object() { Some(Step::Browser(v["browser"].clone())) }
        else if v["kicad"].is_object() { Some(Step::Kicad(v["kicad"].clone())) }
        else if let Some(kind) = v["view"].as_str() { Some(Step::View(kind.into())) }
        else if let Some(a) = v["pan"].as_array() { Some(Step::Pan(a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32, a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32)) }
        else if let Some(a) = v["zoom"].as_array() { Some(Step::Zoom(a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32, a.get(1).and_then(Value::as_f64).unwrap_or(0.5) as f32, a.get(2).and_then(Value::as_f64).unwrap_or(0.5) as f32)) }
        else if let Some(n) = v["scroll_state"].as_str() { Some(Step::ScrollState(n.to_string())) }
        else if let Some(n) = v["scroll"].as_f64() { Some(Step::Scroll(n as f32)) }
        else if let Some(sec) = v["net_sleep"].as_f64() { Some(Step::NetSleep(sec)) }
        else if let Some(k) = v["open_viewer"].as_str() { Some(Step::View(k.into())) }
        else if let Some(f) = v["files"].as_str() { Some(Step::Files(f.into())) }
        else if let Some(a) = v["mgmt"].as_str() { Some(Step::Mgmt(a.into())) }
        else if let Some(n) = v["files_download"].as_str().map(String::from).or_else(|| v["files_download"].as_u64().map(|n| n.to_string())) { Some(Step::Files(format!("download:{n}"))) }
        else if v["open_settings"].as_bool() == Some(true) || v["open_settings"].is_string() { Some(Step::Click("ui:Settings".into())) } // the gear in the title bar, as a click on its control
        else if let Some(k) = ["paste_image"].iter().find(|k| v.get(**k).is_some()) { Some(Step::Unsupported((*k).to_string())) }
        else { None }
    }).collect()
}

/// If the hub's rail array marks `want` as disabled (dimmed) or omits it (hidden), return that settled state immediately.
fn rail_row_state(viewer: &Value, want: &str) -> Option<&'static str> {
    if want.is_empty() { return None; }
    let rows = viewer["rail"].as_array()?;
    match rows.iter().find(|r| r["id"].as_str() == Some(want)) {
        Some(r) if r["state"].as_str() == Some("disabled") => Some("disabled"),
        Some(_) => None,
        None => Some("hidden"),
    }
}

impl Backplane {
    pub fn m(&self, event: &str, fields: Value) { metric(self.started, event, fields); }

    /// After each painted frame (check mode): the layout, the controls, the shown thread, the probes and the links, each on change.
    pub fn drawn_marks(&mut self) {
        let started = self.started;
        // the frame just painted is what is on screen: the controls and probes read below are its own (art::ctl_rects)
        crate::art::probes_frame_done();
        if crate::art::rects_on() {
            // G3: the stable regions' rectangles (as this frame painted them), on change
            let snap = crate::art::layout_snapshot();
            if snap != self.last_layout { self.last_layout = snap.clone(); metric(started, "layout", json!({"rects": serde_json::from_str::<Value>(&snap).unwrap_or(Value::Null)})); }
            // G4: the controls on screen, on change
            let rects = crate::art::ctl_rects();
            let ctl = HEADER.with_borrow(|registry| {
                let inventory = |id: &str, label: &str, rect_id: &str, enabled: bool, folded: bool| {
                    json!({"id": id, "label": label, "rect": rects.get(rect_id).map(|r| json!({"x": r[0], "y": r[1], "w": r[2], "h": r[3]})),
                        "enabled": enabled, "visible": true, "wired": enabled, "disabled_look": !enabled, "folded": folded})
                };
                self.all_controls().filter(|c| !registry.replaces(&c.id))
                    .map(|c| inventory(&c.id, &c.label, &c.id, !matches!(c.ctl, Ctl::None), false))
                    .chain(registry.actions.iter().map(|(id, a)| inventory(id, &a.label, &a.rect_id, a.press.is_some(), a.folded)))
                    .collect::<Vec<_>>()
            });
            let text = serde_json::to_string(&ctl).unwrap_or_default();
            if text != self.ctl_last {
                self.ctl_last = text; metric(started, "controls", json!({"controls": ctl}));
            }
            // G4c: the thread on show and its viewer's page and source, on change (an e2e sees where a link went)
            { static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
              let v = &self.data["thread"]["viewer"]; let shown = json!({"thread": self.selected, "open": v["open"], "key": crate::screens::viewer::shown_key(v), "page": self.project_page});
              let t = shown.to_string(); let mut g = LAST.lock().unwrap(); if *g != t { *g = t; metric(started, "shown", shown); } }
            // G4b: every probed rectangle (menus' items, header tools, rows' pins: pressable things that are not controls), on change
            let mut probes: Vec<(&String, &[f32; 4])> = rects.iter().collect(); probes.sort_by(|a, b| a.0.cmp(b.0));
            let ptext = serde_json::to_string(&probes).unwrap_or_default();
            if ptext != self.probes_last { self.probes_last = ptext; metric(started, "probes", json!({"rects": probes.iter().map(|(k, r)| json!({"id": k, "x": r[0], "y": r[1], "w": r[2], "h": r[3]})).collect::<Vec<_>>()})); }
        }
        if crate::art::rects_on() {
            // links in messages as this frame painted them (the check harness aims at these; a code block's Copy is not one)
            let links: Vec<Value> = self.link_and_copy_rects().into_iter().filter(|(id, _, _)| id.starts_with("link:")).map(|(id, href, b)| json!({"id": id, "href": href, "rect": {"x": f32::from(b.origin.x), "y": f32::from(b.origin.y), "w": f32::from(b.size.width), "h": f32::from(b.size.height)}})).collect();
            // and every selectable text block of them, its text's start and its box (the copy e2e drags across these)
            let blocks: Vec<Value> = self.screens.reg.borrow().blocks.iter().filter_map(|b| b.layout.as_ref().map(|l| (b, l.bounds())))
                .map(|(b, r)| json!({"text": b.text.chars().take(60).collect::<String>(), "rect": {"x": f32::from(r.origin.x), "y": f32::from(r.origin.y), "w": f32::from(r.size.width), "h": f32::from(r.size.height)}})).collect();
            let text = serde_json::to_string(&(&links, &blocks)).unwrap_or_default();
            if !self.links_sent || text != self.links_last { self.links_sent = true; self.links_last = text; metric(started, "links", json!({"links": links, "blocks": blocks, "supported": true})); }
        }
    }

    /// Called from render: arms first_frame, usable, and the pending step's result.
    pub fn frame_marks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.script_total > 0 || crate::art::rects_on() {
            HEADER.with_borrow_mut(|registry| {
                registry.window = Some(window.window_handle());
                registry.actions.clear();
                registry.present = false;
            });
        }
        let started = self.started;
        let this = cx.entity().downgrade();
        if !self.first_frame {
            self.first_frame = true;
            // G3, G4, G4b, G4c and the links: written after each frame is painted, when the controls this render registered and their
            // rectangles from that paint are both known (from render they were a frame behind, and an idle window then wrote nothing more:
            // a panel opened by a click showed in the log only at the next unrelated redraw)
            if crate::art::rects_on() { let me = this.clone(); window.on_frame_drawn(move |_, cx| me.update(cx, |v, _| v.drawn_marks()).is_ok()); }
            window.on_next_frame(move |_, _| metric(started, "first_frame", json!({})));
            match window.gpu_specs() {
                Some(g) => metric(started, "gpu", json!({"device": g.device_name, "driver": g.driver_name, "driver_info": g.driver_info, "software": g.is_software_emulated})),
                None => metric(started, "gpu", json!({"device": null})),
            }
        }
        // G1: the first frame presented after the injected event (its local effect: pressed row, typed text, selection)
        if let Some((label, t0)) = self.ack_pending.take() {
            window.on_next_frame(move |_, _| metric(started, "ack", json!({"step": label, "ack_ms": (t0.elapsed().as_secs_f64() * 1e6).round() / 1e3})));
        }
        // G2: a frame timestamp for every frame drawn while a record is open (frames are kept coming while it is)
        if let Some((_, frames)) = &mut self.recording { frames.push((started.elapsed().as_secs_f64() * 1e6).round() / 1e3); window.request_animation_frame(); }
        let draws = self.data["projects"].is_array(); // (a fresh home has none, and is as usable as one that has)
        if !self.usable && draws {
            self.usable = true;
            let entries = self.data["thread"]["entries"].as_array().map_or(0, |e| e.len());
            let threads = self.thread_ids().len();
            window.on_next_frame(move |_, _| metric(started, "usable", json!({"threads": threads, "entries": entries})));
            let this = this.clone();
            window.on_next_frame(move |_, cx| { let _ = this.update(cx, |t, cx| t.pump(cx)); });
        }
        let Some(p) = &self.pending else { return };
        if p.armed { return; }
        let done = match &p.goal {
            Goal::Thread(id) => self.data["thread"]["id"].as_str().map_or(false, |t| t.rsplit_once('|').map_or(t, |(_, i)| i) == id) && self.data["thread"]["loading"] != true,
            Goal::Frame => true,
            Goal::Changed(epoch) => self.ui_epoch != *epoch,
            Goal::Reconnected => self.reconnected,
            Goal::Home => self.data["projectHome"].is_object() || self.project_page.is_some(),
            Goal::Popup => self.data["thread"]["skills"].as_array().map_or(false, |s| !s.is_empty()), // the hub's rows, nothing local
            Goal::Typed(text) => self.input.read(cx).content.as_ref() == text,
            Goal::Sent(before) => self.user_entries() > *before,
            Goal::Sleep => false,
            Goal::Shot { .. } | Goal::WaitSettled { .. } => true,
        };
        if !done { return; }
        if matches!(p.goal, Goal::WaitSettled { .. }) {
            let (kind, tag, timeout_ms, polling, t0, seq) = match &p.goal {
                Goal::WaitSettled { kind, tag, timeout_ms, polling } => (kind.clone(), tag.clone(), *timeout_ms, *polling, p.started, p.seq),
                _ => unreachable!(),
            };
            let state = match self.viewer_settled(&kind, cx) {
                Some(st) => st,
                None if (t0.elapsed().as_millis() as u64) < timeout_ms => {
                    if !polling {
                        if let Some(Pending { goal: Goal::WaitSettled { polling, .. }, .. }) = &mut self.pending { *polling = true; }
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(Duration::from_millis(100)).await;
                            let _ = this.update(cx, |t, cx| {
                                if let Some(Pending { seq: s, goal: Goal::WaitSettled { polling, .. }, .. }) = &mut t.pending {
                                    if *s == seq { *polling = false; cx.notify(); }
                                }
                            });
                        }).detach();
                    }
                    return;
                }
                None => "timeout",
            };
            let vstate = self.viewer_state_json(cx);
            self.pending.as_mut().unwrap().armed = true;
            window.on_next_frame(move |_, cx| {
                metric(started, "settled", json!({"tag": tag, "kind": kind, "state": state, "settled": state != "timeout", "viewer": vstate}));
                let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } });
            });
            cx.notify();
            return;
        }
        if matches!(p.goal, Goal::Shot { .. }) {
            let (name, settle, dir, timeout_ms, polling, t0, seq) = match &p.goal {
                Goal::Shot { name, settle, dir, timeout_ms, polling } => (name.clone(), settle.clone(), dir.clone(), *timeout_ms, *polling, p.started, p.seq),
                _ => unreachable!(),
            };
            let state = match settle.as_deref() {
                None => "early",
                Some(want) => match self.viewer_settled(want, cx) {
                    Some(st) => st,
                    None if (t0.elapsed().as_millis() as u64) < timeout_ms => {
                        if !polling {
                            if let Some(Pending { goal: Goal::Shot { polling, .. }, .. }) = &mut self.pending { *polling = true; }
                            cx.spawn(async move |this, cx| {
                                cx.background_executor().timer(Duration::from_millis(100)).await;
                                let _ = this.update(cx, |t, cx| {
                                    if let Some(Pending { seq: s, goal: Goal::Shot { polling, .. }, .. }) = &mut t.pending {
                                        if *s == seq { *polling = false; cx.notify(); }
                                    }
                                });
                            }).detach();
                        }
                        return;
                    }
                    None => "timeout",
                },
            };
            let vstate = self.viewer_state_json(cx);
            self.pending.as_mut().unwrap().armed = true;
            let done_file = dir.join(format!("{name}.done"));
            window.on_next_frame(move |_, cx| {
                metric(started, "shot-ready", json!({"name": name, "state": state, "settled": state != "timeout" && state != "early", "viewer": vstate}));
                let _ = this.update(cx, |_, cx| {
                    cx.spawn(async move |this, cx| {
                        let wait_start = Instant::now();
                        let got = cx.background_executor().spawn(async move {
                            while !done_file.exists() {
                                if wait_start.elapsed() >= Duration::from_secs(15) { return false; }
                                std::thread::sleep(Duration::from_millis(25));
                            }
                            true
                        }).await;
                        if got {
                            metric(started, "shot-done", json!({"name": name, "ok": true, "latency_ms": (t0.elapsed().as_secs_f64() * 1e6).round() / 1e3}));
                            let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } });
                        } else {
                            /* i18n-ok: scripted e2e metric, not drawn */ metric(started, "shot-done", json!({"name": name, "ok": false, "error": "done timeout"}));
                            std::process::exit(5);
                        }
                    }).detach();
                });
            });
            cx.notify();
            return;
        }
        let (label, t0, seq) = (p.label.clone(), p.started, p.seq);
        self.pending.as_mut().unwrap().armed = true;
        if let Some((id, action)) = self.click_info.take() { self.m("click", json!({"id": id, "changed": true, "action": action})); }
        // where the result came from: the hub, the cache of a thread seen before, or rows drawn locally while the hub answers
        let source = if self.popup_local() { "local" } else { self.thread_source };
        if source != "hub" { self.confirm = Some((format!("{}:hub", self.pending.as_ref().map_or(String::new(), |p| p.label.clone())), self.pending.as_ref().map_or(Instant::now(), |p| p.started))); }
        // where the time went: hub round trip, decode, delta/merge, applying the body, building the tree, waiting for the frame
        let phases = match (&self.timing, self.applied_at) {
            (Some(t), Some(a)) => Some((t.clone(), self.apply_ms, self.render_us as f64 / 1000.0, a)),
            _ => None,
        };
        window.on_next_frame(move |_, cx| {
            let mut fields = json!({"step": label, "ok": true, "latency_ms": (t0.elapsed().as_secs_f64() * 1e6).round() / 1e3, "source": source});
            if let Some((t, apply_ms, render_ms, applied)) = phases {
                let r = |v: f64| (v * 1e3).round() / 1e3;
                fields["hub_rtt_ms"] = json!(r(t.rtt_ms)); fields["decode_ms"] = json!(r(t.decode_ms)); fields["process_ms"] = json!(r(t.process_ms));
                fields["apply_ms"] = json!(r(apply_ms)); fields["render_build_ms"] = json!(r(render_ms));
                fields["applied_to_frame_ms"] = json!(r(applied.elapsed().as_secs_f64() * 1000.0)); fields["bytes"] = json!(t.bytes); fields["delta"] = json!(t.delta);
            }
            metric(started, "input", fields);
            let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } });
        });
        cx.notify();
    }

    fn begin(&mut self, label: String, goal: Goal, cx: &mut Context<Self>) { self.begin_for(label, goal, 15, cx) }
    /// `fail_after`: seconds before a step whose result never shows is reported failed (a net_sleep is its sleep + 15).
    fn begin_for(&mut self, label: String, goal: Goal, fail_after: u64, cx: &mut Context<Self>) {
        self.seq += 1;
        let seq = self.seq;
        self.timing = None; self.applied_at = None; self.ack_pending = Some((label.clone(), Instant::now()));
        self.pending = Some(Pending { label, started: Instant::now(), goal, armed: false, seq });
        // A step whose result never shows is reported failed after 15 s so the script still ends.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(fail_after)).await;
            let _ = this.update(cx, |t, cx| {
                if let Some(p) = &t.pending { if p.seq == seq && !p.armed {
                    let (l, t0) = (p.label.clone(), p.started);
                    t.m("input", json!({"step": l, "ok": false, "latency_ms": t0.elapsed().as_secs_f64() * 1e3}));
                    t.pending = None; t.pump(cx);
                } }
            });
        }).detach();
    }

    /// Run the next script step when none is pending.
    pub fn pump(&mut self, cx: &mut Context<Self>) {
        if self.pending.is_some() { return; }
        let Some(step) = self.steps.pop_front() else {
            if self.script_total > 0 && !self.script_done {
                self.script_done = true;
                self.m("script_done", json!({"steps": self.script_total}));
                // BACKPLANE_GPUI_HOLD keeps the window up after the script (the harness screenshots it, then ends the process)
                if std::env::var_os("BACKPLANE_GPUI_HOLD").is_none() { cx.spawn(async move |_, cx| { cx.background_executor().timer(Duration::from_millis(300)).await; let _ = cx.update(|cx| cx.quit()); }).detach(); }
            }
            return;
        };
        match step {
            Step::Select(i) => {
                let ids = self.thread_ids();
                match ids.get(i).cloned() {
                    Some(id) => {
                        let raw = id.rsplit_once('|').map_or(id.as_str(), |(_, r)| r).to_string();
                        self.begin(format!("select:{i}"), Goal::Thread(raw), cx);
                        self.select_id(id, cx);
                    }
                    /* i18n-ok: scripted e2e input and its metrics, not drawn */ None => { self.m("input", json!({"step": format!("select:{i}"), "ok": false, "error": "no such thread"})); self.pump(cx); }
                }
            }
            Step::Home(i) => {
                let pid = self.data["projects"][i]["id"].as_str().unwrap_or("").to_string();
                let raw = pid.rsplit_once('|').map_or(pid.as_str(), |(_, r)| r).to_string();
                self.begin(format!("home:{i}"), Goal::Home, cx);
                self.open_project(raw.clone(), cx);
            }
            Step::Popup(text) => { self.begin(format!("popup:{text}"), Goal::Popup, cx); self.set_text(text, cx); }
            Step::Keys(keys) => { self.m("keys", json!({"keys": keys})); self.screens.term.script = Some(keys); self.begin("keys".into(), Goal::Frame, cx); cx.notify(); }
            Step::Type(text) => {
                let full = format!("{}{}", self.input.read(cx).content, text);
                self.begin(format!("type:{}", text.chars().count()), Goal::Typed(full.clone()), cx);
                self.set_text(full, cx);
            }
            Step::Send => {
                let before = self.user_entries();
                self.begin("send".into(), Goal::Sent(before), cx);
                self.do_send(cx);
            }
            Step::Record(Some(name)) => { self.m("record", json!({"state": "start", "name": name})); self.recording = Some((name, Vec::new())); cx.notify(); self.pump(cx); }
            Step::Record(None) => {
                if let Some((name, frames)) = self.recording.take() {
                    self.m("frames", json!({"name": name, "n": frames.len(), "frames_ms": frames}));
                    // the plot canvas's own CPU per paint while the record was open (culling, path building, tessellation, submission), not the rasterizer's time
                    let mut ms = std::mem::take(&mut self.screens.plotview.paint_ms);
                    if !ms.is_empty() {
                        ms.sort_by(|a, b| a.partial_cmp(b).unwrap()); let p = |q: f32| ms[((ms.len() - 1) as f32 * q).round() as usize];
                        let sp = std::mem::take(&mut self.screens.plotview.split);
                        let part = |k: usize| { let mut v: Vec<f32> = sp.iter().map(|x| x[k]).collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); if v.is_empty() { (0.0, 0.0) } else { (v[(v.len() - 1) / 2], v[((v.len() - 1) as f32 * 0.95).round() as usize]) } };
                        let (lp, tess, sub) = (part(0), part(1), part(2));
                        self.m("plot_paint", json!({"name": name, "n": ms.len(), "p50_ms": p(0.5), "p95_ms": p(0.95), "max_ms": p(1.0), "loop_cull_build_p50_p95_ms": [lp.0, lp.1], "tessellate_p50_p95_ms": [tess.0, tess.1], "paint_path_p50_p95_ms": [sub.0, sub.1]}));
                    }
                }
                self.pump(cx);
            }
            // where the timeline stands: e2e reads this, a picture cannot say whether a scroll held
            Step::ScrollState(name) => {
                let max = f32::from(self.timeline.max_offset_for_scrollbar().height);
                let y = -(-f32::from(self.timeline.scroll_px_offset_for_scrollbar().y)).min(max);
                let rows = self.data["thread"]["entries"].as_array().map_or(&[][..], |e| e.as_slice());
                let ids: std::collections::HashSet<&str> = rows.iter().filter_map(|e| e["id"].as_str()).collect();
                let top = self.timeline.logical_scroll_top();
                self.m("scroll", json!({"name": name, "y": y, "max": max, "entries": rows.len(), "dups": rows.len() - ids.len(), "first": rows.first().map_or("", |e| e["id"].as_str().unwrap_or("")),
                    "last": rows.last().map_or("", |e| e["id"].as_str().unwrap_or("")), "top": rows.get(top.item_ix).map_or("(end)", |e| e["id"].as_str().unwrap_or("")), "px": f32::from(top.offset_in_item), "more": self.data["thread"]["historyOlder"].as_str().is_some_and(|c| !c.is_empty())}));
                self.pump(cx);
            }
            Step::Scroll(n) => {
                self.begin(format!("scroll:{n}"), Goal::Frame, cx);
                // a project's page scrolls its own column
                if !self.screens.home_list_key.is_empty() { let l = &self.screens.home_list; let top = l.logical_scroll_top(); self.m("home_scroll", json!({"item": top.item_ix, "px": f32::from(top.offset_in_item), "items": l.item_count()})); l.scroll_by(px(n)); self.ui_epoch += 1; cx.notify(); return; }
                if self.project_page.is_some() || self.data["projectHome"].is_object() { let h = &self.screens.home_scroll; self.m("home_scroll", json!({"y": f32::from(h.offset().y), "max": f32::from(h.max_offset().height)})); let o = h.offset(); h.set_offset(point(o.x, (o.y - px(n)).min(px(0.)).max(-h.max_offset().height))); self.ui_epoch += 1; cx.notify(); return; }
                // at the end the list's own top is past its last row, a screen below where the wheel starts from
                let at_end = self.timeline.logical_scroll_top().item_ix >= self.timeline.item_count();
                self.timeline.scroll_by(px(n) - if at_end { self.timeline.viewport_bounds().size.height } else { px(0.) }); self.ui_epoch += 1; cx.notify();
            }
            Step::NetSleep(secs) => {
                self.reconnected = false; self.begin_for(format!("net_sleep:{secs}"), Goal::Reconnected, secs.max(0.0) as u64 + 15, cx);
                let _ = self.tx.try_send(crate::hub::Command::Sleep(secs));
            }
            Step::Unsupported(k) => { self.m("input", json!({"step": k, "ok": false, "error": "not_implemented"})); self.pump(cx); }
            Step::View(kind) => { // open a viewer page like the rail does (the idempotent `view` action)
                let _ = self.tx.try_send(crate::screens::viewer::view_kind(&kind)); self.m("view", json!({"kind": kind})); cx.notify(); self.pump(cx);
            }
            Step::Mgmt(a) => { crate::screens::mgmt::act(self, &a, cx); self.ui_epoch += 1; self.m("mgmt", json!({"step": a})); self.pump(cx); }
            Step::Files(spec) => { let r = crate::screens::filesui::script(self, &spec, cx); self.m("files", json!({"step": spec, "ok": r.is_ok(), "error": r.err()})); self.pump(cx); }
            Step::Cef(cmd) => { // {"cef":{"report":true|"goto":url|"back"|"forward":true|"click":[x,y]|"type":text|"handback":true}}: the embedded browser page, for the e2e (feature cef; else not implemented)
                #[cfg(feature = "cef")]
                { let page = self.screens.browser.as_ref().and_then(|(_, p)| p.read(cx).cef.clone());
                  /* i18n-ok: scripted e2e input and its metrics, not drawn */ match page { Some(p) => { let r = p.update(cx, |c, cx| c.script(&cmd, cx)); self.m("cef", r); } None => { let v = self.data["thread"]["viewer"].clone(); self.m("cef", json!({"ok": false, "error": "no embedded page", "have_page": self.screens.browser.is_some(), "viewer_open": v["open"], "viewer_keys": v.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>())})) } } }
                #[cfg(not(feature = "cef"))]
                /* i18n-ok: scripted e2e input and its metrics, not drawn */ { let _ = cmd; self.m("cef", json!({"ok": false, "error": "built without the cef feature"})); }
                self.pump(cx);
            }
            Step::Kicad(cmd) => { // {"kicad":{"report":true}} or {"kicad":{"wait_painted":25000,"kind":"PCB","tag":"..."}} (viewers/kicad_pane.rs)
                let tag = cmd["tag"].as_str().map(String::from);
                // {"kicad":{"attach":"board","why":"auto"}}: the thread's design in the embedded KiCad as the rail's reconcile asks for it
                // (why "auto": ShowBy::Automatic, shown without the keyboard; anything else deliberate, as Edit)
                if let Some(open) = cmd["attach"].as_str() {
                    let why = if cmd["why"].as_str() == Some("auto") { crate::viewers::kicad_pane::ShowBy::Automatic } else { crate::viewers::kicad_pane::ShowBy::Deliberate };
                    let ok = self.kicad_attach(open, why, None, cx);
                    self.ui_epoch += 1;
                    self.m("kicad", json!({"attach": open, "why": format!("{why:?}"), "ok": ok, "tag": tag}));
                    self.pump(cx);
                } else if let Some(ms) = cmd["wait_painted"].as_u64() {
                    let want_kind = cmd["kind"].as_str().map(String::from);
                    let min_lit = cmd["min_painted"].as_u64().unwrap_or(15);
                    let t0 = Instant::now();
                    cx.spawn(async move |this, cx| {
                        let mut first = true;
                        loop {
                            let elapsed = t0.elapsed().as_millis() as u64;
                            let step_ms = if elapsed < 500 { 25 } else { 120 };
                            if !first { cx.background_executor().timer(Duration::from_millis(step_ms)).await; }
                            first = false;
                            let done = this.update(cx, |t, cx| {
                                let mut r = match t.screens.kicad.clone() { Some(p) => p.read(cx).report(), None => json!({"ok": false, "error": "not editing"}) }; // i18n-ok: scripted e2e input and its metrics, not drawn
                                let shown = r["shown"].as_u64().unwrap_or(0);
                                let c_lit = r["container"]["painted"].as_u64().unwrap_or(0);
                                let f_ok = shown != 0 && r["frames"].as_array().is_some_and(|fs| fs.iter().any(|f| {
                                    f["xid"].as_u64() == Some(shown)
                                        && f["painted"].as_u64().unwrap_or(0) >= min_lit
                                        && want_kind.as_deref().map_or(true, |wk| f["kind"].as_str() == Some(wk))
                                }));
                                let ready = r["state"].as_str() == Some("ready"); // i18n-ok: wire state enum
                                let failed = matches!(r["state"].as_str(), Some("failed" | "stopped")); // i18n-ok: wire state enum
                                if (ready && c_lit >= min_lit && f_ok) || failed || t0.elapsed().as_millis() as u64 >= ms {
                                    r["wait_ms"] = json!(t0.elapsed().as_millis() as u64);
                                    if let Some(tg) = &tag { r["tag"] = json!(tg); }
                                    t.m("kicad", r);
                                    t.pump(cx);
                                    true
                                } else {
                                    false
                                }
                            }).unwrap_or(true);
                            if done { break }
                        }
                    }).detach();
                } else {
                    /* i18n-ok: scripted e2e input and its metrics, not drawn */ let mut r = match self.screens.kicad.clone() { Some(p) => p.read(cx).report(), None => json!({"ok": false, "error": "not editing"}) };
                    if let Some(tg) = tag { r["tag"] = json!(tg); }
                    self.m("kicad", r);
                    self.pump(cx);
                }
            }
            Step::Browser(cmd) => { // {"browser":{"op":...}}: the Browser tab's page, embedded or the hub's frames (viewers/browser.rs BrowserPage::script)
                /* i18n-ok: scripted e2e input and its metrics, not drawn */ match self.screens.browser.as_ref().map(|(_, p)| p.clone()) { Some(p) => { let r = p.update(cx, |b, cx| b.script(&cmd, cx)); self.m("browser", r); } None => self.m("browser", json!({"ok": false, "error": "no Browser page", "op": cmd["op"]})) }
                self.pump(cx);
            }
            Step::Mark(name) => { self.m("mark", json!({"name": name})); self.pump(cx); }
            Step::Shot { name, settle, timeout_ms } => {
                let dir = std::env::var_os("BACKPLANE_GPUI_SHOT_DIR").map(std::path::PathBuf::from).filter(|p| !p.as_os_str().is_empty());
                let Some(dir) = dir else {
                    /* i18n-ok: scripted e2e metric, not drawn */ self.m("shot", json!({"name": name, "ok": false, "error": "BACKPLANE_GPUI_SHOT_DIR not set"}));
                    self.pump(cx);
                    return;
                };
                if !valid_shot_name(&name) {
                    /* i18n-ok: scripted e2e metric, not drawn */ self.m("shot", json!({"name": name, "ok": false, "error": "invalid shot name"}));
                    self.pump(cx);
                    return;
                }
                let fail_s = (timeout_ms / 1000).max(5) + 20;
                self.begin_for(format!("shot:{name}"), Goal::Shot { name, settle, dir, timeout_ms, polling: false }, fail_s, cx);
                cx.notify();
            }
            Step::WaitSettled { kind, tag, timeout_ms } => {
                let fail_s = (timeout_ms / 1000).max(5) + 15;
                self.begin_for(format!("wait_settled:{tag}"), Goal::WaitSettled { kind, tag, timeout_ms, polling: false }, fail_s, cx);
                cx.notify();
            }
            Step::Fit => { self.screens.plotview.view = None; self.ui_epoch += 1; self.begin("fit".into(), Goal::Frame, cx); cx.notify(); }
            Step::Expect(what) => { // a run that cannot measure what it claims stops itself: "canvas" needs a plot canvas with real height
                let h = self.screens.plotview.bounds.map_or(0.0, |b| f32::from(b.size.height));
                /* i18n-ok: scripted e2e input and its metrics, not drawn */ if what == "canvas" && h < 200.0 { self.m("not_usable", json!({"why": "plot canvas has no height", "canvas_h": h})); eprintln!("expect canvas: height {h} px: stopping"); std::process::exit(4); }
                self.m("expect", json!({"what": what, "canvas_h": h})); self.pump(cx);
            }
            Step::Layers(on, cap, scroll) => { let pv = &mut self.screens.plotview; pv.layers_cap = cap; pv.layers_scroll.set_offset(point(px(0.), px(-scroll)));
                self.screens.plotview.layers_open = on; self.ui_epoch += 1; self.begin("layers".into(), Goal::Frame, cx); cx.notify(); }
            Step::ViewShow(kind, path) => { let t = self.selected.clone(); self.view_show(&t, &kind, &path, cx); self.m("viewshow", json!({"kind": kind, "path": path})); self.ui_epoch += 1; self.begin("viewshow".into(), Goal::Frame, cx); cx.notify(); }
            Step::Compare(i) => { // the Compare menu's row i (null: put the comparison away); the versions are asked for first
                match i { None => crate::screens::histbar::compare(self, None),
                    Some(i) => { if self.screens.plotview.hist.refs.is_none() { crate::screens::histbar::ask_refs(self); } let r = self.screens.plotview.hist.refs.clone().and_then(|r| r.rows.get(i).cloned());
                        /* i18n-ok: scripted e2e input and its metrics, not drawn */ if let Some((_, name, commit)) = r { crate::screens::histbar::compare(self, Some((&commit, &name))); } else { self.m("compare", json!({"error": "no such version yet"})); } } }
                self.ui_epoch += 1; self.begin("compare".into(), Goal::Frame, cx); cx.notify(); }
            Step::Hist(i) => { // the history bar's step i (0-based; null or negative: live); the steps are asked for first
                if self.screens.plotview.hist.steps.is_none() { crate::screens::histbar::ask_steps(self); self.m("hist", json!({"asked": true})); }
                else { crate::screens::histbar::select(self, i.filter(|n| *n >= 0).map(|n| n as usize)); self.m("hist", json!({"at": i})); }
                self.ui_epoch += 1; self.begin("hist".into(), Goal::Frame, cx); cx.notify(); }
            Step::HistMode(m) => { crate::screens::histbar::mode(self, m); self.ui_epoch += 1; self.begin("histmode".into(), Goal::Frame, cx); cx.notify(); }
            Step::HistPlay => { crate::screens::histbar::play(self, cx); self.ui_epoch += 1; self.begin("histplay".into(), Goal::Frame, cx); cx.notify(); }
            Step::Sheets(on) => { crate::screens::sheets::set_open(self, 0, on, None, cx); self.ui_epoch += 1; self.begin("sheets".into(), Goal::Frame, cx); cx.notify(); }
            Step::Tap(fx, fy, slot) => { // a tap at (fx, fy) as fractions of canvas `slot` (0 the Board/Schematic page, 1 a Split's schematic, 2 its board): inspect what lies there
                let (view, bounds) = { let pv = self.screens.pv(slot); (pv.view, pv.bounds) };
                let panes = self.data["thread"]["viewer"]["panes"].as_array().cloned().unwrap_or_default();
                let (key, offs) = if slot == 0 { (self.screens.plot.frame.as_ref().map(|f| f.key.clone()).unwrap_or_default(), self.data["thread"]["viewer"]["off"].as_u64().unwrap_or(0) as u32) }
                    else { let p = panes.get(slot - 1).cloned().unwrap_or_default(); (p["key"].as_str().unwrap_or("").to_string(), p["off"].as_u64().unwrap_or(0) as u32) };
                if let Some(b) = bounds { let at = (f32::from(b.origin.x) + fx * f32::from(b.size.width), f32::from(b.origin.y) + fy * f32::from(b.size.height)); crate::screens::plotview::tap_at(self, slot, &key, offs, at, view, Some(b)); }
                let got = self.screens.pv(slot).picked.is_some(); self.m("tap", json!({"fx": fx, "fy": fy, "slot": slot, "picked": got})); self.ui_epoch += 1; self.begin("tap".into(), Goal::Frame, cx); cx.notify();
            }
            Step::Pan(dx, dy) => { // a drag of the plot canvas by (dx, dy) window px
                let pv = &mut self.screens.plotview; if let Some(v) = pv.view { pv.view = Some(v.pan(dx, dy)); }
                self.ui_epoch += 1; self.begin("pan".into(), Goal::Frame, cx); cx.notify();
            }
            Step::Zoom(steps, fx, fy) => { // wheel notches at (fx, fy) as fractions of the canvas
                let pv = &mut self.screens.plotview;
                if let (Some(v), Some(b)) = (pv.view, pv.bounds) {
                    let (w, h) = (f32::from(b.size.width), f32::from(b.size.height));
                    pv.view = Some(v.zoom((1.0 + steps * 16.0 * 0.0015).clamp(0.5, 2.0), fx * w, fy * h, w, h, pv.fitted));
                }
                self.ui_epoch += 1; self.begin("zoom".into(), Goal::Frame, cx); cx.notify();
            }
            Step::Read(what) => {
                // G6: the composer's text, or the thread selection (GPUI has no text selection yet: "" with supported:false); "timeline": the open conversation
                let (text, supported) = match what.as_str() { "composer" => (self.input.read(cx).content.to_string(), true),
                    // the settings overlay's structure (labels, button actions, field names; never a field's value), to design its layout from the real payload
                    "settings" => (crate::screens::settings::structure(&self.data["settings"]), true),
                    // the viewer pane as last drawn: "<page>[ pressed]|<loading line>|<the rail's lit kind>|<the window's error line>" (the tab-loading e2e)
                    "pane" => (format!("{}|{}|{}", crate::screens::viewer::drawn(), crate::screens::viewer::active_kind(&self.data["thread"]["viewer"], self.screens.rail_pending.as_deref()), self.error.replace('|', "/")), true),
                    // the performance overlay's text; not supported while it is hidden
                    "perf" => crate::perf::lines().map_or((String::new(), false), |l| (l.join("\n"), true)),
                    // the open thread's conversation as the window holds it (its id, then one "role: text" line per entry), for e2e checks that a view
                    // follows its thread without a re-open (lane T, run-c); text only, never drafts or fields
                    "timeline" => { let t = &self.data["thread"];
                        let mut out = vec![format!("thread: {}", t["id"].as_str().unwrap_or(""))]; // i18n-ok: an e2e script's read, never drawn
                        for e in t["entries"].as_array().map_or(&[][..], |r| r.as_slice()) {
                            out.push(format!("{}: {}", e["role"].as_str().or(e["kind"].as_str()).unwrap_or(""), e["text"].as_str().unwrap_or("").replace('\n', " "))); }
                        (out.join("\n"), true) }
                    // the open palette's highlighted item (label + canonical id) before Enter, for B1 verification
                    "palette" => (crate::screens::palette::selected_snapshot(&self.data, &self.screens.palette).to_string(), true),
                    // on-demand rail snapshot trigger (test-mode only: clears the rail metric dedup cache so the next paint re-emits it)
                    "rail" => {
                        let on = crate::ui::rail_rects_on();
                        if on { self.screens.rail_metric = None; cx.notify(); }
                        (String::new(), on)
                    }
                    _ => (String::new(), false) };
                self.m("read", json!({"what": what, "text": text, "supported": supported}));
                self.pump(cx);
            }
            Step::PairFill(link, addr, code) => { crate::screens::pair::fill(self, &link, &addr, &code, cx); self.pump(cx); }
            Step::ClickWithin(id, ms) => {
                // not drawn yet: wait 250 ms and look again; found, or out of time: the plain click (which reports found:false)
                if ms == 0 || self.find_script_control(&id).is_some() { self.steps.push_front(Step::Click(id)); self.pump(cx); return; }
                self.steps.push_front(Step::ClickWithin(id, ms.saturating_sub(250)));
                self.steps.push_front(Step::Wait(250));
                self.pump(cx);
            }
            Step::Press(id) => {
                if let Some(s) = &self.screens.solid {
                    let sr = s.read(cx);
                    PRESS_SOLID.with(|c| *c.borrow_mut() = (sr.key().to_string(), sr.painted_count()));
                }
                let found = self.find_script_control(&id);
                let action = found.map(|e| self.do_click(&e.ctl, cx));
                self.m("press", json!({"id": id, "found": action.is_some(), "action": action}));
                self.ui_epoch += 1;
                cx.notify();
                // Header dispatch borrows the window after this update. Preserve script
                // ordering for a press immediately followed by another input or read.
                let entity = cx.entity();
                cx.defer(move |cx| entity.update(cx, |this, cx| this.pump(cx)));
            }
            Step::Click(id) => {
                let epoch = self.ui_epoch;
                let found = self.find_script_control(&id);
                match found {
                    None => { self.m("click", json!({"id": id, "found": false, "changed": false, "action": Value::Null})); self.pump(cx); }
                    Some(entry) => {
                        self.begin(format!("click:{id}"), Goal::Changed(epoch), cx);
                        let action = self.do_click(&entry.ctl, cx);
                        self.click_info = Some((id.clone(), action.clone()));
                        let seq = self.seq;
                        // a click that changes nothing is reported after 1.5 s (a dead control)
                        cx.spawn(async move |this, cx| {
                            cx.background_executor().timer(Duration::from_millis(1500)).await;
                            let _ = this.update(cx, |t, cx| {
                                if t.pending.as_ref().map_or(false, |p| p.seq == seq && !p.armed) {
                                    t.m("click", json!({"id": id, "changed": false, "action": if action == "none" { Value::Null } else { json!(action) }}));
                                    t.click_info = None; t.pending = None; t.pump(cx);
                                }
                            });
                        }).detach();
                    }
                }
            }
            Step::Field(name, text) => {
                // the e2e hook for typing into a named form field (the rename box): its text set as typing would
                let input = match name.as_str() { "pq" => self.screens.projects.search.clone(), "picker" => self.screens.projects.folder.clone(), _ => self.screens.fields.get(&name).map(|s| s.input.clone()) };
                if let Some(input) = input { input.update(cx, |i, cx| i.set(text.clone(), cx)); }
                self.seq += 1; let seq = self.seq;
                self.pending = Some(Pending { label: "field".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_millis(100)).await; let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } }); }).detach();
            }
            Step::Action(a, v) => {
                // the e2e hook for a view action a menu or a row would send (e.g. a delete asked on a thread that is not selected)
                let v = if a.starts_with("row-") { crate::screens::tools::qualify(&self.data, &v) } else { v.clone() }; // a row's own menu sends the id as the sidebar carries it
                let _ = self.tx.try_send(crate::hub::Command::Action(a.clone(), v));
                self.seq += 1; let seq = self.seq;
                self.pending = Some(Pending { label: "action".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_millis(100)).await; let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } }); }).detach();
            }
            Step::Imp(op) => {
                // Improvements e2e hook (screens::improvements::script_op): the same code as the panel, then a short wait for its effect
                crate::screens::improvements::script_op(self, &op, cx);
                self.seq += 1; let seq = self.seq;
                self.pending = Some(Pending { label: "imp".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_millis(250)).await; let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } }); }).detach();
            }
            Step::Attach(path) => {
                // the e2e hook for a dropped or picked file: the same path as a drop on the window (screens::glue::attach_event)
                self.attach_event(&crate::screens::composer::ComposerEvent::Files(vec![std::path::PathBuf::from(path)]), cx);
                self.seq += 1; let seq = self.seq;
                self.pending = Some(Pending { label: "attach".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| { cx.background_executor().timer(Duration::from_millis(100)).await; let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } }); }).detach();
            }
            Step::Wait(ms) => {
                self.seq += 1;
                let seq = self.seq;
                self.pending = Some(Pending { label: "wait".into(), started: Instant::now(), goal: Goal::Sleep, armed: true, seq });
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_millis(ms)).await;
                    let _ = this.update(cx, |t, cx| { if t.pending.as_ref().map_or(false, |p| p.seq == seq) { t.pending = None; t.pump(cx); } });
                }).detach();
            }
        }
    }

    /// What a click on a registered control does; returns what it sent ("none" for a control with no handler).
    pub fn do_click(&mut self, ctl: &Ctl, cx: &mut Context<Self>) -> String {
        use crate::hub::Command;
        match ctl {
            Ctl::Header(id) => {
                let action = HEADER.with_borrow(|r| Some((r.window?, r.actions.get(id)?.press.clone()?)));
                let Some((handle, press)) = action else { return "none".into() };
                let entity = cx.entity();
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, cx| entity.update(cx, |this, cx| {
                        press.run(this, window, cx);
                        this.ui_epoch += 1;
                        cx.notify();
                    }));
                });
                format!("header {id}") // i18n-ok: script result diagnostic
            }
            Ctl::Hub(a) => { crate::screens::hubpick::act(self, a.clone(), cx); self.ui_epoch += 1; "hub".into() }
            Ctl::PairGo => { crate::screens::pair::go(self, cx); self.ui_epoch += 1; "pair".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::PairYes => { crate::screens::pair::confirm(self, cx); self.ui_epoch += 1; "pair confirmed".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::PairCancel => { crate::screens::pair::cancel(self, cx); self.ui_epoch += 1; "pair cancelled".into() }
            Ctl::None => "none".into(),
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Pad => "pad (mouse only)".into(),
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Rail(kind) => { let cmd = self.rail_view(kind, cx); let shut = matches!(&cmd, crate::hub::Command::Action(_, k) if k.is_empty()); let _ = self.tx.try_send(cmd); self.ui_epoch += 1; if shut { "view.action view close".into() } else { "view.action view".into() } }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Mgmt(a) => { crate::screens::mgmt::act(self, a, cx); self.ui_epoch += 1; format!("mgmt {a}") }
            Ctl::Dictate => { self.voice_press(cx); "dictate".into() }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::ThreadMenu => { let items = crate::screens::ctxmenu::title_items(&self.data); let n = items.len(); if n > 0 { self.ctx_open(point(px(320.), px(40.)), items, cx); } self.ui_epoch += 1; format!("thread menu {n}") }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::PickFiles => { self.pick_files(cx); self.ui_epoch += 1; "attach picker".into() }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::ToggleSidebar => { self.sidebar_toggle(cx); "toggle sidebar".into() }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::ToggleContext => { self.screens.ctx_open = !self.screens.ctx_open; self.ui_epoch += 1; cx.notify(); "context panel".into() }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::Select(id) => { self.select_id(id.clone(), cx); "view.open thread".into() }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::Home(pid) => { self.open_project(pid.clone(), cx); "project page".into() }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::Go(a, v) => { self.go(a, v.clone(), cx); self.ui_epoch += 1; format!("go {a} {v}") }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::Fold(v) => { self.fold_click(v.clone()); "view.action fold".into() }
            /* i18n-ok: scripted e2e input and its metrics */ Ctl::Sub(k) => { if k.is_empty() { self.sub_back(cx); "sub back".into() } else { self.sub_open(k.clone(), cx); "sub open".into() } }
            Ctl::Send => { self.do_send(cx); self.ui_epoch += 1; "send".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Skill(w) => { let _ = self.tx.try_send(Command::Action("skill".into(), w.clone())); self.await_completion(cx); self.ui_epoch += 1; "view.action skill".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Page(v) => { let _ = self.tx.try_send(Command::Page(v.clone())); "view.action native-sidebar-page".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::SettingsPage(p) => { crate::screens::settings::go_page(self, p); self.ui_epoch += 1; cx.notify(); format!("settings page {p}") }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::AccountMenu => { self.screens.acct.script_menu = true; self.ui_epoch += 1; cx.notify(); "account menu".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::AccountsPage => { if !crate::screens::settings::is_open(self) { self.open_settings(cx); } self.screens.settings_ui.device = false; self.screens.settings_ui.accounts = true; crate::screens::accounts::open_page(self, cx); self.ui_epoch += 1; "accounts page".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::OpenPalette => { self.palette_open(cx); self.ui_epoch += 1; "open palette".into() }
            Ctl::ToggleTerminal => {
                let entity = cx.entity();
                if let Some(win) = cx.windows().into_iter().next() {
                    cx.defer(move |cx| { let _ = win.update(cx, |_, window, cx| { entity.update(cx, |this, cx| this.term_toggle(window, cx)); }); });
                }
                self.ui_epoch += 1;
                /* i18n-ok: scripted e2e input and its metrics, not drawn */ "toggle terminal".into()
            }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::FocusSearch => { self.focus_search(cx); self.ui_epoch += 1; "focus search".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::RenameSave(id) => { let text = self.screens.fields.get("title").map(|s| s.input.read(cx).content.to_string()).unwrap_or_default(); if let Some(c) = crate::screens::tools::rename_save(id, &text) { let _ = self.tx.try_send(c); } "view.action rename-save".into() }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Imp(id) => { crate::screens::improvements::act(self, id, cx); self.ui_epoch += 1; format!("improvements {id}") }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Term(op, key) => { match op.as_str() { "expand" => self.card_expand(key, cx), "pop" => self.card_pop(key, None, cx), "stop" => self.card_stop(key, cx), _ => self.card_take_keys(key, None, cx) } format!("term {op}") }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Bot(k, v) => { crate::screens::bots::press(self, k, v, cx); self.ui_epoch += 1; format!("bots {k}") }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::KicadAct(a) => { match a.as_str() {
                "popin" => { let h = self.screens.kicad_window; self.kicad_popin(cx); if let Some(h) = h { let _ = h.update(cx, |_, w, _| w.remove_window()); } }
                "full" | "screen" | "popout" | "checks" => self.kicad_strip_act(a, cx),
                "retry" => { if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, cx| { p.reattach(); cx.notify(); }); } }
                "simulation" => self.open_simulation(cx),
                "gerbview" => { let files: Vec<String> = self.data["thread"]["viewer"]["gerberFiles"].as_array().map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect()).unwrap_or_default(); self.open_in_gerbview(files, cx) }
                k => { if let Some(p) = self.screens.kicad.clone() { let k = k.to_string(); p.update(cx, |p, cx| p.show_kind_by(&k, "client script (test driver)", cx)); } } } // i18n-ok: a debug log cause (BACKPLANE_KICAD_DEBUG)
                self.ui_epoch += 1; format!("kicad {a}") } // i18n-ok: an e2e metric
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::KicadEdit(on) => { if *on { let ok = self.kicad_edit(cx); self.ui_epoch += 1; format!("kicad edit {ok}") } else { self.kicad_view(cx); self.ui_epoch += 1; "kicad view".into() } }
            /* i18n-ok: scripted e2e input and its metrics, not drawn */ Ctl::Node(a, p) => { if a == "open-thread" { self.select_id(p.clone(), cx); return format!("view.action {a}"); } if a == "run-on" { if let Some((pj, to)) = p.split_once('|') { self.screens.projects.picked.insert(pj.to_string(), to.to_string()); } } if a == "flag" && p == "settings" { self.open_settings(cx); return format!("view.action {a}"); } if a == "proj-add" || a == "proj-new" || a == "new-thread" || crate::screens::tools::makes_thread(&a) { self.arm_new(); } self.go(&a, p.clone(), cx); format!("view.action {a}") }
        }
    }
    /// The popup is always the hub's rows now (no client-side rows), so a popup step is never local.
    pub fn popup_local(&self) -> bool { false }
    fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.input.update(cx, |i, cx| i.set(text, cx));
    }
    /// Find a registered control, or a probed viewer bar control (`follow-agent`, `ui:vs board`) when currently drawn.
    fn find_script_control(&self, id: &str) -> Option<CtlEntry> {
        /* i18n-ok: test-mode scripted kicad_edit route (PR #1353), inert outside --script */ if id == "script:kicad-edit" { return Some(CtlEntry { id: id.into(), label: "Edit".into(), ctl: Ctl::KicadEdit(true) }); }
        let header = HEADER.with_borrow(|registry| registry.actions.get(id).map(|a| CtlEntry {
            id: id.into(), label: a.label.clone(), ctl: if a.press.is_some() { Ctl::Header(id.into()) } else { Ctl::None },
        }));
        if header.is_some() || HEADER.with_borrow(|r| r.replaces(id)) { return header; }
        if let Some(c) = self.all_controls().find(|c| c.id == id).cloned() { return Some(c); }
        let rects = crate::art::ctl_rects();
        let v = &self.data["thread"]["viewer"];
        if id == "follow-agent" && v["follow"].is_object() && rects.contains_key("follow-agent") {
            let on = v["follow"]["on"] == true;
            /* i18n-ok: scripted e2e control id, not drawn */ return Some(CtlEntry { id: id.into(), label: "Follow agent".into(), ctl: Ctl::Node("follow".into(), if on { "off" } else { "on" }.into()) });
        }
        /* i18n-ok: scripted e2e control id, not drawn */ if id == "ui:vs board" && v["gerbers"] == true && rects.contains_key("ui:vs board") {
            let on = v["gerbersVsBoard"] == true;
            /* i18n-ok: scripted e2e control id, not drawn */ return Some(CtlEntry { id: id.into(), label: "vs board".into(), ctl: Ctl::Node("view".into(), if on { "gerbers" } else { "gerbers\tboard" }.into()) });
        }
        None
    }
    /// Whether the requested viewer page has settled into one of `ready`, `empty`, `error`, `refused`, `disabled`, or `hidden`.
    fn viewer_settled(&self, want: &str, cx: &Context<Self>) -> Option<&'static str> {
        let th = &self.data["thread"];
        let v = &th["viewer"];
        if let Some(st) = rail_row_state(v, want) { return Some(st); }
        let diff_on = th["diff"].is_object();
        let open = v["open"].as_str().unwrap_or("");
        let active = if diff_on { "diff".to_string() } else { crate::screens::viewer::active_kind(v, None) };
        let (press_key, press_painted) = PRESS_SOLID.with(|c| c.borrow().clone());
        let solid_state = |want_key: &str| -> Option<&'static str> {
            let Some(s) = &self.screens.solid else { return None };
            let sr = s.read(cx);
            if !want_key.is_empty() && sr.key() != want_key { return None; }
            if !sr.note.is_empty() { return Some("empty"); }
            if !want_key.is_empty() && sr.image_key() != want_key { return None; }
            let painted_now = sr.painted_count();
            let painted_for_key = if !want_key.is_empty() && press_key != want_key {
                painted_now > press_painted
            } else {
                painted_now > 0
            };
            if sr.stage_text().is_empty() && painted_for_key && sr.stats.last_size.0 > 0 && sr.stats.last_size.1 > 0 {
                return Some("ready");
            }
            None
        };
        let plot_state = |want_key: &str, frame: Option<&crate::screens::plot::Frame>| -> Option<&'static str> {
            let f = frame?;
            if !want_key.is_empty() && !f.key.is_empty() && f.key != want_key { return None; }
            Some(if f.none.is_empty() { "ready" } else { "empty" })
        };
        match want {
            "" => (!diff_on && open.is_empty()).then_some("ready"),
            "diff" => {
                if !diff_on { return None; }
                let d = &th["diff"];
                if d["files"].as_array().is_some_and(|f| !f.is_empty()) { Some("ready") }
                else if !d["summary"].as_str().unwrap_or("").is_empty() { Some("empty") }
                else { None }
            }
            "board" | "schematic" => {
                if diff_on || open != want || v["gerbers"] == true { return None; }
                plot_state(v["key"].as_str().unwrap_or(""), self.screens.plot.frame.as_ref())
            }
            "gerbers" => {
                if !diff_on && open == "board" && v["gerbers"] == true {
                    return plot_state(v["key"].as_str().unwrap_or(""), self.screens.plot.frame.as_ref());
                }
                if !self.error.is_empty() && crate::screens::viewer::pressed(&active).is_none() { return Some("refused"); }
                None
            }
            "3d" => {
                if diff_on || open != "3d" { return None; }
                solid_state(v["key"].as_str().unwrap_or(""))
            }
            "mech" => {
                if diff_on || open != "mech" { return None; }
                let want_key = v["key"].as_str().unwrap_or("");
                if want_key.is_empty() { return Some("empty"); }
                solid_state(want_key)
            }
            "split" => {
                if diff_on || open != "split" { return None; }
                let panes = v["panes"].as_array()?;
                if panes.len() < 3 { return None; }
                let s0 = plot_state(panes[0]["key"].as_str().unwrap_or(""), self.screens.plot.frame_for(panes[0]["key"].as_str().unwrap_or("")))?;
                let s1 = plot_state(panes[1]["key"].as_str().unwrap_or(""), self.screens.plot.frame_for(panes[1]["key"].as_str().unwrap_or("")))?;
                let s2 = solid_state(panes[2]["key"].as_str().unwrap_or(""))?;
                Some(if s0 == "empty" && s1 == "empty" && s2 == "empty" { "empty" } else { "ready" })
            }
            "files" => {
                if diff_on || !matches!(open, "files" | "image" | "pdf" | "text") { return None; }
                let f = &v["files"];
                if !f.is_object() { return Some("ready"); }
                if f["ready"] == false { return None; }
                if !f["error"].as_str().unwrap_or("").is_empty() { return Some("error"); }
                if f["rows"].as_array().is_some_and(|r| r.is_empty()) { Some("empty") } else { Some("ready") }
            }
            "review" => {
                if diff_on || open != "review" || !v["review"].is_object() { return None; }
                if v["review"]["empty"].is_object() {
                    // Require the drawn empty-state text before settling as "empty" (test-mode only).
                    let drawn = crate::screens::viewer::drawn();
                    let empty_drawn = drawn.strip_prefix("review|").unwrap_or("").trim();
                    return (!empty_drawn.is_empty()).then_some("empty");
                }
                // Wait for the Review pane's first paint to populate review_snapshot rather than
                // settling prematurely as "empty" with empty_text="" before mgmt renders.
                let (state, _) = self.screens.mgmt.review_snapshot.as_ref()?;
                (*state != "loading").then_some(*state)
            }
            "browser" => {
                if diff_on || open != "browser" { return None; }
                Some(if self.screens.browser.is_some() { "ready" } else { "error" })
            }
            _ => Some("ready"),
        }
    }
    /// Structured viewer/diff/follow/probe state snapshot for scripted outcome checks.
    fn viewer_state_json(&self, cx: &Context<Self>) -> Value {
        let th = &self.data["thread"];
        let v = &th["viewer"];
        let d = &th["diff"];
        let rects = crate::art::ctl_rects();
        let (_, press_painted) = PRESS_SOLID.with(|c| c.borrow().clone());
        let (solid_key, solid_image_key, solid_shown, solid_note, solid_stage, solid_painted, solid_last_w, solid_last_h) = match &self.screens.solid {
            Some(s) => { let sr = s.read(cx); (sr.key().to_string(), sr.image_key().to_string(), sr.shown_key().to_string(), sr.note.clone(), sr.stage_text(), sr.painted_count(), sr.stats.last_size.0, sr.stats.last_size.1) }
            None => (String::new(), String::new(), String::new(), String::new(), String::new(), 0, 0, 0),
        };
        let (plot_key, plot_none) = match &self.screens.plot.frame {
            Some(f) => (f.key.clone(), f.none.clone()),
            None => (String::new(), String::new()),
        };
        let panes = v["panes"].as_array().map(|a| a.len()).unwrap_or(0);
        let card_ref = |pv: &crate::screens::plotview::PlotView| -> String {
            pv.card.as_ref().and_then(|c| c.rows.iter().find(|r| r.0 == "ref").map(|r| r.1.clone())).unwrap_or_default()
        };
        let split_sch_picked = self.screens.split_pv[0].picked.is_some();
        let split_pcb_picked = self.screens.split_pv[1].picked.is_some();
        let split_sch_lit = !self.screens.split_pv[0].lit.is_empty();
        let split_pcb_lit = !self.screens.split_pv[1].lit.is_empty();
        let split_sch_ref = card_ref(&self.screens.split_pv[0]);
        let split_pcb_ref = card_ref(&self.screens.split_pv[1]);
        let diff_file_names: Vec<String> = d["files"].as_array().into_iter().flatten().filter_map(|f| f["name"].as_str().map(String::from)).collect();
        let rail: Vec<Value> = v["rail"].as_array().into_iter().flatten().map(|r| json!({
            "id": r["id"].as_str().unwrap_or(""),
            "label": r["label"].as_str().unwrap_or(""),
            "state": r["state"].as_str().unwrap_or(""),
            "why": r["why"].as_str().unwrap_or(""),
        })).collect();
        json!({
            "open": v["open"].as_str().unwrap_or(""),
            "active_kind": if d.is_object() { "diff".to_string() } else { crate::screens::viewer::active_kind(v, self.screens.rail_pending.as_deref()) },
            "key": v["key"].as_str().unwrap_or(""),
            "gerbers": v["gerbers"] == true,
            "gerbers_vs_board": v["gerbersVsBoard"] == true,
            "sim_frame": v["simFrame"] == true,
            "rail": rail,
            "panes_count": panes,
            "plot_key": plot_key,
            "plot_none": plot_none,
            "empty_text": if v["open"] == "review" && v["review"]["empty"].is_object() { crate::screens::viewer::drawn().strip_prefix("review|").unwrap_or("").to_owned() }
                else { self.screens.mgmt.review_snapshot.as_ref().filter(|(s, _)| *s == "empty").map(|(_, line)| line.clone()).unwrap_or_default() },
            "solid_key": solid_key,
            "solid_image_key": solid_image_key,
            "solid_shown_key": solid_shown,
            "solid_note": solid_note,
            "solid_stage": solid_stage,
            "solid_painted": solid_painted,
            "solid_painted_since_press": solid_painted.saturating_sub(press_painted),
            "solid_last_w": solid_last_w,
            "solid_last_h": solid_last_h,
            "diff_open": d.is_object(),
            "diff_files": d["files"].as_array().map_or(0, Vec::len),
            "diff_file_names": diff_file_names,
            "diff_summary": d["summary"].as_str().unwrap_or(""),
            "follow_present": v["follow"].is_object(),
            "follow_on": v["follow"]["on"] == true,
            "follow_paused": v["follow"]["paused"] == true,
            "follow_rect_visible": HEADER.with_borrow(|r| r.actions.get("follow-agent").map_or_else(|| rects.contains_key("follow-agent"), |a| rects.contains_key(&a.rect_id))),
            "probe": v["probe"].as_str().unwrap_or(""),
            "split_picked_sch": split_sch_picked,
            "split_picked_pcb": split_pcb_picked,
            "split_lit_sch": split_sch_lit,
            "split_lit_pcb": split_pcb_lit,
            "split_sch_ref": split_sch_ref,
            "split_pcb_ref": split_pcb_ref,
            "error": self.error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn open_settings_is_a_click_on_the_gear_and_paste_image_is_still_not_implemented() {
        let p = crate::tmp::file("script", "s.json");
        std::fs::write(&p, r#"[{"open_settings":true},{"paste_image":true},{"click":"ui:Settings"}]"#).unwrap();
        let steps = load(&p);
        assert_eq!(steps.len(), 3);
        assert!(matches!(&steps[0], Step::Click(id) if id == "ui:Settings"), "{:?}", steps[0]);
        assert!(matches!(&steps[1], Step::Unsupported(k) if k == "paste_image"), "{:?}", steps[1]);
    }
    #[test] fn a_click_with_within_waits_for_its_control_and_a_plain_click_does_not() {
        let p = crate::tmp::file("script", "w.json");
        std::fs::write(&p, r#"[{"click":"ui:Edit","within":8000},{"click":"ui:Edit"}]"#).unwrap();
        let steps = load(&p);
        assert!(matches!(&steps[0], Step::ClickWithin(id, 8000) if id == "ui:Edit"), "{:?}", steps[0]);
        assert!(matches!(&steps[1], Step::Click(id) if id == "ui:Edit"), "{:?}", steps[1]);
    }
    #[test] fn shot_step_loads_name_and_settle_options() {
        let p = crate::tmp::file("script", "shot.json");
        std::fs::write(&p, r#"[{"press":"rail:Board"},{"wait":300},{"shot":"board-300ms"},{"shot":"board","settle":"board","timeout_ms":12000}]"#).unwrap();
        let steps = load(&p);
        assert_eq!(steps.len(), 4);
        assert!(matches!(&steps[0], Step::Press(id) if id == "rail:Board"), "{:?}", steps[0]);
        assert!(matches!(&steps[1], Step::Wait(300)), "{:?}", steps[1]);
        assert!(matches!(&steps[2], Step::Shot { name, settle: None, timeout_ms: 15000 } if name == "board-300ms"), "{:?}", steps[2]);
        assert!(matches!(&steps[3], Step::Shot { name, settle: Some(s), timeout_ms: 12000 } if name == "board" && s == "board"), "{:?}", steps[3]);
    }
    #[test] fn shot_name_validation_accepts_slugs_only() {
        assert!(valid_shot_name("board"));
        assert!(valid_shot_name("split-after-mech-300ms"));
        assert!(!valid_shot_name(""));
        assert!(!valid_shot_name("../escape"));
        assert!(!valid_shot_name("Board"));
        assert!(!valid_shot_name("a/b"));
        assert!(!valid_shot_name("a_b"));
    }
    #[test] fn wait_settled_step_loads_kind_and_tag() {
        let p = crate::tmp::file("script", "ws.json");
        std::fs::write(&p, r#"[{"wait_settled":"board","tag":"task3-start-board","timeout_ms":9000}]"#).unwrap();
        let steps = load(&p);
        assert_eq!(steps.len(), 1);
        assert!(matches!(&steps[0], Step::WaitSettled { kind, tag, timeout_ms: 9000 } if kind == "board" && tag == "task3-start-board"), "{:?}", steps[0]);
    }
    #[test] fn rail_row_state_detects_disabled_and_hidden_entries() {
        let v = json!({
            "rail": [
                {"id": "board", "label": "Board", "state": "disabled", "why": "No board in this project"},
                {"id": "schematic", "label": "Schematic", "state": "ready", "why": ""},
            ]
        });
        assert_eq!(rail_row_state(&v, ""), None);
        assert_eq!(rail_row_state(&v, "board"), Some("disabled"));
        assert_eq!(rail_row_state(&v, "schematic"), None);
        assert_eq!(rail_row_state(&v, "gerbers"), Some("hidden"));
        assert_eq!(rail_row_state(&json!({}), "board"), None);
    }
}

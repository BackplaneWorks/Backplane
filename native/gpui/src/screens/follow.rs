//! "Follow agent" (issue 1229): the viewer's toggle and the line that says what it follows. The hub decides (kicad/follow.rs: `viewer.follow` =
//! `{on, paused, seq, focus}`, the thread's own setting, so the phones see the same); this only brings Backplane's viewer to a new focus (the
//! page the hub's `view` push opened, the sheet, the box, the route's net) and tells the hub when a person's hand moved the view (pause).
use super::plotview::PlotView;
use crate::{hub::Command, i18n::{t, tf, trs}, Backplane};
use serde_json::Value;
use std::sync::Mutex;

/// The focus not yet shown (its page or sheet or frame is on its way) and the last `seq` of a thread seen: what is old news is never replayed.
struct Seen { thread: String, seq: u64, pending: Option<(Value, std::time::Instant)>, pause_sent: bool }
static SEEN: Mutex<Seen> = Mutex::new(Seen { thread: String::new(), seq: 0, pending: None, pause_sent: false });
const PENDING_FOR: std::time::Duration = std::time::Duration::from_secs(15);

/// Whether a person's hand on the view pauses following: it is on and not already paused.
pub fn pause_wanted(f: &Value) -> bool { f["on"] == true && f["paused"] != true }
/// A person panned, zoomed, keyed or clicked this canvas: counted, and the pause to send (once) while it follows.
pub fn hand(pv: &mut PlotView, f: &Value) -> Option<Command> {
    pv.hands = pv.hands.wrapping_add(1);
    if !pause_wanted(f) { return None }
    let mut s = SEEN.lock().unwrap();
    if s.pause_sent { return None }
    s.pause_sent = true; s.pending = None;
    Some(Command::Action("follow".into(), "pause".into()))
}
/// The viewer's follow object of the app, for the handlers.
pub fn of(app: &Backplane) -> Value { app.data["thread"]["viewer"]["follow"].clone() }
/// A person's input on canvas `slot` (pan, zoom, key, click): follow pauses.
pub fn person_input(app: &mut Backplane, slot: usize) {
    let f = of(app);
    if let Some(c) = hand(app.screens.pv(slot), &f) { let _ = app.tx.try_send(c); }
}

/// The text of the line under the toggle: what it follows, or that it is paused. "" when it is off.
pub fn line(f: &Value) -> String {
    if f["on"] != true { return String::new() }
    if f["paused"] == true { return t("gpui.follow.paused") }
    let focus = &f["focus"];
    if !focus.is_object() { return t("gpui.follow.wait") }
    let page = if focus["page"] == "schematic" { trs("Schematic") } else { trs("Board") };
    let refs: Vec<&str> = focus["refs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if focus["kind"] == "route" {
        let near = refs.first().map(|r| format!(" {}", tf("gpui.follow.near", &[("ref", (*r).into())]))).unwrap_or_default();
        return tf("gpui.follow.route", &[("page", page.into()), ("net", focus["net"].as_str().unwrap_or("").into()), ("near", near.into())])
    }
    let what = if refs.is_empty() { focus["net"].as_str().unwrap_or("").to_string() } else { refs.join(", ") };
    tf("gpui.follow.edit", &[("page", page.into()), ("what", what.into())])
}

/// The focus box in plot micrometres (the hub's box is millimetres).
pub fn box_um(focus: &Value) -> Option<[f32; 4]> {
    let b = focus["box"].as_array().filter(|b| b.len() == 4)?;
    let n: Vec<f32> = b.iter().filter_map(|x| x.as_f64()).map(|x| (x * 1000.0) as f32).collect();
    (n.len() == 4 && n[0] < n[2] && n[1] < n[3]).then(|| [n[0], n[1], n[2], n[3]])
}

/// Whether a plot key shows what the focus is of: board (`0|`), or the schematic (`1|`) on the focus's sheet ("" the root).
pub fn key_fits(key: &str, focus: &Value) -> bool {
    let mut p = key.splitn(3, '|');
    let (kind, _, path) = (p.next().unwrap_or(""), p.next(), p.next().unwrap_or(""));
    match focus["page"].as_str() { Some("board") => kind == "0", Some("schematic") => kind == "1" && path == focus["sheet"].as_str().unwrap_or(""), _ => false }
}

/// Called with each picture: a new focus is brought into view once its page is on show; the camera takes it when the plot of that page and sheet is drawn
/// (`take` in plotview::ensure_view).
pub fn tick(app: &mut Backplane) {
    let f = of(app);
    let thread = app.selected.clone();
    let mut s = SEEN.lock().unwrap();
    if !f.is_object() { s.pending = None; return }
    if f["paused"] == true || f["on"] != true { s.pending = None } else { s.pause_sent = false }
    // another thread, or the first sight of one: what it already has is not replayed
    if s.thread != thread { s.thread = thread; s.seq = f["seq"].as_u64().unwrap_or(0); s.pending = None; return }
    let seq = f["seq"].as_u64().unwrap_or(0);
    if seq != s.seq {
        s.seq = seq;
        if f["on"] == true && f["paused"] != true && f["focus"].is_object() { s.pending = Some((f["focus"].clone(), std::time::Instant::now())); }
    }
    let Some((focus, at)) = s.pending.clone() else { return };
    if at.elapsed() > PENDING_FOR { s.pending = None; return }
    drop(s);
    let viewer = &app.data["thread"]["viewer"];
    // the page is the hub's `view` push (the edit's file): wait for it to be on show
    if viewer["open"].as_str() != focus["page"].as_str() { return }
    let sheet = focus["sheet"].as_str().unwrap_or("");
    if focus["page"] == "schematic" {
        let cur = viewer["key"].as_str().unwrap_or("").splitn(3, '|').nth(2).unwrap_or("");
        if cur != sheet { let _ = app.tx.try_send(Command::Action("view-sheet".into(), sheet.into())); }
    }
    if let Some(b) = box_um(&focus) { let pv = app.screens.pv(0); pv.follow_to = Some((b, focus.clone())); }
    if focus["kind"] == "route" {
        if let Some(card) = focus["net"].as_str().filter(|n| !n.is_empty() && !n.contains([';', '\n', '\u{1}'])).and_then(|n| super::plot::card(&format!("track;net={n}"))) { super::plotview::keep_probe(app, Some(&card)); }
    }
    SEEN.lock().unwrap().pending = None;
}

/// The camera takes a waiting focus when the plot drawn is of its page (and sheet): zoomed on its box, no closer than 12 times the whole drawing's fit.
pub fn take(pv: &mut PlotView, frame_key: &str, w: f32, h: f32) {
    let Some((b, focus)) = pv.follow_to.clone() else { return };
    if !key_fits(frame_key, &focus) { return }
    pv.follow_to = None;
    let mut v = super::plot::View::fit(b, w.max(10.), h.max(10.));
    if pv.fitted > 0. { v.scale = v.scale.min(pv.fitted * 12.); }
    pv.view = Some(v); pv.touched = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn a_persons_pan_pauses_following_once_and_only_while_it_follows() {
        let mut pv = PlotView::default();
        let on = json!({"on": true, "paused": false, "seq": 3});
        assert!(hand(&mut pv, &json!({"on": false})).is_none(), "not following: nothing to pause");
        let c = hand(&mut pv, &on);
        assert!(matches!(c, Some(Command::Action(ref a, ref v)) if a == "follow" && v == "pause"), "following: a hand pauses it");
        assert!(hand(&mut pv, &on).is_none(), "the pause is sent once, not at every mouse move");
        assert_eq!(pv.hands, 3);
        SEEN.lock().unwrap().pause_sent = false;
        assert!(hand(&mut pv, &json!({"on": true, "paused": true})).is_none(), "already paused");
        SEEN.lock().unwrap().pause_sent = false;
    }
    #[test] fn the_line_says_what_is_followed_and_when_it_is_paused() {
        assert_eq!(line(&json!({"on": false})), "");
        assert_eq!(line(&json!({"on": true, "paused": true})), t("gpui.follow.paused"));
        let f = json!({"on": true, "paused": false, "focus": {"page": "board", "kind": "route", "net": "+3V3", "refs": ["U5"]}});
        assert_eq!(line(&f), "Following: Board · routed +3V3 near U5");
        let s = json!({"on": true, "focus": {"page": "schematic", "kind": "edit", "refs": ["R1", "C2"]}});
        assert_eq!(line(&s), "Following: Schematic · edited R1, C2");
    }
    #[test] fn a_focus_is_taken_by_the_plot_of_its_page_and_sheet_only() {
        let focus = json!({"page": "schematic", "sheet": "/p/sub.kicad_sch", "box": [0.0, 0.0, 10.0, 10.0]});
        let mut pv = PlotView::default(); pv.follow_to = Some((box_um(&focus).unwrap(), focus.clone()));
        take(&mut pv, "1|/p|", 800., 600.); assert!(pv.view.is_none(), "the root sheet is not that sheet");
        take(&mut pv, "0|/p|", 800., 600.); assert!(pv.view.is_none(), "nor the board");
        take(&mut pv, "1|/p|/p/sub.kicad_sch", 800., 600.);
        let v = pv.view.expect("the sheet's plot"); assert!(pv.touched && pv.follow_to.is_none());
        assert_eq!((v.cx, v.cy), (5000., 5000.));
    }
}

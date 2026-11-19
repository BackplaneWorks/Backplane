//! "Follow agent" (issue 1229): the hub keeps, per thread, whether the person's viewer follows what an agent edits, and where the last edit was.
//! `act` is the toggle (on, off, pause, resume: the viewer's buttons, so the phones see the same state); `record` is called after each saved
//! `kicad_edit` and, while following is on and not paused, makes a focus (page, sheet, box, net, what) that `picture` puts in `thread.viewer.follow`.
//! The viewer reads the focus only when `seq` has moved. A person's pan, zoom or tap pauses it (`pause`); `on` or `resume` starts it again.
//! The embedded KiCad (the thread's page holds it in this hub's window): `pane` brings KiCad's view to the edit with SelectItems, in the
//! frame on show only (an edit in the other frame is said on the line, never shown: KiCad's own cross-probe would move the frame on show),
//! and never over the person: their canvas input pauses Follow as a hand on the viewer does; a modal, a tool mid-operation or a selection of
//! theirs holds the latest edit until it clears (`decide`).
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock, Weak}, time::{Duration, Instant}};

#[derive(Clone, Default)]
struct Follow { on: bool, paused: bool, seq: u64, focus: Value,
    /// when it was last turned on or resumed: the person's input or selection before it never pauses or holds it
    since: Option<Instant>,
    /// the project of the last edit followed, and what the embedded KiCad did with it: "" (not in a pane), "here", "elsewhere", "held"
    pro: String, pane: &'static str }
fn map() -> &'static Mutex<HashMap<String, Follow>> { static M: OnceLock<Mutex<HashMap<String, Follow>>> = OnceLock::new(); M.get_or_init(Default::default) }

/// The toggle's presses: "on" (also after "off": starts again), "off", "pause" (a person moved the view), "resume". False for any other word.
pub fn act(thread: &str, what: &str) -> bool {
    let mut m = map().lock().unwrap();
    if m.len() > 512 && !m.contains_key(thread) { m.clear(); }
    let f = m.entry(thread.to_string()).or_default();
    match what {
        "on" | "resume" => { f.on = true; f.paused = false; f.since = Some(Instant::now()); }
        "off" => { f.on = false; f.paused = false; f.focus = Value::Null; }
        // pausing something that is not on is nothing
        "pause" => { if f.on { f.paused = true } }
        _ => return false,
    }
    true
}
/// Whether an agent's edit in this thread moves the viewer now.
pub fn following(thread: &str) -> bool { map().lock().unwrap().get(thread).is_some_and(|f| f.on && !f.paused) }

/// `thread.viewer.follow`: `{on, paused, seq, focus}`; the focus is `{page, sheet, box: [x0, y0, x1, y1] mm, net, kind, refs}` of the last edit.
pub fn picture(thread: &str) -> Value {
    let f = map().lock().unwrap().get(thread).cloned().unwrap_or_default();
    json!({"on": f.on, "paused": f.paused, "seq": f.seq, "focus": f.focus, "pane": f.pane})
}

/// A saved edit of `path` with this result: the focus is kept when the thread follows; returns it for the caller to show.
pub fn record(thread: &str, path: &str, root_sch: &str, result: &Value) -> Option<Value> {
    if !following(thread) { return None }
    let focus = focus_of(path, root_sch, result)?;
    let mut m = map().lock().unwrap();
    let f = m.get_mut(thread)?;
    if !f.on || f.paused { return None }
    f.seq += 1; f.focus = focus.clone(); f.pane = ""; f.pro = super::ipc::pro_of(path);
    if f.since.is_none() { f.since = Some(Instant::now()) }
    Some(focus)
}

fn pair(v: &Value) -> Option<(f64, f64)> { let a = v.as_array()?; if a.len() != 2 { return None } Some((a[0].as_f64()?, a[1].as_f64()?)) }
/// Every point (mm) a result names: a row's `at`, `from`, `to`, a created symbol's `pins`, and a route's `box`; the refs and nets beside them.
fn walk(v: &Value, pts: &mut Vec<(f64, f64)>, refs: &mut Vec<String>, nets: &mut Vec<String>, routed: &mut Vec<String>) {
    match v {
        Value::Array(a) => for x in a { walk(x, pts, refs, nets, routed) },
        Value::Object(o) => {
            for (k, x) in o {
                match k.as_str() {
                    "at" | "from" | "to" => if let Some(p) = pair(x) { pts.push(p) },
                    "box" => if let Some(a) = x.as_array().filter(|a| a.len() == 4) { if let (Some(a0), Some(a1), Some(a2), Some(a3)) = (a[0].as_f64(), a[1].as_f64(), a[2].as_f64(), a[3].as_f64()) { pts.push((a0, a1)); pts.push((a2, a3)) } },
                    "pins" => if let Some(p) = x.as_object() { for q in p.values() { if let Some(p) = pair(q) { pts.push(p) } } },
                    "ref" => if let Some(r) = x.as_str().filter(|r| !r.is_empty()) { if !refs.iter().any(|y| y == r) { refs.push(r.into()) } },
                    "net" => if let Some(n) = x.as_str().filter(|n| !n.is_empty()) { if !nets.iter().any(|y| y == n) { nets.push(n.into()) } },
                    "routed" => if let Some(a) = x.as_array() { for r in a { if let Some(n) = r["net"].as_str() { if !routed.iter().any(|y| y == n) { routed.push(n.into()) } } } },
                    _ => {}
                }
                walk(x, pts, refs, nets, routed);
            }
        }
        _ => {}
    }
}

/// The focus of one saved edit; None when it names no place (a rule or a net class change, say). `root_sch` is the project's root schematic: a
/// sub-sheet's file is the `sheet` to open, the root is "". Pure.
pub fn focus_of(path: &str, root_sch: &str, result: &Value) -> Option<Value> {
    let board = path.ends_with(".kicad_pcb");
    let (mut pts, mut refs, mut nets, mut routed) = (vec![], vec![], vec![], vec![]);
    walk(&result["results"], &mut pts, &mut refs, &mut nets, &mut routed);
    if pts.is_empty() { return None }
    let (mut b, kind) = ([f64::MAX, f64::MAX, f64::MIN, f64::MIN], if routed.is_empty() { "edit" } else { "route" });
    for (x, y) in &pts { b = [b[0].min(*x), b[1].min(*y), b[2].max(*x), b[3].max(*y)] }
    // some room round it: at least 5 mm, more for a wide edit
    let pad = ((b[2] - b[0]).max(b[3] - b[1]) * 0.25).max(5.0);
    let r = |v: f64| (v * 100.0).round() / 100.0;
    let sheet = if board || path == root_sch { "" } else { path };
    // the part a route ends at, for "near U5": the first ref the result names
    let mut f = json!({"page": if board { "board" } else { "schematic" }, "sheet": sheet, "kind": kind,
        "box": [r(b[0] - pad), r(b[1] - pad), r(b[2] + pad), r(b[3] + pad)], "refs": refs.iter().take(3).collect::<Vec<_>>()});
    if kind == "route" { f["net"] = json!(routed[0]) } else if let Some(n) = nets.first().filter(|_| board) { f["net"] = json!(n) }
    Some(f)
}

// The embedded KiCad --------------------------------------------------------------------------------------------------------------

/// What Follow does with an edit in the embedded KiCad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// select the edit and fit the view to it now
    Now,
    /// the person is in a modal, a tool mid-operation, or has a selection of their own: kept (the latest only) until that clears
    Hold,
    /// the edit is in a frame not on show: said on the line, nothing sent (KiCad's cross-probe would move the frame on show)
    Elsewhere,
    /// the person's hand on KiCad's canvas since Follow last moved it: paused, as on Backplane's viewer
    Pause,
    /// nothing: not following, KiCad not on show, or no SelectItems for that frame
    Not,
}
/// What `decide` reads. `shown`: the frame the thread's pane shows ("" when the page holds no KiCad); `edit`: the edit's frame (PCB, SCH);
/// `open`: KiCad has that frame and SelectItems; the rest since Follow was turned on, resumed or last moved the view.
#[derive(Clone, Copy, Debug, Default)]
pub struct Look<'a> { pub follows: bool, pub shown: &'a str, pub edit: &'a str, pub open: bool, pub input_since: bool, pub modal: bool, pub tool_busy: bool, pub selection_since: bool }
/// Follow in the embedded KiCad, now, later or not at all. Pure.
pub fn decide(l: &Look) -> Step {
    if !l.follows || l.shown.is_empty() { return Step::Not }
    if l.input_since { return Step::Pause }
    if l.shown != l.edit { return Step::Elsewhere }
    if !l.open { return Step::Not }
    if l.modal || l.tool_busy || l.selection_since { return Step::Hold }
    Step::Now
}
/// The frame of an edited document.
pub fn frame_of(path: &str) -> &'static str { if path.ends_with(".kicad_sch") { "SCH" } else { "PCB" } }
/// SelectItems for a focus: its parts, a route's net, and the view fitted to its box (mm -> nm). Pure.
pub fn select_msg(edit: &str, focus: &Value) -> Value {
    let refs: Vec<&str> = focus["refs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let nets: Vec<&str> = if focus["kind"] == "route" { focus["net"].as_str().into_iter().collect() } else { vec![] };
    let mut m = json!({"kind": if edit == "SCH" { "FK_SCH" } else { "FK_PCB" }, "references": refs, "nets": nets, "zoom": true});
    if let Some(b) = focus["box"].as_array().filter(|b| b.len() == 4) {
        let v: Vec<f64> = b.iter().filter_map(Value::as_f64).collect();
        if v.len() == 4 && v[2] > v[0] && v[3] > v[1] {
            let nm = |x: f64| (x * 1e6).round() as i64;
            m["zoom_box_nm"] = json!({"position": {"x_nm": nm(v[0]), "y_nm": nm(v[1])}, "size": {"x_nm": nm(v[2] - v[0]), "y_nm": nm(v[3] - v[1])}});
        }
    }
    m
}
fn num(v: &Value) -> Option<u64> { v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())) }
/// What GetFrames says of the person: when their last canvas input was (any frame: one UI thread serves them all), whether `edit`'s tool
/// is mid-operation, whether KiCad has that frame. `asked`: when GetFrames was sent (an input `ago` before that is no later than it was).
pub fn frames_say(msg: &Value, edit: &str, asked: Instant) -> (Option<Instant>, bool, bool) {
    let mut input: Option<Instant> = None; let (mut busy, mut open) = (false, false);
    for f in msg["frames"].as_array().into_iter().flatten() {
        let kind = super::embed::kind_of_fk_val(&f["kind"]);
        if let Some(ago) = num(&f["canvas_input_ago_ms"]).filter(|a| *a < u64::MAX / 2) {
            if let Some(at) = asked.checked_sub(Duration::from_millis(ago)) { if input.map_or(true, |i| at > i) { input = Some(at) } }
        }
        if kind == edit { open = true; busy = f["tool_busy"].as_bool().unwrap_or(false) }
    }
    (input, busy, open)
}

/// Per project: when Follow last moved its KiCad's view, and the edit held for the person.
#[derive(Default)]
struct Pane { moved: Option<Instant>, held: Option<Held>, worker: bool }
#[derive(Clone)]
struct Held { thread: String, path: String, seq: u64, at: Instant }
fn panes() -> &'static Mutex<HashMap<String, Pane>> { static P: OnceLock<Mutex<HashMap<String, Pane>>> = OnceLock::new(); P.get_or_init(Default::default) }
static HUB: Mutex<Option<Weak<crate::hub::Hub>>> = Mutex::new(None);
/// How long an edit is held for the person at most, and how often a held one is looked at again.
const HOLD_FOR: Duration = Duration::from_secs(300);
const HOLD_EVERY: Duration = Duration::from_millis(300);

/// The frame shown by the pane that holds the project's embedded KiCad, when that window's page is `thread`'s: the window KiCad is
/// attached to (embed), never another window's report (two windows on one hub, a phone on the same thread).
pub fn pane_frame(thread: &str, path: &str) -> Option<String> {
    super::embed::pane_client(&super::ipc::pro_of(path)).and_then(|c| super::watch::pane_frame_of(c, thread))
}
/// Whether `thread`'s page holds the project's embedded KiCad in this hub's window now.
pub fn in_pane(thread: &str, path: &str) -> bool { pane_frame(thread, path).is_some() }
fn set_pane(thread: &str, seq: u64, word: &'static str) -> bool {
    let mut m = map().lock().unwrap();
    match m.get_mut(thread) { Some(f) if f.seq == seq && f.pane != word => { f.pane = word; true } _ => false }
}
fn bump() { if let Some(h) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) { h.bump_views() } }

/// A followed edit of `path` (after `record`) in a thread whose page holds the embedded KiCad: shown, held or said. Returns what was done.
pub async fn pane(hub: &Arc<crate::hub::Hub>, thread: &str, path: &str) -> Step {
    *HUB.lock().unwrap() = Some(Arc::downgrade(hub));
    let Some(seq) = map().lock().unwrap().get(thread).map(|f| f.seq) else { return Step::Not };
    let key = super::embed::norm_pro_key(&super::ipc::pro_of(path));
    // the latest edit replaces one held: never a queue of stale ones
    panes().lock().unwrap().entry(key.clone()).or_default().held = Some(Held { thread: thread.into(), path: path.into(), seq, at: Instant::now() });
    let step = attempt(&key).await;
    if step == Step::Hold {
        let spawn = { let mut p = panes().lock().unwrap(); let e = p.entry(key.clone()).or_default(); !std::mem::replace(&mut e.worker, true) };
        if spawn { tokio::spawn(async move {
            loop {
                tokio::time::sleep(HOLD_EVERY).await;
                if attempt(&key).await == Step::Hold { continue }
                if worker_stops(&key) { break }
            }
        }); }
    }
    step
}

/// Whether the project's hold worker stops now: only with nothing held, decided under the lock `pane` sets `held` and asks for a worker
/// under, so an edit held between the worker's last attempt and here is looked at by this worker, never left for the next edit.
fn worker_stops(key: &str) -> bool {
    let mut p = panes().lock().unwrap();
    match p.get_mut(key) { Some(e) if e.held.is_some() => false, Some(e) => { e.worker = false; true } None => true }
}
/// The held edit `h` is done with: cleared only if it is still the one held (a newer one held meanwhile, an await away, is the next attempt's).
fn drop_held(key: &str, h: &Held) {
    if let Some(p) = panes().lock().unwrap().get_mut(key) { if p.held.as_ref().is_some_and(|x| x.at == h.at && x.thread == h.thread) { p.held = None } }
}
/// One look at the project's held edit: what `decide` says, done.
async fn attempt(key: &str) -> Step {
    let Some(h) = panes().lock().unwrap().get(key).and_then(|p| p.held.clone()) else { return Step::Not };

    let f = map().lock().unwrap().get(&h.thread).cloned().unwrap_or_default();
    if h.at.elapsed() > HOLD_FOR || f.seq != h.seq { drop_held(key, &h); if set_pane(&h.thread, h.seq, "") { bump() } return Step::Not }
    let edit = frame_of(&h.path);
    let route = super::embed::gui_route(&super::ipc::pro_of(&h.path));
    let shown = pane_frame(&h.thread, &h.path).unwrap_or_default();
    let mut look = Look { follows: f.on && !f.paused, shown: &shown, edit, ..Default::default() };
    let since = [f.since, panes().lock().unwrap().get(key).and_then(|p| p.moved)].into_iter().flatten().max();
    let after = |t: Option<Instant>| t.is_some_and(|t| since.map_or(true, |s| t > s));
    if let Some(r) = route.as_ref().filter(|_| look.follows && !shown.is_empty()) {
        let asked = Instant::now();
        let o = super::embed::gui_call(r, "kiapi.backplane.embed.GetFrames", json!({})).await;
        let (input, busy, open) = frames_say(&o["message"], edit, asked);
        look.input_since = after(input); look.tool_busy = busy; look.open = open && super::xprobe::has_select_items(r);
        look.modal = o["status"] == "AS_BUSY" || super::embed::session_state(&r.pro)["dialogs"].as_array().is_some_and(|d| !d.is_empty());
        look.selection_since = after(super::coedit::person_selection_at(&r.pro, if edit == "SCH" { "sch" } else { "pcb" }));
    }
    let mut step = decide(&look);
    if step == Step::Now {
        let r = route.as_ref().unwrap();
        let msg = select_msg(edit, &f.focus);
        let o = super::embed::gui_call(r, "kiapi.backplane.embed.SelectItems", msg.clone()).await;
        eprintln!("follow: SelectItems {} {} -> {}", h.thread, msg, if o["ok"] == true { "ok".to_string() } else { o["status"].as_str().unwrap_or("failed").to_string() });
        if o["ok"] == true { if let Some(p) = panes().lock().unwrap().get_mut(key) { p.moved = Some(Instant::now()) } }
        else if o["status"] == "AS_BUSY" { step = Step::Hold } else { step = Step::Not }
    } else { eprintln!("follow: {step:?} {} {:?}", h.thread, look); }
    let word = match step { Step::Now => "here", Step::Hold => "held", Step::Elsewhere => "elsewhere", _ => "" };
    if step != Step::Hold { drop_held(key, &h) }
    if step == Step::Pause { act(&h.thread, "pause"); bump(); return step }
    if set_pane(&h.thread, h.seq, word) { bump() }
    step
}

/// An agent's explicit viewer_show of `target` ("PCB" / "SCH") while the window's pane shows `shown`: whether the pane is brought to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bring { No, Yes, Busy }
/// Pure: nothing when it is already on show (or the page holds no KiCad); not while the person is in a modal or mid-operation in KiCad.
pub fn bring(target: &str, shown: &str, modal: bool, tool_busy: bool) -> Bring {
    if target.is_empty() || shown.is_empty() || target == shown { return Bring::No }
    if modal || tool_busy { Bring::Busy } else { Bring::Yes }
}
/// An agent's explicit viewer_show of `path` (issue 1355): the frame the window holding the project's KiCad for `thread` is to show
/// (its pane shows another: the person picked it on the strip), or why not now ("" and None: nothing to do).
pub async fn bring_frame(thread: &str, path: &str) -> (Option<&'static str>, String) {
    if !(path.ends_with(".kicad_pcb") || path.ends_with(".kicad_sch")) { return (None, String::new()) }
    let Some(shown) = pane_frame(thread, path) else { return (None, String::new()) };
    let target = frame_of(path);
    let (mut modal, mut busy) = (false, false);
    if target != shown {
        if let Some(r) = super::embed::gui_route(&super::ipc::pro_of(path)) {
            let o = super::embed::gui_call(&r, "kiapi.backplane.embed.GetFrames", json!({})).await;
            modal = o["status"] == "AS_BUSY" || super::embed::session_state(&r.pro)["dialogs"].as_array().is_some_and(|d| !d.is_empty());
            busy = o["message"]["frames"].as_array().into_iter().flatten().any(|f| f["tool_busy"].as_bool() == Some(true));
        }
    }
    let name = |k: &str| if k == "SCH" { "Schematic" } else if k == "PCB" { "Board" } else { "another editor" };
    match bring(target, &shown, modal, busy) {
        Bring::Yes => (Some(target), String::new()),
        Bring::Busy => (None, format!("KiCad is busy ({}): the pane stays on the {} the person has on show", if modal { "a dialog is open" } else { "a tool is mid-operation" }, name(&shown))),
        Bring::No => (None, String::new()),
    }
}

/// The person pressed or typed in the project's pane (editor.kicad.touched): a thread following it whose page holds KiCad pauses when
/// KiCad says that was on its canvas since Follow last moved the view (as a hand on Backplane's viewer pauses it).
pub fn touched(pro: &str) {
    let key = super::embed::norm_pro_key(pro);
    let threads: Vec<(String, Option<Instant>)> = map().lock().unwrap().iter()
        .filter(|(t, f)| f.on && !f.paused && !f.pro.is_empty() && super::embed::norm_pro_key(&f.pro) == key && pane_frame(t, &f.pro).is_some())
        .map(|(t, f)| (t.clone(), f.since)).collect();
    if threads.is_empty() { return }
    let Some(route) = super::embed::gui_route(pro) else { return };
    tokio::spawn(async move {
        // KiCad has the press a moment after the client saw it
        tokio::time::sleep(Duration::from_millis(150)).await;
        let asked = Instant::now();
        let o = super::embed::gui_call(&route, "kiapi.backplane.embed.GetFrames", json!({})).await;
        let (input, _, _) = frames_say(&o["message"], "", asked);
        let moved = panes().lock().unwrap().get(&key).and_then(|p| p.moved);
        let mut any = false;
        for (t, since) in threads {
            let base = [since, moved].into_iter().flatten().max();
            if input.is_some_and(|i| base.map_or(true, |b| i > b)) { eprintln!("follow: the person's input in KiCad pauses {t}"); any |= act(&t, "pause"); }
        }
        if any { if let Some(p) = panes().lock().unwrap().get_mut(&key) { p.held = None } bump() }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_route_focuses_the_board_on_its_box_and_names_the_net() {
        let res = json!({"results": [{"routed": [{"net": "+3V3", "length": 12.0, "vias": 1, "layers": ["F.Cu"], "box": [10.0, 20.0, 30.0, 25.0]}]}], "saved": "/p/b.kicad_pcb"});
        let f = focus_of("/p/b.kicad_pcb", "/p/b.kicad_sch", &res).unwrap();
        assert_eq!((f["page"].clone(), f["kind"].clone(), f["net"].clone(), f["sheet"].clone()), (json!("board"), json!("route"), json!("+3V3"), json!("")));
        let b: Vec<f64> = f["box"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        assert!(b[0] < 10.0 && b[1] < 20.0 && b[2] > 30.0 && b[3] > 25.0, "{b:?}");
    }
    #[test] fn a_subsheet_edit_opens_its_sheet_and_the_root_sheet_opens_none() {
        let res = json!({"results": [{"updated": [{"ref": "R1", "at": [50.0, 60.0], "sheet": "/sub/"}]}]});
        assert_eq!(focus_of("/p/sub.kicad_sch", "/p/b.kicad_sch", &res).unwrap()["sheet"], json!("/p/sub.kicad_sch"));
        let f = focus_of("/p/b.kicad_sch", "/p/b.kicad_sch", &res).unwrap();
        assert_eq!((f["sheet"].clone(), f["page"].clone(), f["refs"].clone()), (json!(""), json!("schematic"), json!(["R1"])));
    }
    #[test] fn an_edit_that_names_no_place_has_no_focus() {
        assert!(focus_of("/p/b.kicad_pcb", "", &json!({"results": [{"updated": [{"name": "PWR", "net": "+1V8"}]}]})).is_none());
    }
    #[test] fn only_a_thread_that_follows_and_is_not_paused_gets_a_focus() {
        let t = "t-follow-test-1"; let res = json!({"results": [{"updated": [{"ref": "U5", "at": [1.0, 2.0]}]}]});
        assert!(record(t, "/p/b.kicad_pcb", "", &res).is_none(), "off by default");
        act(t, "on"); let f = record(t, "/p/b.kicad_pcb", "", &res).expect("on");
        assert_eq!((f["page"].clone(), picture(t)["seq"].clone(), picture(t)["focus"]["page"].clone()), (json!("board"), json!(1), json!("board")));
        act(t, "pause"); assert!(record(t, "/p/b.kicad_pcb", "", &res).is_none(), "paused"); assert_eq!(picture(t)["paused"], json!(true));
        act(t, "resume"); assert!(record(t, "/p/b.kicad_pcb", "", &res).is_some(), "resumed");
        act(t, "off"); assert!(record(t, "/p/b.kicad_pcb", "", &res).is_none(), "off"); act(t, "on"); assert_eq!(picture(t)["paused"], json!(false));
        assert!(!act(t, "dance"));
    }
    // the embedded KiCad (decide: now, later, not at all) --------------------------------------------------------------------------
    fn look<'a>() -> Look<'a> { Look { follows: true, shown: "PCB", edit: "PCB", open: true, ..Default::default() } }
    #[test] fn an_edit_in_the_frame_on_show_is_followed_now() { assert_eq!(decide(&look()), Step::Now); }
    #[test] fn follow_off_or_paused_or_no_kicad_on_show_sends_nothing() {
        assert_eq!(decide(&Look { follows: false, ..look() }), Step::Not, "off (or paused): nothing, whatever KiCad shows");
        assert_eq!(decide(&Look { follows: false, shown: "SCH", ..look() }), Step::Not, "off and the edit elsewhere: nothing either");
        assert_eq!(decide(&Look { shown: "", ..look() }), Step::Not, "the page holds no KiCad: the viewer's Follow, as before");
        assert_eq!(decide(&Look { open: false, ..look() }), Step::Not, "no such frame or no SelectItems");
    }
    #[test] fn an_edit_in_the_other_frame_never_switches_it_and_sends_nothing() {
        assert_eq!(decide(&Look { shown: "SCH", edit: "PCB", ..look() }), Step::Elsewhere);
        assert_eq!(decide(&Look { shown: "PCB", edit: "SCH", ..look() }), Step::Elsewhere);
        assert_eq!(decide(&Look { shown: "PCB_3D", edit: "PCB", ..look() }), Step::Elsewhere, "3D on show: not the board editor");
        assert_eq!(decide(&Look { shown: "SCH", edit: "PCB", modal: true, ..look() }), Step::Elsewhere, "nothing to hold: nothing will be sent");
    }
    #[test] fn the_persons_hand_on_the_canvas_pauses_it_as_on_the_viewer() {
        assert_eq!(decide(&Look { input_since: true, ..look() }), Step::Pause);
        assert_eq!(decide(&Look { input_since: true, shown: "SCH", ..look() }), Step::Pause, "wherever the edit is");
        assert_eq!(decide(&Look { input_since: true, follows: false, ..look() }), Step::Not, "not following: nothing to pause");
    }
    #[test] fn a_modal_a_busy_tool_or_the_persons_selection_holds_it() {
        assert_eq!(decide(&Look { modal: true, ..look() }), Step::Hold);
        assert_eq!(decide(&Look { tool_busy: true, ..look() }), Step::Hold);
        assert_eq!(decide(&Look { selection_since: true, ..look() }), Step::Hold, "SelectItems clears the selection: never over theirs");
    }
    #[test] fn select_items_names_the_parts_and_fits_the_box_in_nm() {
        let f = json!({"page": "board", "kind": "edit", "box": [10.0, 20.0, 30.0, 25.0], "refs": ["U5", "C3"], "net": "GND"});
        let m = select_msg("PCB", &f);
        assert_eq!((m["kind"].clone(), m["references"].clone(), m["nets"].clone()), (json!("FK_PCB"), json!(["U5", "C3"]), json!([])), "a part's net is not selected");
        assert_eq!(m["zoom_box_nm"], json!({"position": {"x_nm": 10_000_000, "y_nm": 20_000_000}, "size": {"x_nm": 20_000_000, "y_nm": 5_000_000}}));
        let r = select_msg("PCB", &json!({"kind": "route", "net": "+3V3", "box": [0.0, 0.0, 1.0, 1.0], "refs": []}));
        assert_eq!(r["nets"], json!(["+3V3"]), "a route selects its net");
        assert_eq!(select_msg("SCH", &json!({"refs": ["R1"]}))["kind"], json!("FK_SCH"));
        assert!(select_msg("PCB", &json!({"refs": ["R1"], "box": [5.0, 5.0, 5.0, 9.0]})).get("zoom_box_nm").is_none(), "an empty box is not sent");
    }
    #[test] fn get_frames_says_the_last_canvas_input_the_tool_and_the_frame() {
        let asked = Instant::now() + Duration::from_secs(10);
        let msg = json!({"frames": [
            {"kind": "FK_PCB", "canvas_input_ago_ms": "2000", "tool_busy": true},
            {"kind": "FK_SCH", "canvas_input_ago_ms": "500"},
            {"kind": "FK_PCB_3D", "canvas_input_ago_ms": "18446744073709551615"}]});
        let (input, busy, open) = frames_say(&msg, "PCB", asked);
        assert_eq!(input, Some(asked - Duration::from_millis(500)), "the latest over every frame");
        assert!(busy && open);
        let (none, busy, open) = frames_say(&json!({"frames": [{"kind": 2, "canvas_input_ago_ms": u64::MAX}]}), "PCB", asked);
        assert!(none.is_none() && !busy && !open, "no input since the frame opened; no board frame");
    }
    #[test] fn a_new_edit_clears_what_kicad_did_with_the_last_and_resume_restarts_the_clock() {
        let t = "t-follow-test-pane"; let res = json!({"results": [{"updated": [{"ref": "U5", "at": [1.0, 2.0]}]}]});
        act(t, "on"); let s0 = map().lock().unwrap()[t].since.unwrap();
        record(t, "/p/b.kicad_pcb", "", &res).unwrap();
        assert!(set_pane(t, 1, "held")); assert_eq!(picture(t)["pane"], json!("held"));
        assert!(!set_pane(t, 0, "here"), "an older edit's outcome never overwrites the newer");
        record(t, "/p/b.kicad_pcb", "", &res).unwrap(); assert_eq!(picture(t)["pane"], json!(""));
        act(t, "pause"); act(t, "resume"); assert!(map().lock().unwrap()[t].since.unwrap() >= s0);
    }
    #[test] fn a_board_edit_is_the_board_editors_and_a_schematic_edit_the_schematics() {
        assert_eq!((frame_of("/p/b.kicad_pcb"), frame_of("/p/sub.kicad_sch")), ("PCB", "SCH"));
    }
    #[test] fn an_edit_held_while_the_worker_stops_is_still_looked_at() {
        let key = "/p/race-test.kicad_pro";
        let held = |thread: &str| Held { thread: thread.into(), path: "/p/race-test.kicad_pcb".into(), seq: 1, at: Instant::now() };
        panes().lock().unwrap().insert(key.into(), Pane { worker: true, ..Default::default() });
        // the worker's last attempt dropped its edit; before it stops, a new edit is held (pane() then finds `worker` still set and spawns none)
        let old = held("t-race"); panes().lock().unwrap().get_mut(key).unwrap().held = Some(old.clone());
        std::thread::sleep(Duration::from_millis(2));
        let new = held("t-race"); panes().lock().unwrap().get_mut(key).unwrap().held = Some(new.clone());
        drop_held(key, &old);
        assert!(panes().lock().unwrap()[key].held.as_ref().is_some_and(|h| h.at == new.at), "the old attempt never drops the newer edit");
        assert!(!worker_stops(key), "something is held: the worker goes on");
        assert!(panes().lock().unwrap()[key].worker, "and keeps its flag, so pane() spawns no second one");
        drop_held(key, &new);
        assert!(worker_stops(key) && !panes().lock().unwrap()[key].worker, "nothing held: it stops and clears the flag, so the next edit spawns one");
        panes().lock().unwrap().remove(key);
    }
    #[test] fn an_explicit_show_brings_the_pane_to_its_frame_unless_it_is_on_show_or_kicad_is_busy() {
        assert_eq!(bring("PCB", "SCH", false, false), Bring::Yes, "the person's strip tab: the agent's show brings the board");
        assert_eq!(bring("SCH", "PCB", false, false), Bring::Yes);
        assert_eq!(bring("PCB", "PCB", false, false), Bring::No, "already on show");
        assert_eq!(bring("PCB", "", false, false), Bring::No, "the page holds no KiCad: the viewer's page push does it");
        assert_eq!(bring("PCB", "SCH", true, false), Bring::Busy, "a modal: said, not done");
        assert_eq!(bring("PCB", "SCH", false, true), Bring::Busy, "mid-drag: said, not done");
        assert_eq!(bring("PCB", "PCB", true, true), Bring::No, "nothing to bring: nothing to refuse");
        assert_eq!(bring("PCB", "PCB_3D", false, false), Bring::Yes, "3D on show: the board editor");
    }
}

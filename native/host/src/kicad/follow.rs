//! "Follow agent" (issue 1229): the hub keeps, per thread, whether the person's viewer follows what an agent edits, and where the last edit was.
//! `act` is the toggle (on, off, pause, resume: the viewer's buttons, so the phones see the same state); `record` is called after each saved
//! `kicad_edit` and, while following is on and not paused, makes a focus (page, sheet, box, net, what) that `picture` puts in `thread.viewer.follow`.
//! The viewer reads the focus only when `seq` has moved. A person's pan, zoom or tap pauses it (`pause`); `on` or `resume` starts it again.
//! Only Backplane's own viewer is driven: a person's embedded KiCad is never touched.
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Mutex, OnceLock}};

#[derive(Clone, Default)]
struct Follow { on: bool, paused: bool, seq: u64, focus: Value }
fn map() -> &'static Mutex<HashMap<String, Follow>> { static M: OnceLock<Mutex<HashMap<String, Follow>>> = OnceLock::new(); M.get_or_init(Default::default) }

/// The toggle's presses: "on" (also after "off": starts again), "off", "pause" (a person moved the view), "resume". False for any other word.
pub fn act(thread: &str, what: &str) -> bool {
    let mut m = map().lock().unwrap();
    if m.len() > 512 && !m.contains_key(thread) { m.clear(); }
    let f = m.entry(thread.to_string()).or_default();
    match what {
        "on" | "resume" => { f.on = true; f.paused = false; }
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
    json!({"on": f.on, "paused": f.paused, "seq": f.seq, "focus": f.focus})
}

/// A saved edit of `path` with this result: the focus is kept when the thread follows; returns it for the caller to show.
pub fn record(thread: &str, path: &str, root_sch: &str, result: &Value) -> Option<Value> {
    if !following(thread) { return None }
    let focus = focus_of(path, root_sch, result)?;
    let mut m = map().lock().unwrap();
    let f = m.get_mut(thread)?;
    if !f.on || f.paused { return None }
    f.seq += 1; f.focus = focus.clone();
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
}

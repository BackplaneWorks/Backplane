//! The viewer panel beside a thread, for the pages the GPUI desktop reads from `thread.viewer`: Board, Schematic and 3D (src/mobile/view.bend `View.open`, `View.json.single`),
//! with the plot itself riding its own channel (`kicad.watch`). Pure functions over the session and `State`. `None`: a case not ported (Files, Mech, Review, a project with several boards, a
//! version or a file opened from the Files tab, a project home, a thread elsewhere), which the caller refuses retryably. Keys GPUI does not read (`hist`, `bg`, `look`...) keep the idle template.
use crate::state::State;
use serde_json::{json, Value};

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn jstr(x: &str) -> String { serde_json::to_string(x).unwrap_or_else(|_| "\"\"".into()) }
fn drawing(kind: &str) -> bool { matches!(kind, "board" | "schematic" | "3d") }

/// Ui.root: the thread's worktree, else its project's folder
fn root_of(st: &State, sel: &str) -> String {
    let Some(t) = st.threads.get(sel) else { return String::new() };
    if !t.worktree.is_empty() { return t.worktree.clone() }
    st.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default()
}
/// View.key: the source a plot carries ("" with no root or no drawing kind)
fn key(kind: &str, root: &str) -> String {
    if root.is_empty() { return String::new() }
    match kind { "board" => format!("0|{root}|"), "schematic" => format!("1|{root}|"), "3d" => format!("2|{root}|"), _ => String::new() }
}

/// A pure case this port draws: no hierarchy of boards, no history, no file opened from the Files tab, no Mech/Review page behind it
fn simple(st: &State, session: &Value, sel: &str) -> bool {
    let (d, info) = (&session["drafts"], &session["info"]);
    !sel.is_empty() && !sel.contains('~') && !sel.starts_with("project-home-") && !sel.starts_with('@') && st.threads.contains_key(sel)
        && !session["panel"].as_bool().unwrap_or(false)
        && s(d, &format!("@view.file:{sel}")).is_empty() && s(d, "@view.bare").is_empty()
        && s(info, &format!("view.path:{sel}")).is_empty() && s(info, &format!("view.kind:{sel}")).is_empty()
        && s(info, &format!("fab.boards:{sel}")).lines().filter(|l| !l.trim().is_empty()).count() <= 1
}

pub struct ViewOut {
    /// the session patch (drafts, rid)
    pub patch: Value,
    /// the commands before the project card's look (`kicad.watch`)
    pub before: Vec<Value>,
    /// the project's boards are asked for after it (`boards.list`) when the viewer opens on a drawing from anything else
    pub boards_look: bool,
}

/// The `view` action for a drawing kind, or "" to close (the phones' idempotent `view`).
pub fn view_action(st: &State, hub: &str, session: &Value, kind: &str) -> Option<ViewOut> {
    if !(drawing(kind) || kind.is_empty()) { return None }
    let sel = s(session, "sel");
    if !simple(st, session, sel) { return None }
    let d = &session["drafts"]; let prev = s(d, &format!("@view:{sel}"));
    if !(prev.is_empty() || drawing(prev)) { return None }
    let root = root_of(st, sel); let want = key(kind, &root); let at = s(d, "@view.at");
    let mut rid = session["rid"].as_u64().unwrap_or(0);
    let mut before = Vec::new();
    let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view.file:{sel}"), json!("")); drafts.insert(format!("@view:{sel}"), json!(kind));
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    if want != at {
        // View.ask: the kind, the root, the path ("" for the project's own file) and the board tree ("")
        let p = format!("{{\"kind\":{},\"root\":{},\"path\":\"\",\"tree\":\"\"}}", jstr(if want.is_empty() { "" } else { kind }), jstr(&root));
        before.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"kicad.watch\",\"p\":{p}}}")}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    Some(ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: drawing(kind) && !drawing(prev) })
}

const BOARD_LAYERS: [(u32, &str); 17] = [(12, "F.Cu"), (11, "In1.Cu"), (10, "In2.Cu"), (9, "In3.Cu"), (8, "In4.Cu"), (7, "In5.Cu"), (6, "In6.Cu"), (5, "B.Cu"), (13, "Pads"), (23, "Vias"), (24, "Holes"),
    (16, "F.Silkscreen"), (3, "B.Silkscreen"), (19, "User.Drawings"), (20, "User.Comments"), (22, "Edge.Cuts"), (21, "Margin")];
const SCH_LAYERS: [&str; 24] = ["page", "sheet fill", "body fill", "sheet", "graphic", "note", "body", "pin", "pin name", "pin number", "field", "field other", "bus", "bus entry", "wire", "no connect",
    "junction", "label", "global label", "hier label", "sheet name", "sheet file", "sheet pin", "dnp"];

/// `thread.viewer`: the idle template with the page the thread's viewer has open. None for a page not ported.
pub fn viewer_json(st: &State, template: &Value, session: &Value) -> Option<Value> {
    let sel = s(session, "sel");
    let kind = s(&session["drafts"], &format!("@view:{sel}"));
    if kind.is_empty() { return Some(template.clone()) }
    if !drawing(kind) || !simple(st, session, sel) { return None }
    let root = root_of(st, sel); let k = key(kind, &root);
    let mut v = template.clone(); let o = v.as_object_mut()?;
    o.insert("open".into(), json!(kind)); o.insert("key".into(), json!(k));
    // View.layers: a 3D view draws the board's layers on its faces
    o.insert("layers".into(), json!(if kind == "3d" { key("board", &root) } else { k }));
    o.insert("layerModes".into(), json!(kind == "board"));
    let rows: Vec<Value> = match kind {
        "board" => BOARD_LAYERS.iter().map(|(n, name)| json!({"layer": n, "name": name, "on": true, "active": false})).collect(),
        "schematic" => SCH_LAYERS.iter().enumerate().map(|(n, name)| json!({"layer": n, "name": name, "on": true, "active": false})).collect(),
        _ => vec![],
    };
    o.insert("layerList".into(), Value::Array(rows));
    let chips: Vec<Value> = [("Board", "board"), ("Schematic", "schematic"), ("3D", "3d"), ("Mech", "mech"), ("Files", "files"), ("Review", "review")].iter().map(|(l, val)| json!({"label": l, "value": val, "on": *val == kind})).collect();
    o.insert("choices".into(), Value::Array(chips));
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fx() -> Value { serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/panel.json"))).unwrap() }
    fn state() -> (State, String) {
        let mut st = State::new();
        for l in include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-log.jsonl")).lines() { st.apply_line(l); }
        (st, "t1700000003-8".into())
    }
    #[test] fn the_view_action_and_the_viewer_of_board_schematic_and_3d_are_the_helpers() {
        let (st, th) = state(); let f = fx();
        let template: Value = serde_json::from_str::<Value>(include_str!("thread_static.json")).unwrap()["viewer"].clone();
        for kind in ["board", "schematic", "3d"] {
            let want = &f[kind];
            let session = json!({"sel": th, "rid": 1, "drafts": {}, "open": {}, "adding": false, "panel": false, "info": {}});
            let o = view_action(&st, "h", &session, kind).unwrap();
            assert_eq!(o.before, want["cmds"].as_array().unwrap()[..1].to_vec(), "{kind}: kicad.watch");
            assert!(o.boards_look, "{kind}");
            for (k, v) in want["drafts"].as_object().unwrap() { if k != "@fab.at" { assert_eq!(&o.patch["drafts"][k], v, "{kind}: {k}"); } }
            // the viewer on the session the action answered
            let mut s2 = session.clone(); crate::session::patch::merge(&mut s2, &o.patch);
            let got = viewer_json(&st, &template, &s2).unwrap();
            for (k, v) in want["viewer"].as_object().unwrap() { assert_eq!(got.get(k), Some(v), "{kind}: viewer.{k}"); }
        }
        // closing sends the stop, and a second open of the same kind sends nothing
        let s_open = json!({"sel": th, "rid": 5, "drafts": {"@view:t1700000003-8": "board", "@view.at": "0|/tmp/soak2|"}, "open": {}, "adding": false, "panel": false, "info": {}});
        assert!(view_action(&st, "h", &s_open, "board").unwrap().before.is_empty());
        let close = view_action(&st, "h", &s_open, "").unwrap();
        assert_eq!(close.before.len(), 1); assert!(!close.boards_look);
        assert!(close.before[0]["text"].as_str().unwrap().contains("\"kind\":\"\""));
        // not ported: Files, a project home, an open Files page behind it
        assert!(view_action(&st, "h", &s_open, "files").is_none());
    }
}

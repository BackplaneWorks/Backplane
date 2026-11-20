//! Legacy single-page parity test against `origin/main`.
//!
//! Generator: run this test against `origin/main` with `UPDATE_PARITY_SNAPSHOT=1` to write
//! `native/host/src/present/testdata/viewer_legacy_parity_main.json`, then run on the branch
//! without that env var to assert byte-for-byte parity of all legacy commands, drafts, info,
//! and picture fields across every single-page flow.
//!
//! No `if let Some` is used anywhere: any unsupported action fails the test immediately.

use super::viewer;
use crate::{session::{pages, patch}, state::State};
use serde_json::{json, Value};

const SNAPSHOT_REL: &str = "src/present/testdata/viewer_legacy_parity_main.json";

fn normalize(v: &Value, root: &str) -> Value {
    match v {
        Value::String(s) => {
            let s = s.replace(root, "<ROOT>");
            Value::String(s)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(|x| normalize(x, root)).collect()),
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, val) in map {
                if matches!(k.as_str(), "madeAt" | "mtime" | "gerberMadeAt") {
                    out.insert(k.clone(), json!("<TS>"));
                    continue;
                }
                out.insert(k.clone(), normalize(val, root));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Strip ONLY the purely additive PR 2a fields (`layout`, `tiles`, and `rail[*].present`)
/// from a viewer picture so every legacy field is compared in full against `origin/main`.
fn strip_additive_2a(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("layout");
        o.remove("tiles");
        if let Some(rail) = o.get_mut("rail").and_then(Value::as_array_mut) {
            for entry in rail {
                if let Some(eo) = entry.as_object_mut() {
                    eo.remove("present");
                }
            }
        }
    }
    v
}

fn snap_step(st: &State, sess: &Value, cmds: &[Value], root: &str) -> Value {
    let v = viewer::viewer_json(st, &json!({}), sess).expect("viewer_json must succeed");
    json!({
        "cmds": normalize(&Value::Array(cmds.to_vec()), root),
        "drafts": normalize(&sess["drafts"], root),
        "info": normalize(&sess["info"], root),
        "viewer": normalize(&strip_additive_2a(v), root),
    })
}

fn record_all_flows() -> Value {
    let td = crate::plat::tmp::dir("viewer-parity-main");
    let root_p = td.path().to_path_buf();
    let root = root_p.to_string_lossy().into_owned();

    // Deterministic hardware project fixture
    std::fs::write(root_p.join("proj.kicad_pro"), "{}").unwrap();
    std::fs::write(root_p.join("board.kicad_pcb"), "(kicad_pcb (version 20240108))").unwrap();
    std::fs::write(root_p.join("board.kicad_sch"), "(kicad_sch (version 20231120))").unwrap();
    std::fs::create_dir_all(root_p.join("sub")).unwrap();
    std::fs::write(root_p.join("sub/child.kicad_sch"), "(kicad_sch (version 20231120))").unwrap();
    std::fs::create_dir_all(root_p.join("alt")).unwrap();
    std::fs::write(root_p.join("alt/alt.kicad_pcb"), "(kicad_pcb (version 20240108))").unwrap();
    std::fs::write(root_p.join("alt/alt.kicad_sch"), "(kicad_sch (version 20231120))").unwrap();
    std::fs::create_dir_all(root_p.join("mech")).unwrap();
    std::fs::write(root_p.join("mech/case.step"), "ISO-10303-21;").unwrap();
    std::fs::write(root_p.join("mech/other.step"), "ISO-10303-21;").unwrap();
    std::fs::create_dir_all(root_p.join("mfg/rev1")).unwrap();
    std::fs::write(root_p.join("mfg/rev1/a-F_Cu.gbr"), "%FSLAX46Y46*%\nM02*\n").unwrap();
    std::fs::create_dir_all(root_p.join("docs")).unwrap();
    std::fs::write(root_p.join("docs/readme.txt"), "hello parity\n").unwrap();

    let mut st = State::new();
    st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Proj","root":root,"at":1}).to_string());
    st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());

    let mut st_far = State::new();
    st_far.apply_line(&json!({"$":"ProjectCreated","id":"peer~p","title":"Remote","root":"/remote/proj","at":1}).to_string());
    st_far.apply_line(&json!({"$":"ThreadCreated","id":"peer~t","project":"peer~p","title":"T","env":"local","branch":"","worktree":"/remote/wt","provider":"claude","model":"","at":2}).to_string());

    let fresh_sess = || json!({
        "sel": "t",
        "rid": 1,
        "drafts": {},
        "panel": false,
        "info": {
            "fab.board:t": "board.kicad_pcb",
            "fab.boards:t": "board.kicad_pcb\nalt/alt.kicad_pcb"
        }
    });

    let mut flows = serde_json::Map::new();

    // 1. open/close/switch
    {
        let mut s = fresh_sess();
        let mut steps = Vec::new();
        for act in ["board", "schematic", "3d", ""] {
            let o = viewer::view_action(&st, "h", &s, act)
                .unwrap_or_else(|| panic!("view_action({act:?}) returned None"));
            patch::merge(&mut s, &o.patch);
            steps.push(snap_step(&st, &s, &o.before, &root));
        }
        flows.insert("open_close_switch".into(), Value::Array(steps));
    }

    // 2. Review from a watched drawing
    {
        let mut s = fresh_sess();
        let mut steps = Vec::new();
        let ob = viewer::view_action(&st, "h", &s, "board").expect("board");
        patch::merge(&mut s, &ob.patch);
        steps.push(snap_step(&st, &s, &ob.before, &root));
        let orv = viewer::view_action(&st, "h", &s, "review").expect("review");
        patch::merge(&mut s, &orv.patch);
        steps.push(snap_step(&st, &s, &orv.before, &root));
        flows.insert("review_from_watched_drawing".into(), Value::Array(steps));
    }

    // 3. Diff
    {
        let mut s = fresh_sess();
        let mut steps = Vec::new();
        let ob = viewer::view_action(&st, "h", &s, "board").expect("board");
        patch::merge(&mut s, &ob.patch);
        steps.push(snap_step(&st, &s, &ob.before, &root));
        let od = pages::act_for("h", &s, "t", "diff", "", || false).expect("diff");
        patch::merge(&mut s, &od.patch);
        steps.push(snap_step(&st, &s, &od.cmds, &root));
        flows.insert("diff".into(), Value::Array(steps));
    }

    // 4. Picked-board Split
    {
        let mut s = fresh_sess();
        let mut steps = Vec::new();
        let ob = viewer::view_action(&st, "h", &s, "board").expect("board");
        patch::merge(&mut s, &ob.patch);
        let op = viewer::board_action(&st, "h", &s, "alt/alt.kicad_pcb").expect("board_action");
        patch::merge(&mut s, &op.patch);
        steps.push(snap_step(&st, &s, &op.before, &root));
        let os = viewer::view_action(&st, "h", &s, "split").expect("split");
        patch::merge(&mut s, &os.patch);
        steps.push(snap_step(&st, &s, &os.before, &root));
        flows.insert("picked_board_split".into(), Value::Array(steps));
    }

    // 5. Child sheet (Schematic + Split)
    {
        let mut s = fresh_sess();
        let mut steps = Vec::new();
        let osch = viewer::view_action(&st, "h", &s, "schematic").expect("schematic");
        patch::merge(&mut s, &osch.patch);
        let osh = viewer::sheet_action(&st, "h", &s, &format!("{root}/sub/child.kicad_sch")).expect("sheet_action sch");
        patch::merge(&mut s, &osh.patch);
        steps.push(snap_step(&st, &s, &osh.before, &root));
        let osp = viewer::view_action(&st, "h", &s, "split").expect("split");
        patch::merge(&mut s, &osp.patch);
        let osh2 = viewer::sheet_action(&st, "h", &s, &format!("{root}/sub/child.kicad_sch")).expect("sheet_action split");
        patch::merge(&mut s, &osh2.patch);
        steps.push(snap_step(&st, &s, &osh2.before, &root));
        flows.insert("child_sheet".into(), Value::Array(steps));
    }

    // 6. Gerbers and vs board
    {
        let mut s = fresh_sess();
        let mut steps = Vec::new();
        let og = viewer::view_action(&st, "h", &s, "gerbers").expect("gerbers");
        patch::merge(&mut s, &og.patch);
        steps.push(snap_step(&st, &s, &og.before, &root));
        let ogb = viewer::view_action(&st, "h", &s, "gerbers\tboard").expect("gerbers vs board");
        patch::merge(&mut s, &ogb.patch);
        steps.push(snap_step(&st, &s, &ogb.before, &root));
        flows.insert("gerbers_and_vs_board".into(), Value::Array(steps));
    }

    // 7. History single and compare
    {
        let mut steps = Vec::new();
        let hist_single = crate::files::rv::hist_git(&format!("{root}/.git"), "1111111", &root, "board.kicad_pcb");
        let mut s1 = fresh_sess();
        s1["drafts"]["@view:t"] = json!("board");
        s1["drafts"]["@view.file:t"] = json!(hist_single);
        steps.push(snap_step(&st, &s1, &[], &root));

        let hist_cmp = format!(
            "cmp\n{}\n{}",
            crate::files::rv::hist_git(&format!("{root}/.git"), "1111111", &root, "board.kicad_pcb"),
            crate::files::rv::hist_git(&format!("{root}/.git"), "2222222", &root, "board.kicad_pcb"),
        );
        let mut s2 = fresh_sess();
        s2["drafts"]["@view:t"] = json!("board");
        s2["drafts"]["@view.file:t"] = json!(hist_cmp);
        steps.push(snap_step(&st, &s2, &[], &root));
        flows.insert("history_single_and_compare".into(), Value::Array(steps));
    }

    // 8. Named Mech part
    {
        let mut s = fresh_sess();
        let mut steps = Vec::new();
        let om = viewer::view_action(&st, "h", &s, "mech\tmech/case.step").expect("mech named");
        patch::merge(&mut s, &om.patch);
        steps.push(snap_step(&st, &s, &om.before, &root));
        let om2 = viewer::mech_action(&st, "h", &s, "mech/other.step", false).expect("mech_action");
        patch::merge(&mut s, &om2.patch);
        steps.push(snap_step(&st, &s, &om2.before, &root));
        flows.insert("named_mech_part".into(), Value::Array(steps));
    }

    // 9. Files preview
    {
        let mut s = fresh_sess();
        let mut steps = Vec::new();
        let of = viewer::view_action(&st, "h", &s, "files").expect("files");
        patch::merge(&mut s, &of.patch);
        steps.push(snap_step(&st, &s, &of.before, &root));
        let ocd = viewer::files_action(&st, "h", &s, "fm-cd", "docs").expect("fm-cd");
        patch::merge(&mut s, &ocd.patch);
        steps.push(snap_step(&st, &s, &ocd.before, &root));
        let oop = viewer::files_action(&st, "h", &s, "fm-open", "docs/readme.txt").expect("fm-open");
        patch::merge(&mut s, &oop.patch);
        steps.push(snap_step(&st, &s, &oop.before, &root));
        flows.insert("files_preview".into(), Value::Array(steps));
    }

    // 10. Far thread
    {
        let mut s = json!({"sel": "peer~t", "rid": 1, "drafts": {}, "panel": false, "info": {}});
        let mut steps = Vec::new();
        for act in ["board", "schematic", "split", ""] {
            let o = viewer::view_action(&st_far, "h", &s, act)
                .unwrap_or_else(|| panic!("far view_action({act:?}) returned None"));
            patch::merge(&mut s, &o.patch);
            steps.push(snap_step(&st_far, &s, &o.before, "/remote/wt"));
        }
        flows.insert("far_thread".into(), Value::Array(steps));
    }

    // 11. Agent show of each kind
    {
        let mut steps = Vec::new();
        for act in ["board", "schematic", "3d", "mech\tmech/case.step", "gerbers", "files", "review"] {
            let mut s = fresh_sess();
            let o = viewer::view_action(&st, "h", &s, act)
                .unwrap_or_else(|| panic!("agent show view_action({act:?}) returned None"));
            patch::merge(&mut s, &o.patch);
            steps.push(snap_step(&st, &s, &o.before, &root));
        }
        flows.insert("agent_show_each_kind".into(), Value::Array(steps));
    }

    Value::Object(flows)
}

#[test]
fn legacy_single_page_flows_match_origin_main_snapshot() {
    let actual = record_all_flows();
    let snap_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT_REL);
    if std::env::var("UPDATE_PARITY_SNAPSHOT").ok().as_deref() == Some("1") {
        std::fs::create_dir_all(snap_path.parent().unwrap()).unwrap();
        std::fs::write(&snap_path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
        return;
    }
    let raw = std::fs::read_to_string(&snap_path)
        .unwrap_or_else(|e| panic!("missing origin/main parity snapshot at {}: {e}", snap_path.display()));
    let expected: Value = serde_json::from_str(&raw).expect("valid snapshot JSON");
    assert_eq!(actual, expected, "legacy single-page flows must match origin/main snapshot in full");
}

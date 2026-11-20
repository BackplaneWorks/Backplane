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
            // history sources carry the repo top's byte length before it (`rv::hist_git`): it varies with the temp folder's name
            let mut parts: Vec<String> = s.split('\t').map(str::to_string).collect();
            for i in 0..parts.len().saturating_sub(1) {
                if !parts[i].is_empty() && parts[i].bytes().all(|b| b.is_ascii_digit()) && parts[i + 1].starts_with("<ROOT>") { parts[i] = "<LEN>".into(); }
            }
            Value::String(parts.join("\t"))
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

fn fixture() -> (crate::plat::tmp::TempDir, String, std::path::PathBuf, State) {
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

    (td, root, root_p, st)
}

/// What an agent's show names, as main's desktop sent it (`view <kind>`; a part rides after a tab).
pub(super) const AGENT_SHOWS: [&str; 9] = ["board", "schematic", "3d", "mech\tmech/case.step", "gerbers", "files", "review", "split", "browser"];

fn record_all_flows() -> Value {
    let (td, root, root_p, st) = fixture();
    let _keep = &td;
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
        let od = pages::act_for("h", &s, "t", "revert", "ask", || false).expect("diff");
        patch::merge(&mut s, &od.session_patch);
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

    // 7. History single and compare: the page on show is a history source (as the desktop's history play leaves it); what a person does next runs main's real commands
    {
        let mut steps = Vec::new();
        let git = format!("{root}/.git");
        let hist_single = crate::files::rv::hist_git(&git, "1111111", &root, "board.kicad_pcb");
        let hist_cmp = format!(
            "cmp\n{}\n{}",
            crate::files::rv::hist_git(&git, "1111111", &root, "board.kicad_pcb"),
            crate::files::rv::hist_git(&git, "2222222", &root, "board.kicad_pcb"),
        );
        for spec in [hist_single, hist_cmp] {
            let mut s = fresh_sess();
            s["drafts"]["@view:t"] = json!("board");
            s["drafts"]["@view.file:t"] = json!(spec);
            s["drafts"]["@view.at"] = json!(format!("0|{root}|{spec}"));
            steps.push(snap_step(&st, &s, &[], &root));
            // Retry, then each press that leaves the history, each from the history on show
            let o = viewer::view_action_at(&st, "h", &s, "retry", 5000).unwrap_or_else(|| panic!("retry on history returned None"));
            let mut after = s.clone(); patch::merge(&mut after, &o.patch);
            steps.push(snap_step(&st, &after, &o.before, &root));
            for act in ["board", "schematic", ""] {
                let o = viewer::view_action(&st, "h", &s, act).unwrap_or_else(|| panic!("view {act:?} from history returned None"));
                let mut after = s.clone(); patch::merge(&mut after, &o.patch);
                steps.push(snap_step(&st, &after, &o.before, &root));
            }
        }
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
        for act in AGENT_SHOWS {
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

/// The generator: run ONLY by `scripts/parity-snapshot.sh` on a checkout of origin/main. It refuses on the branch (a branch-only symbol is in the source) and without the main commit it is run at.
#[test]
#[ignore = "generator: scripts/parity-snapshot.sh on a main checkout"]
fn generate_main_snapshot() {
    assert!(!include_str!("viewer.rs").contains("fn viewer_show_action"), "this tree has branch-only code: the snapshot is generated from origin/main only");
    let commit = std::env::var("PARITY_MAIN_COMMIT").unwrap_or_default();
    assert!(commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()), "PARITY_MAIN_COMMIT must be the main checkout's full commit id");
    let flows = record_all_flows();
    let snap = json!({"generated_from": commit, "flows_sha256": flows_hash(&flows), "flows": flows});
    let snap_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT_REL);
    std::fs::create_dir_all(snap_path.parent().unwrap()).unwrap();
    std::fs::write(&snap_path, serde_json::to_string_pretty(&snap).unwrap() + "\n").unwrap();
}

fn flows_hash(flows: &Value) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(serde_json::to_string(flows).unwrap().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Every path where `main` and `branch` differ, with both values (the failure message names them, never a whole-document dump).
fn diff_paths(main: &Value, branch: &Value, at: &str, out: &mut Vec<String>) {
    match (main, branch) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a { match b.get(k) { Some(w) => diff_paths(v, w, &format!("{at}/{k}"), out), None => out.push(format!("{at}/{k}: main {v} / branch <absent>")) } }
            for (k, w) in b { if !a.contains_key(k) { out.push(format!("{at}/{k}: main <absent> / branch {w}")) } }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => { for (i, (v, w)) in a.iter().zip(b).enumerate() { diff_paths(v, w, &format!("{at}/{i}"), out) } }
        (a, b) => { if a != b { out.push(format!("{at}: main {a} / branch {b}")) } }
    }
}

// ---- branch-only: everything below needs code origin/main lacks; the generator run on main drops it (`sed '/^\/\/ ---- branch-only/,$d'`) ----
/// The checked-in snapshot: it names the main commit it came from and carries its content hash; the test never writes it.
fn load_snapshot() -> Value {
    let snap_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT_REL);
    let raw = std::fs::read_to_string(&snap_path).unwrap_or_else(|e| panic!("missing origin/main parity snapshot at {}: {e}", snap_path.display()));
    let snap: Value = serde_json::from_str(&raw).expect("valid snapshot JSON");
    let from = snap["generated_from"].as_str().unwrap_or("");
    assert!(from.len() == 40 && from.bytes().all(|b| b.is_ascii_hexdigit()), "the snapshot names no main commit: it was not made by scripts/parity-snapshot.sh");
    assert_eq!(snap["flows_sha256"].as_str().unwrap_or(""), flows_hash(&snap["flows"]), "the snapshot was edited by hand or from the branch; regenerate it with scripts/parity-snapshot.sh on origin/main");
    snap
}

#[test]
fn legacy_single_page_flows_match_origin_main_snapshot() {
    let expected = load_snapshot()["flows"].clone();
    let actual = record_all_flows();
    let mut diffs = Vec::new();
    diff_paths(&expected, &actual, "", &mut diffs);
    assert!(diffs.is_empty(), "legacy single-page flows must match the origin/main snapshot in full; differences (main vs branch):\n{}", diffs.join("\n"));
}

/// On this branch the desktop's agent show is `view-show` (`viewer_show_action`), main's was `view <kind>`: from a session with nothing open
/// the pictures, commands and drafts must be main's (the snapshot's `agent_show_each_kind`), except the differences listed here.
#[test]
fn agent_show_from_idle_matches_main_view_for_each_kind() {
    let expected = load_snapshot()["flows"].clone();
    let want = expected["agent_show_each_kind"].as_array().expect("agent_show_each_kind flow");
    let (_td, root, _rp, st) = fixture();
    // the desktop's show: the kind, and the part only for Mech (a tab-joined wire value, as main's `view`)
    let shows: Vec<(&str, &str)> = AGENT_SHOWS.iter().map(|w| w.split_once('\t').unwrap_or((w, ""))).collect();
    assert_eq!(want.len(), shows.len(), "the snapshot flow and this list name the same kinds");
    let mut diffs = Vec::new();
    for (i, (k, path)) in shows.iter().enumerate() {
        let mut s = json!({"sel":"t","rid":1,"drafts":{},"panel":false,"info":{"fab.board:t":"board.kicad_pcb","fab.boards:t":"board.kicad_pcb\nalt/alt.kicad_pcb"}});
        let o = match viewer::viewer_show_action(&st, "h", &s, Some(1), "en", k, path).unwrap_or_else(|| panic!("view-show {k} unsupported")) {
            viewer::LayoutOut::Ok(o) => o,
            viewer::LayoutOut::Refused { text, .. } => panic!("view-show {k} refused: {text}"),
        };
        patch::merge(&mut s, &o.patch);
        let got = snap_step(&st, &s, &o.before, &root);
        // a stored layout never exists for one tile: no `@view.layout` and no layout-only picture fields beyond the stripped additive ones
        diff_paths(&want[i], &got, &format!("/{k}"), &mut diffs);
    }
    assert!(diffs.is_empty(), "view-show differs from main's `view` from idle (main vs branch):\n{}", diffs.join("\n"));
}

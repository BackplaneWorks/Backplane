//! The Rust presenter (docs/PRESENTER-PORT.md). `Routed` is a `Presenter` that asks each ported `Screen` first and falls to the helper
//! for what is not ported yet; a screen takes the same params the helper takes and returns what the helper's V returns.
//! `BACKPLANE_PRESENTER_DUMP=<dir>` writes every `(params, helper output)` pair to `<dir>/views.jsonl` (0600): the input of the differential test.
//! The dump holds message text, drafts and `session`: SYNTHETIC logs only. It is written only by a copy run, only while the helper is running, and only for a log under
//! `copy::BIG_LOG` (a copy of a real store is far larger); `frames.jsonl` (the fold stream) and a frame index per view are the next step and not written yet.
pub mod keys;
pub mod panels;
pub mod tokstats;
pub mod picker;
pub mod thread;
pub mod design_mode;
pub mod layout;
pub mod rail;
pub mod identity_context;
pub mod viewer;
pub mod settings;
pub mod onboard;
pub mod onboard_alwayson;
pub mod onboard_uber;
pub mod signin_there;
pub mod botforms;
pub mod swarmform;
pub mod ready;
pub mod badge;
mod refusal;
pub mod tz;
pub mod window;
#[cfg(test)] mod viewer_parity_tests;
use crate::presenter::{Fut, Presenter};
use anyhow::Result;
use serde_json::{json, Value};
use std::{io::Write, path::PathBuf, sync::{Arc, Mutex}};
pub mod screen;
pub mod merged;
pub mod sections;
pub mod linkopen;
pub mod filelink;
mod native_sidebar;

/// One ported part of the presentation. `None`: not mine, ask the next one (finally the helper).
pub trait Screen: Send + Sync {
    fn name(&self) -> &'static str;
    fn view(&self, params: &Value) -> Option<Value>;
}

pub struct Routed { helper: Arc<dyn Presenter>, screens: Vec<Box<dyn Screen>>, dump: Option<Mutex<std::fs::File>> }

impl Routed {
    pub fn new(helper: Arc<dyn Presenter>, screens: Vec<Box<dyn Screen>>, home: &std::path::Path, helper_running: bool) -> Routed {
        let log = std::fs::metadata(home.join("events.jsonl")).map(|m| m.len()).unwrap_or(0);
        Routed { helper, screens, dump: Self::open_dump(std::env::var_os("BACKPLANE_PRESENTER_DUMP").map(PathBuf::from), crate::copy::on(), helper_running, log) }
    }
    /// Refused (said once) unless: a copy run, the helper is running (nothing to record otherwise), and the log is small (`copy::BIG_LOG`): a dump of a real-size copy would hold real content.
    fn open_dump(dir: Option<PathBuf>, copy: bool, helper_running: bool, log_bytes: u64) -> Option<Mutex<std::fs::File>> {
        use crate::plat::fs::{DirBuilderExt, OpenOptionsExt};
        let dir = dir?;
        if !copy || !helper_running || log_bytes > crate::copy::BIG_LOG {
            eprintln!("BACKPLANE_PRESENTER_DUMP ignored: it needs a copy run (BACKPLANE_COPY=1), a running helper and a log under {} bytes (synthetic logs only)", crate::copy::BIG_LOG);
            return None
        }
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).ok()?;
        match std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(dir.join("views.jsonl")) {
            Ok(f) => Some(Mutex::new(f)),
            Err(e) => { eprintln!("BACKPLANE_PRESENTER_DUMP: {e}"); None }
        }
    }
    fn dump(&self, params: &Value, out: &Value) {
        if let Some(f) = &self.dump { if let Ok(mut f) = f.lock() { let _ = writeln!(f, "{}", json!({"params": params, "out": out})); } }
    }
}

impl Presenter for Routed {
    fn name(&self) -> &'static str { "routed (rust screens, then the helper)" }
    fn fold<'a>(&'a self, frame: &'a Value) -> Fut<'a, ()> { self.helper.fold(frame) }
    fn view<'a>(&'a self, params: &'a Value) -> Fut<'a, Value> {
        Box::pin(async move {
            // a request carrying the hub's own answers (reads: git.diff, mech.list, review pages...) is the helper's: only its Ui applies them, and a Rust screen would draw the
            // thread as it was and drop them
            // (a `fab.look` answer only feeds the header's Buy it, and the thread must not stop answering because the hub looked: every action that moves a thread sends one)
            // (with no helper there is nothing to send the others to: the Rust screens apply them themselves (session/pages.rs), and a view that carries one is answered like any other)
            let reads = params.get("reads").and_then(Value::as_array).map_or(false, |r| r.iter().any(|x| x["kind"] != "fab.look")) && !self.helper.absent();
            if !reads { for s in &self.screens { if let Some(v) = s.view(params) { return Ok(v) } } }
            let out = self.helper.view(params).await?;
            self.dump(params, &out);
            Ok(out)
        })
    }
    fn ping(&self) -> Fut<'_, ()> { self.helper.ping() }
    fn wants_log(&self) -> bool { self.helper.wants_log() }
    fn is_down(&self) -> bool { self.helper.is_down() }
    fn restart(&self) -> Fut<'_, ()> { self.helper.restart() }
    fn why_down(&self) -> Option<String> { self.helper.why_down() }
    fn absent(&self) -> bool { self.helper.absent() }
}

/// The helper-less hub's base presenter: a view nothing ported answers says so (the session turns the error into the retryable PRESENTER_UNAVAILABLE) and
/// nothing is folded into it, so it holds nothing and the hub does not feed it the log (`wants_log` is false).
pub struct NotPorted;
impl Presenter for NotPorted {
    fn name(&self) -> &'static str { "not ported (no helper)" }
    fn fold<'a>(&'a self, _frame: &'a Value) -> Fut<'a, ()> { Box::pin(async { Ok(()) }) }
    fn view<'a>(&'a self, params: &'a Value) -> Fut<'a, Value> {
        Box::pin(async move { anyhow::bail!("not ported: {} {}", params["target"]["kind"].as_str().unwrap_or("?"), params["action"].as_str().unwrap_or("")) })
    }
    fn ping(&self) -> Fut<'_, ()> { Box::pin(async { Ok(()) }) }
    fn wants_log(&self) -> bool { false }
    fn absent(&self) -> bool { true }
}

#[allow(dead_code)]
type _R = Result<()>;

#[cfg(test)]
mod tests {
    use super::*; use crate::presenter::Canned;
    #[test]
    fn viewer_rail_reflects_project_files_on_the_next_picture() {
        let dir = crate::plat::tmp::dir("rail-project");
        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":dir.to_str().unwrap(),"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        let idle = json!({"sel":"t","drafts":{},"panel":false,"info":{}});
        let picture = |session: &Value| viewer::viewer_json(&st, &json!({}), session).unwrap();
        let state = |v: &Value, id: &str| v["rail"].as_array().unwrap().iter().find(|r| r["id"] == id).map(|r| r["state"].as_str().unwrap().to_owned());
        let check = |board, sch, mech| {
            let v = picture(&idle);
            assert_eq!(v["has"], json!({"board":board, "schematic":sch, "mech":mech, "gerbers":false}));
            for (id, enabled) in [("board", board), ("schematic", sch), ("3d", board), ("split", board && sch), ("mech", mech)] {
                assert_eq!(state(&v, id).as_deref(), Some(if enabled { "off" } else { "disabled" }), "{id}");
            }
            assert_eq!(state(&v, "gerbers"), None);
        };
        check(false, false, false);
        let mut session = idle.clone();
        session["drafts"]["@view:t"] = json!("board");
        let open = picture(&session);
        assert_eq!(state(&open, "board").as_deref(), Some("on"));
        assert_eq!(open["has"]["board"], false);
        assert_eq!(state(&picture(&idle), "simulation"), None);
        std::fs::create_dir(dir.join("mech")).unwrap();
        std::fs::write(dir.join("mech/case.step"), "").unwrap();
        check(false, false, true);
        std::fs::remove_file(dir.join("mech/case.step")).unwrap();
        std::fs::write(dir.join("unit.kicad_sch"), "").unwrap();
        check(false, true, false);
        std::fs::write(dir.join("unit.kicad_pcb"), "").unwrap();
        check(true, true, false);
        for page in ["diff", "review", "files", "browser"] {
            let mut session = idle.clone();
            if matches!(page, "diff" | "review") {
                session["info"]["view.kind:t"] = json!(page);
                session["panel"] = json!(true);
            } else { session["drafts"]["@view:t"] = json!(page); }
            assert_eq!(state(&picture(&session), page).as_deref(), Some("on"));
        }
        std::fs::write(dir.join("unit.PrjPcb"), "[Design]").unwrap();
        std::fs::write(dir.join("unit.PcbDoc"), "").unwrap();
        std::fs::write(dir.join("unit.SchDoc"), "").unwrap();
        std::fs::write(dir.join(".backplane.json"), r#"{"project":"unit.PrjPcb","pcb":"unit.PcbDoc","schematic":"unit.SchDoc"}"#).unwrap();
        let v = picture(&idle);
        assert_eq!(v["cad"], "altium");
        check(true, true, false);
        assert_eq!(state(&v, "simulation"), None);
    }

    #[test]
    fn viewer_json_emits_derived_layout_for_one_page_split_gerbers_vs_board_and_history() {
        let d = crate::plat::tmp::dir("viewer-layout");
        let root = d.to_string_lossy().into_owned();
        std::fs::write(d.join("unit.kicad_pcb"), "(kicad_pcb)").unwrap();
        std::fs::write(d.join("unit.kicad_sch"), "(kicad_sch)").unwrap();
        std::fs::create_dir_all(d.join("mfg/rev1")).unwrap();
        std::fs::write(d.join("mfg/rev1/unit-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();

        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());

        let picture = |drafts: Value| {
            let session = json!({"sel":"t","drafts":drafts,"panel":false,"info":{}});
            viewer::viewer_json(&st, &json!({}), &session).unwrap()
        };

        // 0. Idle
        let v_idle = picture(json!({}));
        let l_idle: layout::Layout = serde_json::from_value(v_idle["layout"].clone()).unwrap();
        layout::check_invariants(&l_idle).unwrap();
        assert!(l_idle.is_empty());

        // 1. One page (board)
        let v_one = picture(json!({"@view:t": "board"}));
        let l_one: layout::Layout = serde_json::from_value(v_one["layout"].clone()).unwrap();
        layout::check_invariants(&l_one).unwrap();
        assert_eq!(l_one.tile("t1").unwrap().kind, layout::PageKind::Board);
        assert_eq!(l_one.tile("t1").unwrap().source, "unit.kicad_pcb");

        // 2. Split
        let v_split = picture(json!({"@view:t": "split"}));
        let l_split: layout::Layout = serde_json::from_value(v_split["layout"].clone()).unwrap();
        layout::check_invariants(&l_split).unwrap();
        assert_eq!(l_split.tiles().len(), 3);
        let ctx = layout::WatchContext::from_root(&root);
        let mut resolver = layout::VersionSourceMap::new();
        resolver.register(format!("{root}/.git"), root.clone());
        let watches = layout::plan_watches(&l_split, &ctx, &resolver).unwrap();
        assert_eq!(watches.iter().map(|w| (w.kind, w.key.clone(), w.path.clone())).collect::<Vec<_>>(), vec![
            (1, format!("1|{root}|"), String::new()),
            (0, format!("0|{root}|"), String::new()),
            (2, format!("2|{root}|"), String::new()),
        ]);

        // 3. Gerbers vs board
        let gcmp = crate::files::kicad::gerber::cmp_path(&format!("{root}/mfg/rev1"), &format!("{root}/unit.kicad_pcb"));
        let v_gvb = picture(json!({"@view:t": "board", "@view.gerbers:t": "1", "@view.file:t": gcmp}));
        let l_gvb: layout::Layout = serde_json::from_value(v_gvb["layout"].clone()).unwrap();
        layout::check_invariants(&l_gvb).unwrap();
        let gvb_t = l_gvb.tile("t1").unwrap();
        assert_eq!(gvb_t.kind, layout::PageKind::Board);
        assert_eq!(gvb_t.source, "unit.kicad_pcb");
        assert_eq!(gvb_t.overlay.as_ref().unwrap().kind, layout::PageKind::Gerbers);
        assert_eq!(gvb_t.overlay.as_ref().unwrap().source, "mfg/rev1");

        // 4. History comparison (encoded via real `files::rv::hist_git` with full 40-char SHAs + layer bits)
        let sha_old = "1111aaaa99998888777766665555444433332222";
        let sha_new = "2222bbbb88889999000011112222333344445555";
        let old_wire = crate::files::rv::hist_git(&format!("{root}/.git"), sha_old, &root, "unit.kicad_pcb");
        let new_wire = crate::files::rv::hist_git(&format!("{root}/.git"), sha_new, &root, "unit.kicad_pcb");
        let diff_raw = format!("diff\n{old_wire}\n{new_wire}\n3");
        let v_hist = picture(json!({"@view:t": "board", "@view.file:t": diff_raw}));
        let l_hist: layout::Layout = serde_json::from_value(v_hist["layout"].clone()).unwrap();
        layout::check_invariants(&l_hist).unwrap();
        let ht = l_hist.tile("t1").unwrap();
        assert_eq!(ht.source, "unit.kicad_pcb");
        assert_eq!(ht.version.as_ref().unwrap().id, sha_new);
        assert_eq!(ht.version.as_ref().unwrap().label, "2222bbbb");
        assert_eq!(ht.overlay.as_ref().unwrap().version.as_ref().unwrap().id, sha_old);
        assert_eq!(ht.overlay.as_ref().unwrap().layer_bits, Some(3));
        // Round-trip history comparison through plan_watches matches exact live wire path
        let hist_specs = layout::plan_watches(&l_hist, &ctx, &resolver).unwrap();
        assert_eq!(hist_specs.len(), 1);
        assert_eq!(hist_specs[0].path, diff_raw);
        assert_eq!(hist_specs[0].key, crate::files::kicad::watch::key(0, &root, &diff_raw));

        for v in [&v_one, &v_split, &v_gvb, &v_hist] {
            let s = v["layout"].to_string();
            assert!(!s.contains(&root), "viewer.layout leaked absolute root: {s}");
            assert!(!s.contains("git\\t"), "viewer.layout leaked raw git spec: {s}");
            assert!(!s.contains("\"kicad\""), "viewer.layout leaked transient kicad flag: {s}");
        }
        println!("LAYOUT_JSON_ONE={}", v_one["layout"]);
        println!("LAYOUT_JSON_SPLIT={}", v_split["layout"]);
        println!("LAYOUT_JSON_GVB={}", v_gvb["layout"]);
        println!("LAYOUT_JSON_HIST={}", v_hist["layout"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn viewer_json_emits_layout_for_linked_machine_thread_with_worktree_without_local_probes() {
        let mut st = crate::state::State::new();
        let remote_root = "/nonexistent-remote-machine/project";
        let remote_wt = "/nonexistent-remote-machine/worktrees/feat";
        st.apply_line(&json!({"$":"ProjectCreated","id":"peer~p","title":"Remote","root":remote_root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"peer~t","project":"peer~p","title":"T","env":"local","branch":"feat","worktree":remote_wt,"provider":"claude","model":"","at":2}).to_string());
        // Linked thread with non-empty worktree and picked board in Split: no local probe, no invented sibling schematic
        let session = json!({"sel":"peer~t","drafts":{"@view:peer~t":"split","@view.board:peer~t":"sub/main.kicad_pcb"},"panel":false,"info":{}});
        let v = viewer::viewer_json(&st, &json!({}), &session).unwrap();
        let l: layout::Layout = serde_json::from_value(v["layout"].clone()).unwrap();
        // legacy Split on a picked board watches the schematic beside it, so the migrated layout carries that sibling (no probe of the other machine)
        assert_eq!(l.tiles().len(), 3, "Split on a picked board keeps its sibling schematic on a linked thread");
        assert_eq!(l.tile("pcb").unwrap().source, "sub/main.kicad_pcb");
        assert_eq!(l.tile("sch").unwrap().source, "sub/main.kicad_sch");
        assert!(!v["layout"].to_string().contains(remote_root));
        assert!(!v["layout"].to_string().contains(remote_wt));

        // Remembered board selection on shut viewer and Files emits clean layout with zero sibling probes
        let shut = json!({"sel":"peer~t","drafts":{"@view.board:peer~t":"sub/main.kicad_pcb"},"panel":false,"info":{}});
        let l_shut: layout::Layout = serde_json::from_value(viewer::viewer_json(&st, &json!({}), &shut).unwrap()["layout"].clone()).unwrap();
        assert!(l_shut.is_empty());
        let files = json!({"sel":"peer~t","drafts":{"@view:peer~t":"files","@view.board:peer~t":"sub/main.kicad_pcb"},"panel":false,"info":{}});
        let l_files: layout::Layout = serde_json::from_value(viewer::viewer_json(&st, &json!({}), &files).unwrap()["layout"].clone()).unwrap();
        assert_eq!(l_files.tile("t1").unwrap().kind, layout::PageKind::Files);
    }

    #[test]
    fn plan_watches_matches_live_view_action_and_watch_split_on_all_real_inputs() {
        let d = crate::plat::tmp::dir("watch-parity");
        let root = d.to_string_lossy().into_owned();
        std::fs::write(d.join("main.kicad_pcb"), "(kicad_pcb)").unwrap();
        std::fs::write(d.join("main.kicad_sch"), "(kicad_sch)").unwrap();
        std::fs::write(d.join("power.kicad_sch"), "(kicad_sch)").unwrap();
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("sub/with_sch.kicad_pcb"), "(kicad_pcb)").unwrap();
        std::fs::write(d.join("sub/with_sch.kicad_sch"), "(kicad_sch)").unwrap();
        std::fs::write(d.join("sub/alone.kicad_pcb"), "(kicad_pcb)").unwrap();
        std::fs::create_dir_all(d.join("mfg/rev1")).unwrap();
        std::fs::write(d.join("mfg/rev1/main-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();

        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());

        let ctx = layout::WatchContext::from_root(&root);
        let mut resolver = layout::VersionSourceMap::new();
        resolver.register(format!("{root}/.git"), root.clone());

        let plan_for = |drafts: Value| {
            let session = json!({"sel":"t","drafts":drafts,"panel":false,"info":{}});
            let v = viewer::viewer_json(&st, &json!({}), &session).unwrap();
            let l: layout::Layout = serde_json::from_value(v["layout"].clone()).unwrap();
            (v, l.clone(), layout::plan_watches(&l, &ctx, &resolver).unwrap())
        };
        let sent_watch = |action: &str, base_drafts: Value| {
            let session = json!({"sel":"t","drafts":base_drafts,"panel":false,"info":{}});
            let out = viewer::view_action(&st, "h", &session, action).unwrap();
            let txt = out.before[0]["text"].as_str().unwrap();
            let msg: Value = serde_json::from_str(txt).unwrap();
            (out.patch, msg["p"].clone())
        };
        // Exact mirror of `kicad/watch.rs:160-172` `watch_split` subscription calculation from a live `view_action` payload
        let live_split_specs = |wp: &Value| {
            let r = wp["root"].as_str().unwrap();
            let path = wp["path"].as_str().unwrap_or("");
            let board = wp["board"].as_str().unwrap_or("");
            let f = crate::files::kicad::project::read(r);
            let picked = !board.is_empty();
            let own_sch = if picked {
                board.strip_suffix(".kicad_pcb").map(|b| format!("{b}.kicad_sch")).filter(|p| std::path::Path::new(p).is_file()).unwrap_or_default()
            } else { f.sch.clone() };
            let sch = if path.is_empty() { own_sch.clone() } else { path.to_string() };
            let pcb = if picked { board.to_string() } else { f.pcb.clone() };
            let (sch_at, pcb_at) = if picked { (if path.is_empty() { own_sch.as_str() } else { path }, board) } else { (path, "") };
            let mut out = vec![];
            if !sch.is_empty() { out.push(layout::WatchSpec { kind: 1, key: crate::files::kicad::watch::key(1, r, sch_at), root: r.into(), path: sch_at.into() }); }
            if !pcb.is_empty() {
                out.push(layout::WatchSpec { kind: 0, key: crate::files::kicad::watch::key(0, r, pcb_at), root: r.into(), path: pcb_at.into() });
                out.push(layout::WatchSpec { kind: 2, key: crate::files::kicad::watch::key(2, r, pcb_at), root: r.into(), path: pcb_at.into() });
            }
            out
        };

        // 1. Default Board (live view_action)
        let (_, wp_board) = sent_watch("board", json!({}));
        let (_, _, specs_board) = plan_for(json!({"@view:t": "board"}));
        let pb = wp_board["path"].as_str().unwrap();
        assert_eq!(specs_board, vec![layout::WatchSpec { kind: 0, key: crate::files::kicad::watch::key(0, &root, pb), root: root.clone(), path: pb.into() }]);

        // 2. Standalone 3D (live view_action + watch.rs:132-133)
        let (_, wp_3d) = sent_watch("3d", json!({}));
        let (_, _, specs_3d) = plan_for(json!({"@view:t": "3d"}));
        let p3d = wp_3d["path"].as_str().unwrap();
        assert_eq!(specs_3d, vec![
            layout::WatchSpec { kind: 2, key: crate::files::kicad::watch::key(2, &root, p3d), root: root.clone(), path: p3d.into() },
            layout::WatchSpec { kind: 0, key: crate::files::kicad::watch::key(0, &root, p3d), root: root.clone(), path: p3d.into() },
        ]);

        // 3. Split (default board) vs live view_action("split")
        let (_, wp_split_def) = sent_watch("split", json!({}));
        let (_, _, specs_split_def) = plan_for(json!({"@view:t": "split"}));
        assert_eq!(specs_split_def, live_split_specs(&wp_split_def));

        // 4. Counterexample: Split with EXPLICITLY picked DEFAULT board (`main.kicad_pcb`) keys by absolute path, NOT ""
        let (_, wp_split_exp_def) = sent_watch("split", json!({"@view.board:t": "main.kicad_pcb"}));
        let (_, _, specs_split_exp_def) = plan_for(json!({"@view:t": "split", "@view.board:t": "main.kicad_pcb"}));
        assert_eq!(specs_split_exp_def, live_split_specs(&wp_split_exp_def));
        assert_eq!(specs_split_exp_def[1].path, format!("{root}/main.kicad_pcb"));

        // 5. Split with picked alternate board WITH sibling schematic
        let (_, wp_split_alt) = sent_watch("split", json!({"@view.board:t": "sub/with_sch.kicad_pcb"}));
        let (_, _, specs_alt_sch) = plan_for(json!({"@view:t": "split", "@view.board:t": "sub/with_sch.kicad_pcb"}));
        assert_eq!(specs_alt_sch, live_split_specs(&wp_split_alt));

        // 6. Split with picked alternate board with NO sibling schematic -> Board + 3D ONLY
        let (_, wp_split_alone) = sent_watch("split", json!({"@view.board:t": "sub/alone.kicad_pcb"}));
        let (_, l_alone, specs_alone) = plan_for(json!({"@view:t": "split", "@view.board:t": "sub/alone.kicad_pcb"}));
        assert_eq!(l_alone.tiles().len(), 2);
        assert_eq!(specs_alone, live_split_specs(&wp_split_alone));

        // 7. Child sheet inside Split vs live view_action("retry") on Split with @view.sheet
        let child = format!("{root}/power.kicad_sch");
        let (_, wp_split_child) = sent_watch("retry", json!({"@view:t": "split", "@view.sheet:t": child}));
        let (_, _, specs_split_child) = plan_for(json!({"@view:t": "split", "@view.sheet:t": child}));
        assert_eq!(specs_split_child, live_split_specs(&wp_split_child));

        // 8. History single version & live `viewer::compare_path` diff vs live `viewer_json` key
        let sha_a = "1111aaaa99998888777766665555444433332222";
        let wa = crate::files::rv::hist_git(&format!("{root}/.git"), sha_a, &root, "main.kicad_pcb");
        let (v_sv, _, specs_single_ver) = plan_for(json!({"@view:t": "board", "@view.file:t": wa}));
        assert_eq!(specs_single_ver, vec![layout::WatchSpec { kind: 0, key: v_sv["key"].as_str().unwrap().into(), root: root.clone(), path: wa.clone() }]);
        let cmp_live = viewer::compare_path(&format!("{root}/.git"), &root, sha_a, &format!("{root}/main.kicad_pcb"), 5).unwrap();
        let (v_df, _, specs_diff) = plan_for(json!({"@view:t": "board", "@view.file:t": cmp_live}));
        assert_eq!(specs_diff, vec![layout::WatchSpec { kind: 0, key: v_df["key"].as_str().unwrap().into(), root: root.clone(), path: cmp_live.clone() }]);

        // 9. Gerbers with `noboard\n` via LIVE `viewer::view_action("gerbers\tboard")` on a boardless project
        let d_nb = crate::plat::tmp::dir("watch-parity-noboard");
        let root_nb = d_nb.to_string_lossy().into_owned();
        std::fs::create_dir_all(d_nb.join("mfg/rev1")).unwrap();
        std::fs::write(d_nb.join("mfg/rev1/b-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
        let mut st_nb = crate::state::State::new();
        st_nb.apply_line(&json!({"$":"ProjectCreated","id":"pnb","title":"NoBoard","root":root_nb,"at":1}).to_string());
        st_nb.apply_line(&json!({"$":"ThreadCreated","id":"tnb","project":"pnb","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        let mut sess_nb = json!({"sel":"tnb","drafts":{},"panel":false,"info":{}});
        let out_nb = viewer::view_action(&st_nb, "h", &sess_nb, "gerbers\tboard").unwrap();
        crate::session::patch::merge(&mut sess_nb, &out_nb.patch);
        assert!(out_nb.before.is_empty(), "live view_action sends no watch for noboard Gerbers");
        let v_nb = viewer::viewer_json(&st_nb, &json!({}), &sess_nb).unwrap();
        let l_nb: layout::Layout = serde_json::from_value(v_nb["layout"].clone()).unwrap();
        let specs_nb = layout::plan_watches(&l_nb, &layout::WatchContext::from_root(&root_nb), &resolver).unwrap();
        assert_eq!(v_nb["key"].as_str(), Some(""));
        assert_eq!(l_nb.tile("t1").unwrap().source, "");
        assert!(specs_nb.is_empty());
        let _ = std::fs::remove_dir_all(&d_nb);

        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rail_v_2a_layout_actions_watches_sessions_locks_versions_and_malformed_payloads() {
        let d = crate::plat::tmp::dir("rail-v-2a");
        let root = d.to_string_lossy().into_owned();
        std::fs::write(d.join("unit.kicad_pcb"), "(kicad_pcb)").unwrap();
        std::fs::write(d.join("unit.kicad_sch"), "(kicad_sch)").unwrap();
        std::fs::create_dir_all(d.join("mfg/rev1")).unwrap();
        std::fs::write(d.join("mfg/rev1/unit-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
        std::fs::create_dir_all(d.join("mech")).unwrap();
        std::fs::write(d.join("mech/case.step"), "ISO-10303-21;").unwrap();

        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());

        let watch_msg = |vo: &viewer::ViewOut| -> Option<Value> {
            vo.before.first().and_then(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok()).map(|m| m["p"].clone())
        };

        // 1. Two independent sessions on the same thread have independent per-session layouts
        let mut sess_a = json!({"sel":"t","rid":1,"drafts":{"@view:t":"board","@view.at":format!("0|{root}|")},"panel":false,"info":{}});
        let mut sess_b = json!({"sel":"t","rid":1,"drafts":{"@view:t":"schematic","@view.at":format!("1|{root}|")},"panel":false,"info":{}});

        // Session A adds Gerbers beside Board -> emits exact kicad.watch for Gerbers and updates @view.at
        let out = viewer::layout_action(&st, "h", &sess_a, Some(10), "en", "layout-add", r#"{"side":"right","kind":"gerbers"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo) = out else { panic!("expected Ok") };
        let wp = watch_msg(&vo).expect("layout-add focusing new Gerbers tile must emit kicad.watch");
        assert_eq!(wp["kind"], "board");
        assert_eq!(wp["path"], format!("{root}/mfg/rev1"));
        crate::session::patch::merge(&mut sess_a, &vo.patch);
        assert_eq!(sess_a["drafts"]["@view.at"], format!("0|{root}|{root}/mfg/rev1"));

        // Session B is unaffected (still 1-tile Schematic)
        let vb = viewer::viewer_json(&st, &json!({}), &sess_b).unwrap();
        assert_eq!(vb["layout"]["root"]["kind"], "schematic");

        // Session A picture has both tiles + viewer.rail[].present
        let va = viewer::viewer_json(&st, &json!({}), &sess_a).unwrap();
        let rail_row = |v: &Value, id: &str| v["rail"].as_array().unwrap().iter().find(|r| r["id"] == id).unwrap().clone();
        assert_eq!(rail_row(&va, "board")["present"], true);
        assert_eq!(rail_row(&va, "board")["state"], "off");
        assert_eq!(rail_row(&va, "gerbers")["present"], true);
        assert_eq!(rail_row(&va, "gerbers")["state"], "on");

        // 2. Optimistic close (`view ""`) with surviving sibling closes FOCUSED tile only (Gerbers), switches watch back to Board
        let close_one = viewer::view_action_for(&st, "h", &sess_a, Some(10), "en", "", 0).unwrap();
        let viewer::LayoutOut::Ok(vo_c1) = close_one else { panic!("expected close focused tile Ok") };
        let wp_c1 = watch_msg(&vo_c1).expect("closing Gerbers tile must switch watch back to Board");
        assert_eq!(wp_c1["kind"], "board");
        assert_eq!(wp_c1["path"], "");
        let mut sess_after_c1 = sess_a.clone();
        crate::session::patch::merge(&mut sess_after_c1, &vo_c1.patch);
        let l_c1: layout::Layout = serde_json::from_value(viewer::viewer_json(&st, &json!({}), &sess_after_c1).unwrap()["layout"].clone()).unwrap();
        assert_eq!(l_c1.tiles().len(), 1);
        assert_eq!(l_c1.tile("t1").unwrap().kind, layout::PageKind::Board);

        // Closing the LAST tile stops the watch (`kind: ""`)
        let close_last = viewer::layout_action(&st, "h", &sess_after_c1, Some(10), "en", "layout-close", r#"{"tile":"t1"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_cl) = close_last else { panic!("expected close last Ok") };
        let wp_cl = watch_msg(&vo_cl).expect("closing last tile must stop watch");
        assert_eq!(wp_cl["kind"], "");
        assert_eq!(vo_cl.patch["drafts"]["@view.at"], "");

        // 3. `layout-resize` with Diff up does NOT dismiss Diff and emits ZERO watches
        let mut sess_diff = sess_a.clone();
        sess_diff["panel"] = json!(true);
        sess_diff["info"]["view.kind:t"] = json!("diff");
        let res_diff = viewer::layout_action(&st, "h", &sess_diff, Some(10), "en", "layout-resize", r#"{"split":"s1","ratio":0.6,"bounds":[1000,800]}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_rd) = res_diff else { panic!("expected resize Ok") };
        assert!(vo_rd.before.is_empty(), "layout-resize must emit zero watches");
        assert!(vo_rd.patch.get("panel").is_none(), "layout-resize must NOT call pages::leave or dismiss Diff");

        // 4. Hub-only KiCad guard (`resolve_kicad_guard`): NEVER trusts session["info"];
        // holder's `clientHasKicad` derived by hub; non-holder cannot set locked/kicadTile/clientHasKicad;
        // dialog of another project does not lock this one.
        let l_multi: layout::Layout = serde_json::from_value(va["layout"].clone()).unwrap();
        let g_holder = layout::resolve_kicad_guard(&l_multi, Some(&json!({"holder": 10})), false, Some(10), None, None);
        assert!(g_holder.is_holder && g_holder.client_has_kicad && g_holder.kicad_tile.as_deref() == Some("t1"));
        assert_eq!(layout::add(&l_multi, Some("t1"), layout::Side::Bottom, layout::Tile { id: "t3".into(), kind: layout::PageKind::Simulation, source: "unit.kicad_sch".into(), explicit: false, version: None, unresolved_version: false, overlay: None }, g_holder.client_has_kicad, g_holder.locked.as_deref()), Err(layout::Refusal::OneKicad));
        assert_eq!(layout::Refusal::OneKicad.text_for("ja"), "KiCad タイルは 1 つまでです");
        // Non-holder (99) trying to spoof locked/kicadTile has zero effect
        let g_other = layout::resolve_kicad_guard(&l_multi, Some(&json!({"holder": 10})), true, Some(99), Some("t1"), Some("t1"));
        assert!(!g_other.is_holder && !g_other.client_has_kicad && g_other.locked.is_none() && g_other.kicad_tile.is_none());
        // Holder with hub dialog open -> locked automatically even without client_locked
        let g_dlg = layout::resolve_kicad_guard(&l_multi, Some(&json!({"holder": 10})), true, Some(10), None, None);
        assert_eq!(g_dlg.locked.as_deref(), Some("t1"));
        assert_eq!(layout::Refusal::Locked.text_for("ja"), "KiCad でダイアログが開いています");
        // Request-session info spoofing holder/dialogs is ignored by hub_kicad_guard
        sess_a["info"]["editor.kicad.spoof"] = json!({"holder": 10, "dialogs": ["Fake"], "runtime": {"holder": 10}});
        let spoof_close = viewer::layout_action(&st, "h", &sess_a, Some(10), "en", "layout-close", r#"{"tile":"t1","locked":"t1"}"#).unwrap();
        assert!(matches!(spoof_close, viewer::LayoutOut::Ok(_)), "session info must never forge a KiCad lock");

        // 5. Malformed payloads (bad JSON, unknown kind/side/mode, invalid overlay) return `invalidInput` refusal without mutating state
        for (act, bad_val) in [
            ("layout-add", "{not json"),
            ("layout-add", r#"{"kind":"bogus","side":"right"}"#),
            ("layout-add", r#"{"kind":"board","side":"diagonal"}"#),
            ("layout-overlay", r#"{"base":"t1","overlay":"t2","mode":"bogus"}"#),
            ("layout-overlay", r#"{"base":"t1","overlay":123}"#),
        ] {
            let r = viewer::layout_action(&st, "h", &sess_a, Some(10), "ja", act, bad_val).unwrap();
            assert!(matches!(r, viewer::LayoutOut::Refused { code: "invalidInput", text: "無効なレイアウト要求です", .. }), "{act} {bad_val}");
        }

        // 6. Older existing Gerber set (`mfg/older`) is NOT falsely marked vanished; truly deleted set IS vanished
        std::fs::create_dir_all(d.join("mfg/older")).unwrap();
        std::fs::write(d.join("mfg/older/unit-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
        let mut sess_old_g = json!({"sel":"t","rid":1,"drafts":{"@view:t":"board"},"panel":false,"info":{}});
        let add_old = viewer::layout_action(&st, "h", &sess_old_g, Some(10), "en", "layout-add", r#"{"side":"right","kind":"gerbers","source":"mfg/older"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_og) = add_old else { panic!() };
        crate::session::patch::merge(&mut sess_old_g, &vo_og.patch);
        let v_og = viewer::viewer_json(&st, &json!({}), &sess_old_g).unwrap();
        assert!(v_og["tiles"]["t2"].get("empty").is_none(), "existing non-newest Gerber set must NOT be marked vanished");
        assert_eq!(v_og["tiles"]["t2"]["key"], format!("0|{root}|{root}/mfg/older"));

        std::fs::remove_dir_all(d.join("mfg")).unwrap();
        viewer::refresh_gerber_sets_for_root(&root); // what a watch start does: the picture itself touches no file
        let v_vanished = viewer::viewer_json(&st, &json!({}), &sess_a).unwrap();
        assert_eq!(v_vanished["tiles"]["t2"]["key"], "");
        assert_eq!(v_vanished["tiles"]["t2"]["empty"], "No Gerber files in this project yet");
        assert_eq!(v_vanished["tiles"]["t2"]["identity"]["missing"], true);

        // Unresolved version tile: error state + key = "", and focusing it never watches live artwork
        let mut sess_unres = json!({"sel":"t","rid":1,"drafts":{"@view:t":"board","@view.at":format!("0|{root}|")},"panel":false,"info":{}});
        let bad_ver_layout = layout::add(
            &layout::Layout::single(layout::Tile {
                id: "t1".into(), kind: layout::PageKind::Board, source: "unit.kicad_pcb".into(), explicit: false,
                version: Some(layout::TileVersion { source_id: "vsrc-unknown00000000".into(), id: "1111aaaa99998888777766665555444433332222".into(), label: "1111aaaa".into() }),
                unresolved_version: false, overlay: None,
            }).unwrap(),
            Some("t1"), layout::Side::Right,
            layout::Tile { id: "t2".into(), kind: layout::PageKind::Schematic, source: "unit.kicad_sch".into(), explicit: false, version: None, unresolved_version: false, overlay: None },
            false, None,
        ).unwrap();
        sess_unres["drafts"]["@view.layout:t"] = json!(serde_json::to_string(&bad_ver_layout).unwrap());
        let v_unres = viewer::viewer_json(&st, &json!({}), &sess_unres).unwrap();
        assert_eq!(v_unres["tiles"]["t1"]["key"], "");
        assert_eq!(v_unres["tiles"]["t1"]["error"], "Unresolved version");
        let focus_unres = viewer::layout_action(&st, "h", &sess_unres, Some(10), "en", "layout-focus", r#"{"tile":"t1"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_fu) = focus_unres else { panic!() };
        assert_eq!(watch_msg(&vo_fu).unwrap()["kind"], "", "focusing unresolved version tile must stop watch, NEVER watch live board");

        // 7. Unfocused Mech & Files tiles carry their own rel/dir; multi-tile `sheet_action` updates stored focused tile
        let out_m = viewer::layout_action(&st, "h", &sess_b, Some(11), "en", "layout-add", r#"{"side":"right","kind":"mech","source":"mech/case.step"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_m) = out_m else { panic!() };
        crate::session::patch::merge(&mut sess_b, &vo_m.patch);
        let out_f = viewer::layout_action(&st, "h", &sess_b, Some(11), "en", "layout-focus", r#"{"tile":"t1"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_f) = out_f else { panic!() };
        crate::session::patch::merge(&mut sess_b, &vo_f.patch);
        let vb2 = viewer::viewer_json(&st, &json!({}), &sess_b).unwrap();
        assert_eq!(vb2["tiles"]["t2"]["mech"]["rel"], "mech/case.step");
        assert_eq!(vb2["tiles"]["t2"]["mech"]["name"], "case");
        std::fs::write(d.join("power.kicad_sch"), "(kicad_sch)").unwrap();
        let sh_out = viewer::sheet_action(&st, "h", &sess_b, &format!("{root}/power.kicad_sch")).unwrap();
        crate::session::patch::merge(&mut sess_b, &sh_out.patch);
        let vb_sh = viewer::viewer_json(&st, &json!({}), &sess_b).unwrap();
        assert_eq!(vb_sh["tiles"]["t1"]["source"], "power.kicad_sch");

        // 8. Linked thread Split with child sheet
        let mut st_far = crate::state::State::new();
        st_far.apply_line(&json!({"$":"ProjectCreated","id":"peer~p","title":"Remote","root":"/remote/proj","at":1}).to_string());
        st_far.apply_line(&json!({"$":"ThreadCreated","id":"peer~t","project":"peer~p","title":"T","env":"local","branch":"","worktree":"/remote/wt","provider":"claude","model":"","at":2}).to_string());
        let sess_far = json!({"sel":"peer~t","rid":1,"drafts":{"@view:peer~t":"split","@view.sheet:peer~t":"/remote/wt/power.kicad_sch"},"panel":false,"info":{}});
        let v_far = viewer::viewer_json(&st_far, &json!({}), &sess_far).unwrap();
        assert_eq!(v_far["layout"]["root"]["a"]["source"], "power.kicad_sch");

        // 9. Idempotent `view-show` (`viewer_show_action`) NEVER closes an already-focused tile (multi-tile or legacy Files)
        assert!(viewer::show_refusal("mech", "mech/case.step", &[root.clone()]).is_none());
        let vs1 = viewer::viewer_show_action(&st, "h", &sess_b, Some(11), "en", "mech", &format!("{root}/mech/case.step")).unwrap();
        let viewer::LayoutOut::Ok(vo_vs1) = vs1 else { panic!() };
        crate::session::patch::merge(&mut sess_b, &vo_vs1.patch);
        // Repeat show of already-focused Mech -> must stay open (2 tiles, focused on t2), NEVER close!
        let vs2 = viewer::viewer_show_action(&st, "h", &sess_b, Some(11), "en", "mech", &format!("{root}/mech/case.step")).unwrap();
        let viewer::LayoutOut::Ok(vo_vs2) = vs2 else { panic!() };
        crate::session::patch::merge(&mut sess_b, &vo_vs2.patch);
        let vb3 = viewer::viewer_json(&st, &json!({}), &sess_b).unwrap();
        assert_eq!(vb3["layout"]["focus"], "t2");
        assert_eq!(vb3["tiles"].as_object().unwrap().len(), 2);

        // Legacy single-tile mode: repeated `view-show files` does NOT close Files
        let sess_files = json!({"sel":"t","rid":1,"drafts":{"@view:t":"files"},"panel":false,"info":{}});
        let vs_files = viewer::viewer_show_action(&st, "h", &sess_files, None, "en", "files", "").unwrap();
        let viewer::LayoutOut::Ok(vo_vf) = vs_files else { panic!() };
        assert!(vo_vf.patch.as_object().unwrap().is_empty(), "repeated view-show files is an idempotent no-op");

        // 10. Legacy replay parity: with no stored `@view.layout:t`, legacy actions NEVER write `@view.layout:t`
        // and Review from a watched drawing emits exactly ONE stop command.
        let mut s_leg = json!({"sel":"t","rid":1,"drafts":{},"panel":false,"info":{}});
        for act in ["board", "schematic", "3d", "split", "mech", "files", "review", ""] {
            if act == "review" {
                let ob = viewer::view_action(&st, "h", &s_leg, "board").unwrap();
                crate::session::patch::merge(&mut s_leg, &ob.patch);
                let orv = viewer::view_action(&st, "h", &s_leg, "review").unwrap();
                assert!(orv.patch["drafts"].get("@view.layout:t").is_none(), "legacy review must not write @view.layout");
                let watch_stops = orv.before.iter().filter(|c| c["text"].as_str().is_some_and(|t| t.contains("\"kicad.watch\""))).count();
                assert_eq!(watch_stops, 1, "Review from watched drawing must emit exactly ONE watch stop");
                crate::session::patch::merge(&mut s_leg, &orv.patch);
                continue;
            }
            let o = viewer::view_action(&st, "h", &s_leg, act).unwrap_or_else(|| panic!("legacy action {act:?} must be supported"));
            assert!(o.patch["drafts"].get("@view.layout:t").is_none(), "legacy action {act:?} must never write @view.layout");
            crate::session::patch::merge(&mut s_leg, &o.patch);
        }

        // 11. Item 2: Entering multi-tile from legacy Review preserves BOTH Review and Board;
        // linked migration with picked board preserves picked sibling schematic.
        let mut s_rev = json!({"sel":"t","rid":1,"drafts":{},"panel":false,"info":{}});
        let (p_rev, _) = crate::session::pages::open_review(&s_rev, "h", "t", "bom", false);
        crate::session::patch::merge(&mut s_rev, &p_rev);
        assert_eq!(s_rev["drafts"]["@view:t"], "");
        assert_eq!(s_rev["info"]["view.kind:t"], "review");
        let add_from_rev = viewer::layout_action(&st, "h", &s_rev, Some(10), "en", "layout-add", r#"{"side":"right","kind":"board"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_afr) = add_from_rev else { panic!() };
        crate::session::patch::merge(&mut s_rev, &vo_afr.patch);
        let v_afr = viewer::viewer_json(&st, &json!({}), &s_rev).unwrap();
        let kinds_afr: Vec<&str> = v_afr["tiles"].as_object().unwrap().values().filter_map(|t| t["kind"].as_str()).collect();
        assert!(kinds_afr.contains(&"review") && kinds_afr.contains(&"board"), "entering multi-tile from Review must keep both Review and Board: {kinds_afr:?}");

        let sess_far_picked = json!({"sel":"peer~t","rid":1,"drafts":{"@view:peer~t":"split","@view.board:peer~t":"alt/alt.kicad_pcb"},"panel":false,"info":{}});
        let add_far = viewer::layout_action(&st_far, "h", &sess_far_picked, Some(10), "en", "layout-add", r#"{"side":"right","kind":"files"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_af) = add_far else { panic!() };
        let mut s_far2 = sess_far_picked.clone();
        crate::session::patch::merge(&mut s_far2, &vo_af.patch);
        let v_far2 = viewer::viewer_json(&st_far, &json!({}), &s_far2).unwrap();
        assert!(v_far2["tiles"].as_object().unwrap().values().any(|t| t["kind"] == "schematic" && t["source"] == "alt/alt.kicad_sch"), "linked migration must preserve picked sibling schematic: {}", v_far2["tiles"]);

        // 12. Item 3: Leaving multi-tile to 1 tile clears stale @view.layout, @view.file, @view.sheet, @view.gerbers
        let mut s_leave = json!({"sel":"t","rid":1,"drafts":{"@view:t":"schematic","@view.sheet:t":format!("{root}/power.kicad_sch"),"@view.file:t":format!("{root}/stale.kicad_pcb"),"@view.gerbers:t":"1"},"panel":false,"info":{}});
        let add_l = viewer::layout_action(&st, "h", &s_leave, Some(10), "en", "layout-add", r#"{"side":"right","kind":"diff"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_al) = add_l else { panic!() };
        crate::session::patch::merge(&mut s_leave, &vo_al.patch);
        let close_sch = viewer::layout_action(&st, "h", &s_leave, Some(10), "en", "layout-close", r#"{"tile":"t1"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_cd) = close_sch else { panic!() };
        crate::session::patch::merge(&mut s_leave, &vo_cd.patch);
        assert_eq!(s_leave["drafts"]["@view.layout:t"], "");
        assert_eq!(s_leave["drafts"]["@view.sheet:t"], "");
        assert_eq!(s_leave["drafts"]["@view.file:t"], "");
        assert_eq!(s_leave["drafts"]["@view.gerbers:t"], "");

        // 13. Item 4: Unresolved historical tile surviving handoff stays stored & error; Retry never watches live board
        let mut s_unres_surv = sess_unres.clone();
        let close_live = viewer::layout_action(&st, "h", &s_unres_surv, Some(10), "en", "layout-close", r#"{"tile":"t2"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_cl) = close_live else { panic!() };
        crate::session::patch::merge(&mut s_unres_surv, &vo_cl.patch);
        assert!(!s_unres_surv["drafts"]["@view.layout:t"].as_str().unwrap_or("").is_empty(), "unresolved survivor must retain stored layout");
        let v_us = viewer::viewer_json(&st, &json!({}), &s_unres_surv).unwrap();
        assert_eq!(v_us["key"], "");
        assert_eq!(v_us["error"], "Unresolved version");
        let retry_us = viewer::view_action_for(&st, "h", &s_unres_surv, Some(10), "en", "retry", 5000).unwrap();
        let viewer::LayoutOut::Ok(vo_ru) = retry_us else { panic!() };
        assert!(watch_msg(&vo_ru).is_none_or(|p| p["kind"] == ""), "Retry on unresolved survivor must NEVER watch live artwork");

        // 14. Item 5: Far Review watch stop carries `on`
        let s_far_rev = json!({"sel":"peer~t","rid":1,"drafts":{"@view:peer~t":"board","@view.at":"0|/remote/wt|"},"panel":false,"info":{}});
        let (_, far_rev_cmds) = crate::session::pages::open_review(&s_far_rev, "h", "peer~t", "bom", false);
        let far_stop = far_rev_cmds.iter().find_map(|c| {
            let v: Value = serde_json::from_str(c["text"].as_str()?).ok()?;
            (v["m"] == "kicad.watch").then_some(v["p"].clone())
        }).expect("far review must stop active watch");
        assert_eq!(far_stop["on"], "peer");

        // 15. Item 6: resolve_kicad_guard never trusts client_locked when hub_dialog_open == false;
        // stored Split sheet change with non-Schematic leaf focused updates stored schematic leaf.
        let g_no_dlg = layout::resolve_kicad_guard(&l_multi, Some(&json!({"holder": 10})), false, Some(10), Some("t1"), Some("t1"));
        assert_eq!(g_no_dlg.locked, None, "client_locked must never lock when hub_dialog_open is false");

        let mut s_sp = json!({"sel":"t","rid":1,"drafts":{"@view:t":"split"},"panel":false,"info":{}});
        let mut l_sp = layout::derive_layout(&layout::DeriveInput { root: &root, open: "split", default_pcb: &format!("{root}/board.kicad_pcb"), default_sch: &format!("{root}/board.kicad_sch"), ..Default::default() });
        l_sp.focus = Some("pcb".into()); // Board focused, NOT Schematic!
        s_sp["drafts"]["@view.layout:t"] = json!(serde_json::to_string(&l_sp).unwrap());
        let sh_sp = viewer::sheet_action_for(&st, "h", &s_sp, None, "en", &format!("{root}/power.kicad_sch")).unwrap();
        let viewer::LayoutOut::Ok(vo_shsp) = sh_sp else { panic!() };
        crate::session::patch::merge(&mut s_sp, &vo_shsp.patch);
        let v_shsp = viewer::viewer_json(&st, &json!({}), &s_sp).unwrap();
        assert_eq!(v_shsp["tiles"]["sch"]["source"], "power.kicad_sch", "sheet change in stored Split with Board focused must update Schematic leaf");

        // 16. A plain `view` is always main's `view`: right after an agent's push it still toggles Files closed, as on main (nothing is inferred)
        let sess_old_files = json!({"sel":"t","rid":1,"drafts":{"@view:t":"files"},"panel":false,"info":{}});
        let _tok = viewer::issue_show("t", "files", "");
        let old_ans = viewer::view_action_for(&st, "h", &sess_old_files, Some(42), "en", "files", 0).unwrap();
        let viewer::LayoutOut::Ok(vo_oa) = old_ans else { panic!() };
        assert_eq!(vo_oa.patch["drafts"]["@view:t"], "", "a plain `view files` right after a push closes Files, as on main");

        // 17. Item 8: Empty Gerber directory (no `.gbr` members) is vanished via contained member check
        std::fs::create_dir_all(d.join("mfg/empty_rev")).unwrap();
        let mut s_g = json!({"sel":"t","rid":1,"drafts":{"@view:t":"board"},"panel":false,"info":{}});
        let add_g = viewer::layout_action(&st, "h", &s_g, None, "en", "layout-add", r#"{"side":"right","kind":"gerbers","source":"mfg/empty_rev"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_ag) = add_g else { panic!() };
        crate::session::patch::merge(&mut s_g, &vo_ag.patch);
        let v_g = viewer::viewer_json(&st, &json!({}), &s_g).unwrap();
        assert_eq!(v_g["tiles"]["t2"]["key"], "");
        assert_eq!(v_g["tiles"]["t2"]["identity"]["missing"], true);

        // 18. Item 9: Unfocused tiles and ALL historical tiles never expose absolute paths, `.git`, or repo top
        let v_no_abs = viewer::viewer_json(&st, &json!({}), &sess_b).unwrap();
        let focus_id = v_no_abs["layout"]["focus"].as_str().unwrap().to_string();
        let mut unfocused = 0;
        for (id, tile) in v_no_abs["tiles"].as_object().unwrap() {
            if *id == focus_id { continue }
            unfocused += 1;
            let k = tile["key"].as_str().unwrap_or("");
            assert!(!k.contains(&root) && !k.contains(".git") && !k.starts_with('/'), "unfocused tile {id} key must not expose an absolute path or .git: {k}");
            let l = tile["layers"].as_str().unwrap_or("");
            assert!(!l.contains(&root) && !l.contains(".git"), "unfocused tile {id} layers must not expose an absolute path: {l}");
        }
        assert!(unfocused >= 1, "the check must cover an unfocused tile");
        // a resolved historical tile (focused or not) carries an opaque key: never the repo top, the git dir or an absolute path
        let sid = layout::opaque_source_id(&format!("{root}/.git"), &root);
        let hist_tile = |id: &str, kind| layout::Tile { id: id.into(), kind, source: "unit.kicad_pcb".into(), explicit: false,
            version: Some(layout::TileVersion { source_id: sid.clone(), id: "1111aaaa99998888777766665555444433332222".into(), label: "1111aaaa".into() }),
            unresolved_version: false, overlay: None };
        for focus in ["t1", "t2"] {
            let mut hl = layout::add(&layout::Layout::single(hist_tile("t1", layout::PageKind::Board)).unwrap(), Some("t1"), layout::Side::Right,
                layout::Tile { id: "t2".into(), kind: layout::PageKind::Schematic, source: "unit.kicad_sch".into(), explicit: false, version: None, unresolved_version: false, overlay: None }, false, None).unwrap();
            hl.focus = Some(focus.into());
            let sess_h = json!({"sel":"t","rid":1,"drafts":{"@view:t":"board","@view.layout:t":serde_json::to_string(&hl).unwrap()},"panel":false,"info":{}});
            let vh = viewer::viewer_json(&st, &json!({}), &sess_h).unwrap();
            let hk = vh["tiles"]["t1"]["key"].as_str().unwrap();
            assert!(!hk.is_empty() && !hk.contains(&root) && !hk.contains(".git") && !hk.starts_with('/'), "historical tile key (focus {focus}) must be opaque and resolved: {hk}");
            assert!(vh["tiles"]["t1"].get("error").is_none());
        }
        let _ = std::fs::remove_dir_all(&d);
    }
    /// One writer for every survivor (C), Retry on a resolved historical tile (D) and opaque historical identities (F).
    #[test]
    fn rail_v_2a_survivors_retry_and_historical_identities() {
        let d = crate::plat::tmp::dir("rail-v-2a-surv");
        let root = d.to_string_lossy().into_owned();
        for f in ["unit.kicad_pcb", "unit.kicad_sch", "power.kicad_sch"] { std::fs::write(d.join(f), "()").unwrap(); }
        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        let tile = |id: &str, kind, source: &str, explicit| layout::Tile { id: id.into(), kind, source: source.into(), explicit, version: None, unresolved_version: false, overlay: None };
        let watch = |vo: &viewer::ViewOut| -> Option<Value> { vo.before.iter().find_map(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok().filter(|m| m["m"] == "kicad.watch")).map(|m| m["p"].clone()) };

        // C: a child Schematic closed while Review survives: nothing of the sheet stays, and the layout is gone
        let mut l = layout::add(&layout::Layout::single(tile("t1", layout::PageKind::Review, "", false)).unwrap(), Some("t1"), layout::Side::Right,
            tile("t2", layout::PageKind::Schematic, "power.kicad_sch", true), false, None).unwrap();
        l.focus = Some("t2".into());
        let sheet = format!("{root}/power.kicad_sch");
        let mut s = json!({"sel":"t","rid":1,"drafts":{"@view:t":"schematic","@view.sheet:t":sheet,"@view.at":format!("1|{root}|{sheet}"),"@view.layout:t":serde_json::to_string(&l).unwrap()},"panel":false,"info":{}});
        let viewer::LayoutOut::Ok(vo) = viewer::layout_action(&st, "h", &s, Some(1), "en", "layout-close", r#"{"tile":"t2"}"#).unwrap() else { panic!("close refused") };
        assert!(watch(&vo).is_some_and(|p| p["kind"] == ""), "the child sheet's watch stops");
        crate::session::patch::merge(&mut s, &vo.patch);
        for k in ["@view.layout:t", "@view.sheet:t", "@view.file:t", "@view.gerbers:t", "@view:t"] {
            assert_eq!(s["drafts"][k].as_str().unwrap_or(""), "", "{k} must not outlive the layout");
        }
        assert_eq!(s["info"]["view.kind:t"], "review");

        // D: Retry on a resolved historical tile whose key is already the one on show still sends the watch, forced
        let sid = layout::opaque_source_id(&format!("{root}/.git"), &root);
        let ver = |label: &str| layout::TileVersion { source_id: sid.clone(), id: "1111aaaa99998888777766665555444433332222".into(), label: label.into() };
        let hist = layout::Tile { version: Some(ver("1111aaaa")), ..tile("t1", layout::PageKind::Board, "unit.kicad_pcb", false) };
        let hl = layout::add(&layout::Layout::single(hist).unwrap(), Some("t1"), layout::Side::Right, tile("t2", layout::PageKind::Schematic, "unit.kicad_sch", false), false, None).unwrap();
        let mut sh = json!({"sel":"t","rid":1,"drafts":{"@view:t":"board","@view.layout:t":serde_json::to_string(&hl).unwrap()},"panel":false,"info":{}});
        let viewer::LayoutOut::Ok(vf) = viewer::layout_action(&st, "h", &sh, Some(1), "en", "layout-focus", r#"{"tile":"t1"}"#).unwrap() else { panic!() };
        crate::session::patch::merge(&mut sh, &vf.patch);
        let on_show = sh["drafts"]["@view.at"].as_str().unwrap().to_string();
        assert!(!on_show.is_empty(), "the historical tile is the one watched");
        let viewer::LayoutOut::Ok(vr) = viewer::view_action_for(&st, "h", &sh, Some(1), "en", "retry", 5000).unwrap() else { panic!() };
        let wp = watch(&vr).expect("Retry on a historical tile whose key is on show must still send its watch");
        assert_eq!((wp["force"].clone(), wp["kind"].clone()), (json!(true), json!("board")));
        assert!(wp["path"].as_str().unwrap().contains("1111aaaa"), "the historical source, not live artwork: {wp}");
        crate::session::patch::merge(&mut sh, &vr.patch);
        assert_eq!(sh["drafts"]["@view.at"].as_str().unwrap(), on_show);
        // (a second Retry inside the second is the spam guard's)
        let viewer::LayoutOut::Ok(vr2) = viewer::view_action_for(&st, "h", &sh, Some(1), "en", "retry", 5500).unwrap() else { panic!() };
        assert!(vr2.before.is_empty());

        // F: the same revision of another source, and the 3D page of a revision, are different identities
        let other = layout::TileVersion { source_id: layout::opaque_source_id("/elsewhere/.git", "/elsewhere"), ..ver("1111aaaa") };
        let mk = |v: layout::TileVersion, kind| layout::Tile { version: Some(v), ..tile("t1", kind, "unit.kicad_pcb", false) };
        let two = |a: layout::Tile| layout::add(&layout::Layout::single(a).unwrap(), Some("t1"), layout::Side::Right, tile("t2", layout::PageKind::Schematic, "unit.kicad_sch", false), false, None).unwrap();
        let pic = |l: &layout::Layout| { let mut s = sh.clone(); s["drafts"]["@view.layout:t"] = json!(serde_json::to_string(l).unwrap()); s["drafts"]["@view.at"] = json!(""); viewer::viewer_json(&st, &json!({}), &s).unwrap() };
        let a = pic(&two(mk(ver("1111aaaa"), layout::PageKind::Board)));
        let b = pic(&two(mk(other.clone(), layout::PageKind::Board)));
        assert_ne!(a["tiles"]["t1"]["key"], b["tiles"]["t1"]["key"], "the source id is part of a historical subscription identity");
        let three_d = pic(&two(mk(ver("1111aaaa"), layout::PageKind::Solid3d)));
        assert!(three_d["tiles"]["t1"]["layers"].as_str().unwrap().contains("1111aaaa99998888777766665555444433332222"), "a historical 3D layer key names its revision");
        assert!(!three_d["tiles"]["t1"]["layers"].as_str().unwrap().contains(&root));
    }
    /// An agent's show is applied only through its token (kind and full path unchanged, this thread), for every kind; the read-back settles exactly.
    #[test]
    fn rail_v_2a_show_tokens_apply_exactly_and_read_back_exactly() {
        let d = crate::plat::tmp::dir("rail-v-2a-show");
        let root = d.to_string_lossy().into_owned();
        for f in ["unit.kicad_pcb", "unit.kicad_sch"] { std::fs::write(d.join(f), "()").unwrap(); }
        std::fs::create_dir_all(d.join("alt")).unwrap(); std::fs::write(d.join("alt/alt.kicad_pcb"), "()").unwrap();
        std::fs::create_dir_all(d.join("mech")).unwrap(); std::fs::write(d.join("mech/case.step"), "ISO-10303-21;").unwrap();
        std::fs::create_dir_all(d.join("mfg/rev1")).unwrap(); std::fs::write(d.join("mfg/rev1/u-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        let idle = json!({"sel":"t","rid":1,"drafts":{},"panel":false,"info":{}});
        let shows = [("board", format!("{root}/unit.kicad_pcb")), ("pcb", format!("{root}/alt/alt.kicad_pcb")), ("schematic", format!("{root}/unit.kicad_sch")), ("3d", format!("{root}/unit.kicad_pcb")),
            ("gerbers", format!("{root}/mfg/rev1")), ("split", format!("{root}/unit.kicad_pcb")), ("mech", format!("{root}/mech/case.step")), ("3d", format!("{root}/mech/case.step")),
            ("files", String::new()), ("browser", String::new()), ("review", String::new())];
        for (k, p) in &shows {
            let tok = viewer::issue_show("t", k, p);
            assert!(!viewer::show_applied_by(&tok, 1) && viewer::show_refusal_of(&tok).is_none());
            let viewer::LayoutOut::Ok(vo) = viewer::view_show_token(&st, "h", &idle, Some(1), "en", k, p, &tok).unwrap() else { panic!("{k} refused") };
            assert!(vo.patch.as_object().is_some_and(|o| !o.is_empty()), "{k} {p}: the show applies");
            assert!(viewer::show_applied_by(&tok, 1), "{k}");
            // the same token with another kind, another path, another thread, or no token of the hub's: nothing applies
            let k2 = if *k == "board" { "schematic" } else { "board" };
            for (k2, p2, t2, why) in [(k2, p.as_str(), "t", "another kind"), (*k, "/elsewhere/x.kicad_pcb", "t", "another path"), (*k, p.as_str(), "u", "another thread")] {
                let issued = viewer::issue_show(t2, if why == "another kind" { *k } else { k2 }, if why == "another path" { p.as_str() } else { p.as_str() });
                let viewer::LayoutOut::Ok(none) = viewer::view_show_token(&st, "h", &idle, Some(1), "en", k2, p2, &issued).unwrap() else { panic!() };
                assert!(none.patch.as_object().unwrap().is_empty() && none.before.is_empty(), "{k}: {why}: a token for something else applies nothing");
            }
            let viewer::LayoutOut::Ok(none) = viewer::view_show_token(&st, "h", &idle, Some(1), "en", k, p, "show-nonsense").unwrap() else { panic!() };
            assert!(none.patch.as_object().unwrap().is_empty(), "an unknown token applies nothing");
            let viewer::LayoutOut::Ok(none) = viewer::view_show_token(&st, "h", &idle, Some(1), "en", k, p, "").unwrap() else { panic!() };
            assert!(none.patch.as_object().unwrap().is_empty(), "an empty token applies nothing");
        }
        // another thread's token never applies here
        let other = viewer::issue_show("u", "board", &format!("{root}/unit.kicad_pcb"));
        let viewer::LayoutOut::Ok(none) = viewer::view_show_token(&st, "h", &idle, Some(1), "en", "board", &format!("{root}/unit.kicad_pcb"), &other).unwrap() else { panic!() };
        assert!(none.patch.as_object().unwrap().is_empty());

        // the read-back: exact page and source, the project's own file resolved to its name first
        let dflt = viewer::ShowDefaults { root: root.clone(), pcb: format!("{root}/unit.kicad_pcb"), sch: format!("{root}/unit.kicad_sch"), set: format!("{root}/mfg/rev1") };
        let want = |k: &str, p: &str| viewer::show_want(k, p, &root, &dflt);
        let own = want("board", &format!("{root}/unit.kicad_pcb"));
        assert!(viewer::show_settled(&own, &json!({"open":"board","key":format!("0|{root}|")}), &dflt), "a key with no path is the project's own board");
        assert!(viewer::show_settled(&own, &json!({"open":"board","key":format!("0|{root}|{root}/unit.kicad_pcb")}), &dflt));
        assert!(viewer::show_settled(&want("board", ""), &json!({"open":"board","key":format!("0|{root}|")}), &dflt), "a show with no path is the project's own board");
        assert!(!viewer::show_settled(&own, &json!({"open":"board","key":format!("0|{root}|{root}/alt/alt.kicad_pcb")}), &dflt), "another board");
        assert!(!viewer::show_settled(&own, &json!({"open":"board","key":format!("0|{root}|x{root}/unit.kicad_pcb")}), &dflt), "no suffix matching");
        let alt = want("pcb", &format!("{root}/alt/alt.kicad_pcb"));
        assert!(!viewer::show_settled(&alt, &json!({"open":"board","key":format!("0|{root}|")}), &dflt), "the own board is not the alt board");
        for bad in [json!({}), json!({"open":"board"}), json!({"open":"board","key":""}), json!({"key":format!("0|{root}|")}), json!({"open":"schematic","key":format!("0|{root}|")})] {
            assert!(!viewer::show_settled(&own, &bad, &dflt), "{bad}");
        }
        assert!(!viewer::show_settled(&own, &json!({"open":"board","gerbers":true,"key":format!("0|{root}|")}), &dflt), "Gerbers are not the board");
        let set = want("gerbers", "");
        assert!(viewer::show_settled(&set, &json!({"open":"board","gerbers":true,"key":format!("0|{root}|")}), &dflt));
        assert!(!viewer::show_settled(&set, &json!({"open":"board","key":format!("0|{root}|")}), &dflt));
        let mech = want("mech", &format!("{root}/mech/case.step"));
        assert!(viewer::show_settled(&mech, &json!({"open":"mech","part":"mech/case.step"}), &dflt));
        for bad in [json!({"open":"mech","part":""}), json!({"open":"mech"}), json!({"open":"mech","part":"mech/other.step"}), json!({"open":"mech","part":"x/mech/case.step"})] { assert!(!viewer::show_settled(&mech, &bad, &dflt), "{bad}"); }
        assert!(viewer::show_settled(&want("browser", ""), &json!({"open":"browser"}), &dflt));
        // Split settles on the board its panes draw, not on the Split's own key
        let split = want("split", &format!("{root}/alt/alt.kicad_pcb"));
        let panes = |board: &str| json!({"open":"split","key":format!("split|{root}|"),"panes":[format!("1|{root}|"), format!("0|{root}|{board}"), format!("2|{root}|{board}")]});
        assert!(viewer::show_settled(&split, &panes(&format!("{root}/alt/alt.kicad_pcb")), &dflt));
        assert!(!viewer::show_settled(&split, &panes(""), &dflt), "the own board is not the alt board");
        assert!(!viewer::show_settled(&split, &json!({"open":"split","key":format!("split|{root}|")}), &dflt), "the Split's own key says nothing of its board");
        assert!(viewer::show_settled(&want("split", ""), &panes(""), &dflt));
        // a named Files folder or file settles only on that folder or that file's preview; no folder named is the page alone
        assert!(viewer::show_settled(&want("files", ""), &json!({"open":"files"}), &dflt));
        assert!(viewer::show_settled(&want("files", &format!("{root}/mech")), &json!({"open":"files","dir":"mech"}), &dflt));
        assert!(!viewer::show_settled(&want("files", &format!("{root}/mech")), &json!({"open":"files","dir":""}), &dflt));
        let txt = want("files", &format!("{root}/mech/notes.txt"));
        assert_eq!(txt.kind, "file");
        assert!(viewer::show_settled(&txt, &json!({"open":"text","file":format!("/img?path={}", root.replace('/', "%2F") + "%2Fmech%2Fnotes.txt")}), &dflt));
        assert!(!viewer::show_settled(&txt, &json!({"open":"text","file":format!("/img?path={}", root.replace('/', "%2F") + "%2Fmech%2Fother.txt")}), &dflt));
        assert!(!viewer::show_settled(&txt, &json!({"open":"files","dir":"mech"}), &dflt), "the folder is not the file's preview");
        // (a repeated identical show: the picture already shows it, so it settles with no newer report)

        // under a KiCad holder's dialog a show may only re-show the locked tile with the source it names
        let board = layout::derive_layout(&layout::DeriveInput { root: &root, open: "board", picked_board: "alt/alt.kicad_pcb", ..Default::default() });
        let lid = board.tiles()[0].id.clone();
        assert!(viewer::show_preserves_locked(&board, &lid, Some(layout::PageKind::Board), "alt/alt.kicad_pcb"));
        assert!(!viewer::show_preserves_locked(&board, &lid, Some(layout::PageKind::Board), "unit.kicad_pcb"), "another board is refused");
        assert!(!viewer::show_preserves_locked(&board, &lid, Some(layout::PageKind::Board), ""), "an empty source is never the same");
        assert!(!viewer::show_preserves_locked(&board, &lid, Some(layout::PageKind::Schematic), "alt/alt.kicad_pcb"));
        assert!(!viewer::show_preserves_locked(&board, &lid, None, "alt/alt.kicad_pcb"), "Split is not the locked page");
    }
    /// An agent's raw push, through the token handler a desktop answers with, to the page and source the hub then draws: every kind, the
    /// normalized kinds ("" with a board path, "3d" with a model) and another board or sheet included.
    #[test]
    fn rail_show_raw_push_to_token_handler_to_the_selected_page_and_source() {
        let d = crate::plat::tmp::dir("rail-show-push");
        let root = d.to_string_lossy().into_owned();
        for f in ["unit.kicad_pcb", "unit.kicad_sch", "power.kicad_sch"] { std::fs::write(d.join(f), "()").unwrap(); }
        std::fs::create_dir_all(d.join("alt")).unwrap(); std::fs::write(d.join("alt/alt.kicad_pcb"), "()").unwrap();
        std::fs::create_dir_all(d.join("mech")).unwrap(); std::fs::write(d.join("mech/case.step"), "ISO-10303-21;").unwrap(); std::fs::write(d.join("mech/notes.txt"), "hi").unwrap();
        std::fs::create_dir_all(d.join("mfg/rev1")).unwrap(); std::fs::write(d.join("mfg/rev1/u-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();
        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"tp","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        let idle = json!({"sel":"tp","rid":1,"drafts":{},"panel":false,"info":{"fab.board:tp":"unit.kicad_pcb","fab.boards:tp":"unit.kicad_pcb\nalt/alt.kicad_pcb"}});
        // the project's own files as the hub finds them (the agent's read-back resolves a key with no path the same way)
        let own = crate::files::kicad::project::read(&root);
        let dflt = viewer::ShowDefaults { root: root.clone(), pcb: own.pcb, sch: own.sch, set: crate::files::kicad::gerber::default_set(std::path::Path::new(&root)).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default() };
        // what the desktop reports of a picture (gpui `plotview::state_json`)
        let report = |v: &Value| {
            let open = v["open"].as_str().unwrap_or("");
            let mut r = json!({"open": open, "key": v["key"], "part": if open == "mech" { v["mech"]["rel"].clone() } else { Value::Null }});
            if v["gerbers"] == true { r["gerbers"] = json!(true) }
            if open == "split" { r["panes"] = json!(v["panes"].as_array().unwrap().iter().filter_map(|p| p["key"].as_str()).collect::<Vec<_>>()) }
            if open == "files" { r["dir"] = json!(v["files"]["trail"].as_array().and_then(|t| t.last()).and_then(|c| c["value"].as_str()).unwrap_or("")) }
            if matches!(open, "image" | "pdf" | "text") { r["file"] = v["file"]["url"].clone() }
            r
        };
        let shows = [("board", String::new()), ("board", format!("{root}/unit.kicad_pcb")), ("pcb", format!("{root}/alt/alt.kicad_pcb")), ("", format!("{root}/alt/alt.kicad_pcb")),
            ("schematic", String::new()), ("schematic", format!("{root}/unit.kicad_sch")), ("schematic", format!("{root}/power.kicad_sch")), ("", format!("{root}/power.kicad_sch")),
            ("3d", format!("{root}/unit.kicad_pcb")), ("3d", format!("{root}/mech/case.step")), ("", format!("{root}/mech/case.step")), ("mech", format!("{root}/mech/case.step")),
            ("gerbers", String::new()), ("gerbers", format!("{root}/mfg/rev1")), ("split", format!("{root}/unit.kicad_pcb")), ("split", format!("{root}/alt/alt.kicad_pcb")),
            ("files", String::new()), ("files", format!("{root}/mech")), ("files", format!("{root}/mech/notes.txt")), ("browser", String::new())];
        for (k, p) in &shows {
            // the push the hub sends, and what a desktop answers with from it
            let push = viewer::show_push("tp", k, p);
            let value = format!("{}\t{}\t{}", push["kind"].as_str().unwrap(), push["path"].as_str().unwrap(), push["token"].as_str().unwrap());
            let mut parts = value.splitn(3, '\t'); let (pk, pp, tok) = (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap());
            let out = viewer::view_show_token(&st, "h", &idle, Some(1), "en", pk, pp, tok).unwrap_or_else(|| panic!("{k} {p}: unanswered"));
            let viewer::LayoutOut::Ok(vo) = out else { panic!("{k} {p}: refused") };
            assert!(viewer::show_applied_by(tok, 1), "{k} {p}");
            let mut s = idle.clone(); crate::session::patch::merge(&mut s, &vo.patch);
            let pic = viewer::viewer_json(&st, &json!({}), &s).unwrap_or(Value::Null);
            let want = viewer::show_want(k, p, &root, &dflt);
            assert!(viewer::show_settled(&want, &report(&pic), &dflt), "{k} {p}: the hub draws {} (key {}), not {want:?}", pic["open"], pic["key"]);
            // a close is never what a show does
            assert!(pic["open"].as_str().is_some_and(|o| !o.is_empty()), "{k} {p}: a show never closes");
        }
        // Review is the page of the pages writer (it is no viewer picture): the show opens it
        let push = viewer::show_push("tp", "review", "");
        let viewer::LayoutOut::Ok(vo) = viewer::view_show_token(&st, "h", &idle, Some(2), "en", "review", "", push["token"].as_str().unwrap()).unwrap() else { panic!() };
        assert_eq!(vo.patch["info"]["view.kind:tp"], "review");
        // the normalized kinds say what they are: a model under "3d" is Mech, a board under "" is that board (and the other board is the picked one)
        let alt = format!("{root}/alt/alt.kicad_pcb");
        let push = viewer::show_push("tp", "", &alt);
        let viewer::LayoutOut::Ok(vo) = viewer::view_show_token(&st, "h", &idle, Some(3), "en", "", &alt, push["token"].as_str().unwrap()).unwrap() else { panic!() };
        assert_eq!((vo.patch["drafts"]["@view:tp"].as_str(), vo.patch["drafts"]["@view.board:tp"].as_str()), (Some("board"), Some("alt/alt.kicad_pcb")));
    }

    /// A token is applied at most once per session, only while it is the thread's newest show, and only for its lifetime; several sessions each
    /// apply one broadcast once; the table stays bounded.
    #[test]
    fn rail_show_token_lifetime_once_per_session_superseded_expiring_and_bounded() {
        let d = crate::plat::tmp::dir("rail-show-life");
        let root = d.to_string_lossy().into_owned();
        std::fs::write(d.join("unit.kicad_pcb"), "()").unwrap();
        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"tl","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        let idle = json!({"sel":"tl","rid":1,"drafts":{},"panel":false,"info":{}});
        let applies = |client: u64, k: &str, p: &str, tok: &str| -> bool {
            let viewer::LayoutOut::Ok(vo) = viewer::view_show_token(&st, "h", &idle, Some(client), "en", k, p, tok).unwrap() else { panic!() };
            vo.patch.as_object().is_some_and(|o| !o.is_empty())
        };
        // once per session; another session applies the same broadcast once too
        let tok = viewer::show_push("tl", "files", "")["token"].as_str().unwrap().to_string();
        assert!(applies(1, "files", "", &tok));
        assert!(!applies(1, "files", "", &tok), "a session applies a token once");
        assert!(applies(2, "files", "", &tok), "another session on the thread applies the broadcast once");
        assert!(!applies(2, "files", "", &tok));
        // a newer show for the thread supersedes it
        let old = viewer::show_push("tl", "browser", "")["token"].as_str().unwrap().to_string();
        let newer = viewer::show_push("tl", "files", "")["token"].as_str().unwrap().to_string();
        assert!(!applies(3, "browser", "", &old), "a superseded token applies nothing");
        assert!(applies(3, "files", "", &newer));
        // another thread's show does not supersede this one
        let mine = viewer::show_push("tl", "files", "")["token"].as_str().unwrap().to_string();
        let _other = viewer::show_push("tother", "files", "");
        assert!(applies(4, "files", "", &mine));
        // it expires: a token issued longer ago than its life applies nothing
        let past = std::time::Instant::now().checked_sub(viewer::TOKEN_LIFE + std::time::Duration::from_secs(1)).unwrap();
        let stale = viewer::issue_show_at_for_test("tl", "files", "", past);
        assert!(!applies(5, "files", "", &stale), "an expired token applies nothing");
        // bounded
        for i in 0..400 { viewer::show_push(&format!("tb{i}"), "files", ""); }
        assert!(viewer::shows_len_for_test() <= 128, "the token table is bounded: {}", viewer::shows_len_for_test());
    }
    /// Review round on the agent show: a prior board pick, a linked thread, a stored layout's named board and Split, an already-open preview.
    #[test]
    fn rail_show_applies_the_named_source_over_a_pick_a_stored_layout_and_a_preview() {
        let d = crate::plat::tmp::dir("rail-show-src");
        let root = d.to_string_lossy().into_owned();
        for f in ["unit.kicad_pcb", "unit.kicad_sch"] { std::fs::write(d.join(f), "()").unwrap(); }
        std::fs::create_dir_all(d.join("alt")).unwrap(); for f in ["alt/alt.kicad_pcb", "alt/alt.kicad_sch"] { std::fs::write(d.join(f), "()").unwrap(); }
        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"ts","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        st.apply_line(&json!({"$":"ProjectCreated","id":"peer~p","title":"Remote","root":"/remote/proj","at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"peer~ts","project":"peer~p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        let show = |session: &Value, thread: &str, kind: &str, path: &str, client: u64| -> viewer::LayoutOut {
            let push = viewer::show_push(thread, kind, path);
            viewer::view_show_token(&st, "h", session, Some(client), "en", kind, path, push["token"].as_str().unwrap()).unwrap()
        };
        let ok = |o: viewer::LayoutOut| match o { viewer::LayoutOut::Ok(v) => v, viewer::LayoutOut::Refused { text, .. } => panic!("refused: {text}") };
        let watches = |vo: &viewer::ViewOut| vo.before.iter().filter_map(|c| serde_json::from_str::<Value>(c["text"].as_str()?).ok()).filter(|m| m["m"] == "kicad.watch").map(|m| m["p"].clone()).collect::<Vec<_>>();

        // 1. an alternate board is picked; the project's own schematic is shown by its full path: the pick follows the schematic
        let picked = json!({"sel":"ts","rid":1,"drafts":{"@view:ts":"board","@view.board:ts":"alt/alt.kicad_pcb","@view.file:ts":format!("{root}/alt/alt.kicad_pcb"),"@view.at":format!("0|{root}|{root}/alt/alt.kicad_pcb")},"panel":false,
            "info":{"fab.board:ts":"unit.kicad_pcb","fab.boards:ts":"unit.kicad_pcb\nalt/alt.kicad_pcb"}});
        let own_sch = format!("{root}/unit.kicad_sch");
        let vo = ok(show(&picked, "ts", "schematic", &own_sch, 1));
        let mut s = picked.clone(); crate::session::patch::merge(&mut s, &vo.patch);
        assert_eq!(s["drafts"]["@view.board:ts"], "", "the own board is no pick");
        let pic = viewer::viewer_json(&st, &json!({}), &s).unwrap();
        let own_files = crate::files::kicad::project::read(&root);
        let dflt = viewer::ShowDefaults { root: root.clone(), pcb: own_files.pcb, sch: own_files.sch, set: String::new() };
        let report = json!({"open": pic["open"], "key": pic["key"]});
        assert!(viewer::show_settled(&viewer::show_want("schematic", &own_sch, &root, &dflt), &report, &dflt), "the own schematic is on show, not the alternate board's: {}", pic["key"]);
        // ... and the alternate board's own schematic shows with its board picked
        let alt_sch = format!("{root}/alt/alt.kicad_sch");
        let vo = ok(show(&json!({"sel":"ts","rid":1,"drafts":{},"panel":false,"info":{"fab.board:ts":"unit.kicad_pcb"}}), "ts", "schematic", &alt_sch, 1));
        assert_eq!(vo.patch["drafts"]["@view.board:ts"], "alt/alt.kicad_pcb");
        // on a linked thread the same rule, to the machine that has the files
        let far = json!({"sel":"peer~ts","rid":1,"drafts":{"@view:peer~ts":"board","@view.board:peer~ts":"alt/alt.kicad_pcb"},"panel":false,"info":{"fab.board:peer~ts":"unit.kicad_pcb","fab.boards:peer~ts":"unit.kicad_pcb\nalt/alt.kicad_pcb"}});
        let vo = ok(show(&far, "peer~ts", "schematic", "/remote/proj/unit.kicad_sch", 2));
        assert_eq!(vo.patch["drafts"]["@view.board:peer~ts"], "", "a linked show of the own board's schematic clears the alternate pick");
        let vo = ok(show(&far, "peer~ts", "schematic", "/remote/proj/alt/alt.kicad_sch", 2));
        assert_eq!(vo.patch["drafts"]["@view.board:peer~ts"], "alt/alt.kicad_pcb", "a linked show picks the board that owns the schematic");
        let vo = ok(show(&json!({"sel":"peer~ts","rid":1,"drafts":{},"panel":false,"info":{}}), "peer~ts", "schematic", "/remote/proj/sub/child.kicad_sch", 3));
        assert!(watches(&vo).iter().all(|w| w["on"] == "peer"), "every watch of a linked show goes to its machine: {:?}", watches(&vo));
        assert_eq!(vo.patch["drafts"]["@view.sheet:peer~ts"], "/remote/proj/sub/child.kicad_sch");

        // 2. a stored layout, a named drawing: the tile is explicit and the watch carries the named path
        let tile = |id: &str, kind, src: &str, explicit| layout::Tile { id: id.into(), kind, source: src.into(), explicit, version: None, unresolved_version: false, overlay: None };
        let base = layout::add(&layout::Layout::single(tile("t1", layout::PageKind::Board, "unit.kicad_pcb", false)).unwrap(), Some("t1"), layout::Side::Right, tile("t2", layout::PageKind::Schematic, "unit.kicad_sch", false), false, None).unwrap();
        let stored = json!({"sel":"ts","rid":1,"drafts":{"@view:ts":"board","@view.layout:ts":serde_json::to_string(&base).unwrap(),"@view.at":format!("0|{root}|")},"panel":false,"info":{"fab.board:ts":"unit.kicad_pcb"}});
        let alt = format!("{root}/alt/alt.kicad_pcb");
        let vo = ok(show(&stored, "ts", "pcb", &alt, 4));
        let w = watches(&vo); assert_eq!(w.len(), 1, "{w:?}"); assert_eq!(w[0]["path"], alt, "the watch carries the named board, not the default");
        let l: layout::Layout = serde_json::from_str(vo.patch["drafts"]["@view.layout:ts"].as_str().unwrap()).unwrap();
        assert!(l.tile("t1").unwrap().explicit && l.tile("t1").unwrap().source == "alt/alt.kicad_pcb");

        // 3. a stored layout, Split: never discarded; satisfied by the Board and Schematic it holds, else the missing one is added, else refused
        let vo = ok(show(&stored, "ts", "split", &format!("{root}/unit.kicad_pcb"), 5));
        let l: layout::Layout = serde_json::from_str(vo.patch["drafts"]["@view.layout:ts"].as_str().unwrap()).expect("the layout is kept");
        assert_eq!(l.tiles().len(), 2, "a layout holding Board and Schematic is only focused");
        let files_only = layout::add(&layout::Layout::single(tile("t1", layout::PageKind::Board, "unit.kicad_pcb", false)).unwrap(), Some("t1"), layout::Side::Right, tile("t2", layout::PageKind::Files, "", false), false, None).unwrap();
        let s2 = json!({"sel":"ts","rid":1,"drafts":{"@view:ts":"board","@view.layout:ts":serde_json::to_string(&files_only).unwrap()},"panel":false,"info":{}});
        let vo = ok(show(&s2, "ts", "split", &format!("{root}/unit.kicad_pcb"), 6));
        let l: layout::Layout = serde_json::from_str(vo.patch["drafts"]["@view.layout:ts"].as_str().unwrap()).unwrap();
        assert!(l.tiles().iter().any(|t| t.kind == layout::PageKind::Schematic) && l.tiles().iter().any(|t| t.kind == layout::PageKind::Files), "the Schematic is added beside, the person's Files stays");
        assert!(layout::show_split(&files_only, "", "", false, Some("t1")).is_ok_and(|l| l.tile("t1").is_some()), "adding beside the locked tile leaves it as it is");
        assert_eq!(layout::show_split(&files_only, "/abs/x.kicad_pcb", "", false, None), Err(layout::Refusal::InvalidSource), "a Split that cannot be shown is refused with a reason, never by clearing the layout");

        // 4. an unnamed Files show over an already-open preview settles on the preview
        let dflt = viewer::ShowDefaults { root: root.clone(), ..Default::default() };
        let want = viewer::show_want("files", "", &root, &dflt);
        for open in ["files", "image", "pdf", "text"] { assert!(viewer::show_settled(&want, &json!({"open": open}), &dflt), "{open}"); }
        assert!(!viewer::show_settled(&viewer::show_want("files", &format!("{root}/alt"), &root, &dflt), &json!({"open": "image"}), &dflt), "a named folder is not satisfied by a preview");
        // a stored layout reports its tiles; Split is read off them
        let sp = viewer::show_want("split", &format!("{root}/unit.kicad_pcb"), &root, &viewer::ShowDefaults { root: root.clone(), pcb: format!("{root}/unit.kicad_pcb"), ..Default::default() });
        assert!(viewer::show_settled(&sp, &json!({"open":"board","tiles":["board\tunit.kicad_pcb","schematic\tunit.kicad_sch"]}), &dflt));
        assert!(!viewer::show_settled(&sp, &json!({"open":"board","tiles":["board\talt/alt.kicad_pcb","schematic\tunit.kicad_sch"]}), &dflt));
    }
    /// Review round 3: a child sheet keeps the person's pick; a stored Split restores a maximized pair and is read back on the visible, sibling pair.
    #[test]
    fn rail_show_child_sheet_keeps_the_pick_and_stored_split_reads_back_the_visible_sibling_pair() {
        let d = crate::plat::tmp::dir("rail-show-r3");
        let root = d.to_string_lossy().into_owned();
        for f in ["unit.kicad_pcb", "unit.kicad_sch", "power.kicad_sch"] { std::fs::write(d.join(f), "()").unwrap(); }
        std::fs::create_dir_all(d.join("alt")).unwrap(); for f in ["alt/alt.kicad_pcb", "alt/alt.kicad_sch"] { std::fs::write(d.join(f), "()").unwrap(); }
        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"tr","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());
        let show = |session: &Value, kind: &str, path: &str, client: u64| -> (viewer::LayoutOut, String) {
            let push = viewer::show_push("tr", kind, path); let tok = push["token"].as_str().unwrap().to_string();
            (viewer::view_show_token(&st, "h", session, Some(client), "en", kind, path, &tok).unwrap(), tok)
        };
        // 1. Board B is picked; a child sheet (no board owns it) is shown; the next Board press still shows B
        let picked = json!({"sel":"tr","rid":1,"drafts":{"@view:tr":"board","@view.board:tr":"alt/alt.kicad_pcb","@view.file:tr":format!("{root}/alt/alt.kicad_pcb"),"@view.at":format!("0|{root}|{root}/alt/alt.kicad_pcb")},"panel":false,
            "info":{"fab.board:tr":"unit.kicad_pcb","fab.boards:tr":"unit.kicad_pcb\nalt/alt.kicad_pcb"}});
        let (viewer::LayoutOut::Ok(vo), _) = show(&picked, "schematic", &format!("{root}/power.kicad_sch"), 1) else { panic!() };
        let mut s = picked.clone(); crate::session::patch::merge(&mut s, &vo.patch);
        assert_eq!(s["drafts"]["@view.board:tr"], "alt/alt.kicad_pcb", "a child sheet leaves the pick");
        assert_eq!(s["drafts"]["@view.sheet:tr"], format!("{root}/power.kicad_sch"));
        let b = viewer::view_action(&st, "h", &s, "board").unwrap();
        let mut after = s.clone(); crate::session::patch::merge(&mut after, &b.patch);
        assert_eq!(after["drafts"]["@view.file:tr"], format!("{root}/alt/alt.kicad_pcb"), "the next Board press shows B");

        // 2. a stored layout with the matching Board maximized: the show restores the pair; the read-back is of the visible, sibling pair
        let tile = |id: &str, kind, src: &str| layout::Tile { id: id.into(), kind, source: src.into(), explicit: true, version: None, unresolved_version: false, overlay: None };
        let mut l = layout::add(&layout::Layout::single(tile("t1", layout::PageKind::Board, "unit.kicad_pcb")).unwrap(), Some("t1"), layout::Side::Right, tile("t2", layout::PageKind::Schematic, "unit.kicad_sch"), false, None).unwrap();
        l.maximized = Some("t1".into());
        let stored = json!({"sel":"tr","rid":1,"drafts":{"@view:tr":"board","@view.layout:tr":serde_json::to_string(&l).unwrap()},"panel":false,"info":{}});
        let (viewer::LayoutOut::Ok(vo), tok) = show(&stored, "split", &format!("{root}/unit.kicad_pcb"), 5) else { panic!() };
        let nl: layout::Layout = serde_json::from_str(vo.patch["drafts"]["@view.layout:tr"].as_str().unwrap()).unwrap();
        assert!(nl.maximized.is_none(), "the pair is shown: a maximized tile is restored");
        assert_eq!(viewer::show_resolved(&tok, 5), Some(("unit.kicad_pcb".to_string(), "unit.kicad_sch".to_string())));
        let dflt = viewer::ShowDefaults { root: root.clone(), pcb: format!("{root}/unit.kicad_pcb"), sch: format!("{root}/unit.kicad_sch"), set: String::new() };
        let want = viewer::show_want("split", &format!("{root}/unit.kicad_pcb"), &root, &dflt);
        let res = viewer::show_resolved(&tok, 5);
        let settled = |tiles: Value| viewer::show_settled_with(&want, &json!({"open":"board","tiles":tiles}), &dflt, res.as_ref());
        assert!(!settled(json!(["board\tunit.kicad_pcb"])), "a Board alone on screen is not the Split");
        assert!(settled(json!(["board\tunit.kicad_pcb", "schematic\tunit.kicad_sch"])));
        assert!(!settled(json!(["board\tunit.kicad_pcb", "schematic\talt/alt.kicad_sch"])), "the Schematic must be the board's sibling, not any schematic");
        // an unnamed Split means the session's current pick (else the project's own): the mutation and the read-back share it
        assert_eq!(viewer::split_sources(&picked, &root, false, ""), ("alt/alt.kicad_pcb".to_string(), "alt/alt.kicad_sch".to_string()));
        let own = crate::files::kicad::project::read(&root);
        assert_eq!(viewer::split_sources(&json!({"sel":"tr","drafts":{}}), &root, false, ""), (layout::sanitize_rel(&root, &own.pcb), layout::sanitize_rel(&root, &own.sch)), "the project's own pair, as the hub finds it");
    }
    struct Mine; impl Screen for Mine {
        fn name(&self) -> &'static str { "mine" }
        fn view(&self, p: &Value) -> Option<Value> { if p["target"]["kind"] == "thread" { Some(json!({"shown":{"by":"rust"}})) } else { None } }
    }
    fn helper() -> Arc<dyn Presenter> { Arc::new(Canned { reply: json!({"shown":{"by":"helper"}}) }) }
    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::uncreated(&format!("present-{n}")) }

    #[tokio::test] async fn a_screen_answers_what_it_has_and_the_helper_the_rest() {
        let r = Routed { helper: helper(), screens: vec![Box::new(Mine)], dump: None };
        assert_eq!(r.view(&json!({"target":{"kind":"thread","id":"t"}})).await.unwrap()["shown"]["by"], "rust");
        assert_eq!(r.view(&json!({"target":{"kind":"recent"}})).await.unwrap()["shown"]["by"], "helper");
    }
    #[tokio::test] async fn without_a_helper_an_unported_view_says_so_and_the_log_is_not_fed() {
        let r = Routed { helper: Arc::new(NotPorted), screens: vec![Box::new(Mine)], dump: None };
        assert_eq!(r.view(&json!({"target":{"kind":"thread","id":"t"}})).await.unwrap()["shown"]["by"], "rust", "a ported screen still answers");
        let e = r.view(&json!({"target":{"kind":"settings"},"action":"x"})).await.unwrap_err().to_string();
        assert!(e.contains("not ported") && e.contains("settings"), "{e}");
        assert!(!r.wants_log() && r.ping().await.is_ok() && !r.is_down());
    }
    #[tokio::test] async fn no_screens_is_the_helper_unchanged() {
        let r = Routed { helper: helper(), screens: vec![], dump: None };
        assert_eq!(r.view(&json!({})).await.unwrap(), json!({"shown":{"by":"helper"}}));
    }
    // the hub's explanation of a dead helper must pass through the router (the supervisor prints it before each restart)
    #[tokio::test] async fn why_down_is_forwarded_to_the_helper() {
        struct H; impl Presenter for H {
            fn name(&self) -> &'static str { "h" }
            fn fold<'a>(&'a self, _f: &'a Value) -> Fut<'a, ()> { Box::pin(async { Ok(()) }) }
            fn view<'a>(&'a self, _p: &'a Value) -> Fut<'a, Value> { Box::pin(async { Ok(json!({})) }) }
            fn ping(&self) -> Fut<'_, ()> { Box::pin(async { Ok(()) }) }
            fn why_down(&self) -> Option<String> { Some("the policy helper died: killed by SIGKILL".into()) }
        }
        let r = Routed { helper: Arc::new(H), screens: vec![], dump: None };
        assert_eq!(r.why_down().as_deref(), Some("the policy helper died: killed by SIGKILL"));
        assert!(Routed { helper: helper(), screens: vec![], dump: None }.why_down().is_none(), "a helper with nothing to say says nothing");
    }
    #[tokio::test] async fn the_dump_records_the_helper_pairs_and_only_on_a_copy_run() {
        let d = tmp("on");
        let r = Routed { helper: helper(), screens: vec![Box::new(Mine)], dump: Routed::open_dump(Some(d.to_path_buf()), true, true, 1000) };
        r.view(&json!({"target":{"kind":"recent"}})).await.unwrap();   // helper answered: recorded
        r.view(&json!({"target":{"kind":"thread","id":"t"}})).await.unwrap(); // rust answered: not recorded (nothing to compare with)
        let text = std::fs::read_to_string(d.join("views.jsonl")).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        let l: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(l["params"]["target"]["kind"], "recent"); assert_eq!(l["out"]["shown"]["by"], "helper");
        let d2 = tmp("off");
        assert!(Routed::open_dump(Some(d2.to_path_buf()), false, true, 1000).is_none() && !d2.exists(), "a non-copy run writes no dump");
        assert!(Routed::open_dump(Some(d2.to_path_buf()), true, false, 1000).is_none() && !d2.exists(), "no helper, nothing to record");
        assert!(Routed::open_dump(Some(d2.to_path_buf()), true, true, crate::copy::BIG_LOG + 1).is_none() && !d2.exists(), "a log of real size is never dumped");
        use crate::plat::fs::PermissionsExt;
        assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(d.join("views.jsonl")).unwrap().permissions().mode() & 0o777, 0o600), 0o600, "the dump is private");
    }
    #[tokio::test] async fn a_fab_look_answer_does_not_send_the_view_to_the_helper_but_another_read_does() {
        let r = Routed { helper: helper(), screens: vec![Box::new(Mine)], dump: None };
        let p = |k: &str| json!({"target":{"kind":"thread","id":"t"},"reads":[{"kind":k,"id":1,"ok":true,"result":{}}]});
        assert_eq!(r.view(&p("fab.look")).await.unwrap()["shown"]["by"], "rust");
        assert_eq!(r.view(&p("git.diff")).await.unwrap()["shown"]["by"], "helper");
    }
}

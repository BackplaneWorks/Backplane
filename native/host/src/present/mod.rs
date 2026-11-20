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
        assert_eq!(l.tiles().len(), 2, "must not invent a sibling schematic on linked thread");
        assert_eq!(l.tile("pcb").unwrap().source, "sub/main.kicad_pcb");
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
    fn rail_v_2a_layout_actions_tiles_rail_present_phone_press_and_holder_lock() {
        let d = crate::plat::tmp::dir("rail-v-2a");
        let root = d.to_string_lossy().into_owned();
        std::fs::write(d.join("unit.kicad_pcb"), "(kicad_pcb)").unwrap();
        std::fs::write(d.join("unit.kicad_sch"), "(kicad_sch)").unwrap();
        std::fs::create_dir_all(d.join("mfg/rev1")).unwrap();
        std::fs::write(d.join("mfg/rev1/unit-F_Cu.gbr"), "%FSLAX46Y46*%M02*").unwrap();

        let mut st = crate::state::State::new();
        st.apply_line(&json!({"$":"ProjectCreated","id":"p","title":"Board","root":root,"at":1}).to_string());
        st.apply_line(&json!({"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}).to_string());

        let mut sess = json!({"sel":"t","rid":1,"drafts":{"@view:t":"board"},"panel":false,"info":{}});

        // 1. Add a Gerbers tile beside the Board tile (Scorecard Task 4)
        let out = viewer::layout_action(&st, "h", &sess, Some(10), "layout-add", r#"{"side":"right","kind":"gerbers"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo) = out else { panic!("expected Ok") };
        crate::session::patch::merge(&mut sess, &vo.patch);
        let v = viewer::viewer_json(&st, &json!({}), &sess).unwrap();
        let l: layout::Layout = serde_json::from_value(v["layout"].clone()).unwrap();
        assert_eq!(l.tiles().len(), 2);
        assert_eq!(l.focus.as_deref(), Some("t2"));
        // viewer.tiles has both t1 (Board) and t2 (Gerbers) with plot keys, layers, identity
        assert_eq!(v["tiles"]["t1"]["kind"], "board");
        assert_eq!(v["tiles"]["t1"]["key"], format!("0|{root}|"));
        assert_eq!(v["tiles"]["t1"]["identity"]["page"], "board");
        assert_eq!(v["tiles"]["t1"]["identity"]["source"]["rel"], "unit.kicad_pcb");
        assert_eq!(v["tiles"]["t2"]["kind"], "gerbers");
        assert_eq!(v["tiles"]["t2"]["gerbers"], true);
        assert_eq!(v["tiles"]["t2"]["key"], format!("0|{root}|{root}/mfg/rev1"));
        assert_eq!(v["tiles"]["t2"]["identity"]["page"], "gerbers");
        assert_eq!(v["tiles"]["t2"]["identity"]["set"]["rel"], "mfg/rev1");

        // 2. viewer.rail[].present is true for both board & gerbers, while state=="on" is ONLY gerbers (focused)
        let rail_row = |v: &Value, id: &str| v["rail"].as_array().unwrap().iter().find(|r| r["id"] == id).unwrap().clone();
        assert_eq!(rail_row(&v, "board")["present"], true);
        assert_eq!(rail_row(&v, "board")["state"], "off");
        assert_eq!(rail_row(&v, "gerbers")["present"], true);
        assert_eq!(rail_row(&v, "gerbers")["state"], "on");
        assert_eq!(rail_row(&v, "schematic")["present"], false);

        // 3. Phone (or any client) pressing "board" while desktop shows Board + Gerbers focuses Board and preserves Gerbers!
        let phone_press = viewer::view_action(&st, "h", &sess, "board").unwrap();
        crate::session::patch::merge(&mut sess, &phone_press.patch);
        let v_after_phone = viewer::viewer_json(&st, &json!({}), &sess).unwrap();
        let l_after_phone: layout::Layout = serde_json::from_value(v_after_phone["layout"].clone()).unwrap();
        assert_eq!(l_after_phone.tiles().len(), 2, "desktop's second tile must survive phone tab press of present kind");
        assert_eq!(l_after_phone.focus.as_deref(), Some("t1"));
        assert_eq!(rail_row(&v_after_phone, "board")["state"], "on");
        assert_eq!(rail_row(&v_after_phone, "gerbers")["present"], true);
        assert_eq!(rail_row(&v_after_phone, "gerbers")["state"], "off");

        // 4. layout-resize never emits watches; tooSmall returns catalog refusal code + key + text
        let res_ok = viewer::layout_action(&st, "h", &sess, Some(10), "layout-resize", r#"{"split":"s1","ratio":0.6,"bounds":[1000,800],"kicadTile":"t1"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_r) = res_ok else { panic!("expected resize Ok") };
        assert!(vo_r.before.is_empty(), "layout-resize must never change watches");
        let res_small = viewer::layout_action(&st, "h", &sess, Some(10), "layout-resize", r#"{"split":"s1","ratio":0.5,"bounds":[500,800],"kicadTile":"t1"}"#).unwrap();
        assert!(matches!(res_small, viewer::LayoutOut::Refused { code: "tooSmall", key: "viewer.layout.refusal.tooSmall", text: "Too small for KiCad" }));

        // 5. Ruling 1: `locked` honoured ONLY for holder; non-holder's `locked` ignored; hub open dialog locks even without client `locked`
        sess["info"]["editor.kicad.abc"] = json!({"holder": 10, "dialogs": [], "runtime": {"holder": 10}});
        // Holder (10) sends locked="t1" -> close("t1") refused with catalog text
        let r_holder = viewer::layout_action(&st, "h", &sess, Some(10), "layout-close", r#"{"tile":"t1","locked":"t1"}"#).unwrap();
        assert!(matches!(r_holder, viewer::LayoutOut::Refused { code: "locked", key: "viewer.layout.refusal.locked", text: "KiCad has a dialog open" }));
        // Non-holder (99) sends locked="t1" -> ignored, close succeeds
        let r_other = viewer::layout_action(&st, "h", &sess, Some(99), "layout-close", r#"{"tile":"t1","locked":"t1"}"#).unwrap();
        assert!(matches!(r_other, viewer::LayoutOut::Ok(_)));
        // Hub session info shows open KiCad dialog -> hub refuses even when client (10) forgot `locked`!
        sess["info"]["editor.kicad.abc"]["dialogs"] = json!(["Save Changes"]);
        let r_dialog = viewer::layout_action(&st, "h", &sess, Some(10), "layout-close", r#"{"tile":"t1"}"#).unwrap();
        assert!(matches!(r_dialog, viewer::LayoutOut::Refused { code: "locked", text: "KiCad has a dialog open", .. }));
        sess["info"]["editor.kicad.abc"]["dialogs"] = json!([]);

        // 6. Overlay toggle: merge t2 (Gerbers) onto t1 (Board), then unsplit back to 2 tiles
        let ov_out = viewer::layout_action(&st, "h", &sess, Some(10), "layout-overlay", r#"{"base":"t1","overlay":"t2","mode":"diff"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_ov) = ov_out else { panic!("expected overlay Ok") };
        crate::session::patch::merge(&mut sess, &vo_ov.patch);
        let v_ov = viewer::viewer_json(&st, &json!({}), &sess).unwrap();
        assert_eq!(v_ov["tiles"]["t1"]["gerbersVsBoard"], true);
        let un_out = viewer::layout_action(&st, "h", &sess, Some(10), "layout-unsplit", r#"{"tile":"t1","side":"right"}"#).unwrap();
        let viewer::LayoutOut::Ok(vo_un) = un_out else { panic!("expected unsplit Ok") };
        crate::session::patch::merge(&mut sess, &vo_un.patch);
        let v_un = viewer::viewer_json(&st, &json!({}), &sess).unwrap();
        let l_un: layout::Layout = serde_json::from_value(v_un["layout"].clone()).unwrap();
        assert_eq!(l_un.tiles().len(), 2);

        // 7. viewer_show_action updates & focuses existing tile of kind, else replaces focused
        let vs = viewer::viewer_show_action(&st, "h", &sess, Some(10), "board", &format!("{root}/unit.kicad_pcb")).unwrap();
        let viewer::LayoutOut::Ok(vo_vs) = vs else { panic!("expected viewer_show Ok") };
        crate::session::patch::merge(&mut sess, &vo_vs.patch);
        let v_vs = viewer::viewer_json(&st, &json!({}), &sess).unwrap();
        assert_eq!(v_vs["layout"]["focus"], "t1");

        println!("RAIL_V_2A_TILES_EXAMPLE={}", json!({"layout": v["layout"], "tiles": v["tiles"]}));
        let _ = std::fs::remove_dir_all(&d);
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

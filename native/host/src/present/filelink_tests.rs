//! A file link opens inside Backplane: its project's thread, the tab that draws it, the file loaded; never the system's app for a design file.
use super::*;

fn st() -> State {
    let mut s = State::new(); s.lean = true;
    for e in [json!({"$":"ProjectCreated","id":"pa","title":"Amp","root":"/w/amp","at":1}),
              json!({"$":"ProjectCreated","id":"pk","title":"Kit","root":"/w/amp/kit","at":1}),
              json!({"$":"ProjectCreated","id":"pe","title":"Empty","root":"/w/empty","at":1}),
              json!({"$":"ThreadCreated","id":"ta","project":"pa","title":"A","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":5}),
              json!({"$":"ThreadCreated","id":"tb","project":"pa","title":"B","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":9}),
              json!({"$":"ThreadCreated","id":"tk","project":"pk","title":"K","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":3})] { assert!(s.apply(&e)); }
    s
}
fn files(p: &str) -> bool { matches!(p, "/w/amp/boards/main/main.kicad_pcb" | "/w/amp/boards/main/main.kicad_sch" | "/x/loose.kicad_pcb") }
fn plan_of(sel: &str, href: &str) -> Plan { plan(&st(), sel, href, "/home/me", &files) }
fn open(thread: &str, pid: &str, action: &'static str, value: &str) -> Plan { Plan::Open { thread: thread.into(), open: vec![(format!("project-closed:{pid}"), false)], action, value: value.into() } }

#[test] fn the_design_kinds_by_name() {
    for (n, k) in [("a.kicad_pcb", "board"), ("A.KICAD_SCH", "schematic"), ("x.kicad_pro", "project"), ("m.step", "model"), ("m.STP", "model"), ("m.glb", "model"),
                   ("b-F_Cu.gbr", "gerber"), ("b.gtl", "gerber"), ("b.drl", "gerber"), ("b.g2", "gerber"), ("notes.md", ""), ("x.rs", ""), ("g", "")] { assert_eq!(design_kind(n), k, "{n}"); }
}
#[test] fn a_path_is_made_absolute_from_the_link() {
    assert_eq!(absolute("file:///w/amp/a%20b.kicad_pcb", "", "").as_deref(), Some("/w/amp/a b.kicad_pcb"));
    assert_eq!(absolute("file://localhost/w/x.step", "", "").as_deref(), Some("/w/x.step"));
    assert_eq!(absolute("boards/main/main.kicad_sch:12", "/w/amp", "").as_deref(), Some("/w/amp/boards/main/main.kicad_sch"));
    assert_eq!(absolute("./a/../b.kicad_pcb#L3", "/w/amp", "").as_deref(), Some("/w/amp/b.kicad_pcb"));
    assert_eq!(absolute("~/x.kicad_pcb", "", "/home/me").as_deref(), Some("/home/me/x.kicad_pcb"));
    assert_eq!(absolute("rel.kicad_pcb", "", ""), None, "relative with no folder");
    assert_eq!(absolute("https://a.b/c.kicad_pcb", "/w", ""), None, "a web address is no file");
}
#[test] fn a_file_opens_in_its_projects_thread_on_the_tab_that_draws_it() {
    // the thread on show is the project's: it stays
    assert_eq!(plan_of("ta", "boards/main/main.kicad_pcb"), open("ta", "pa", "fm-open", "boards/main/main.kicad_pcb"));
    assert_eq!(plan_of("ta", "/w/amp/boards/main/main.kicad_sch"), open("ta", "pa", "fm-open", "boards/main/main.kicad_sch"));
    // another project's thread on show: the file's project, at its newest thread
    assert_eq!(plan_of("tk", "file:///w/amp/boards/main/main.kicad_pcb"), open("tb", "pa", "fm-open", "boards/main/main.kicad_pcb"));
    // the deepest project folder holds it
    assert_eq!(plan_of("ta", "/w/amp/kit/k.kicad_sch"), open("tk", "pk", "fm-open", "k.kicad_sch"));
    // a model on Mech (by its path), a KiCad project on its board, Gerbers on the project's Board, a note on Files
    assert_eq!(plan_of("ta", "/w/amp/mech/case.step"), open("ta", "pa", "view", "mech\t/w/amp/mech/case.step"));
    assert_eq!(plan_of("tk", "/w/amp/boards/main/main.kicad_pro"), open("tb", "pa", "fm-open", "boards/main/main.kicad_pcb"));
    assert_eq!(plan_of("ta", "/w/amp/mfg/gerbers/main-F_Cu.gbr"), open("ta", "pa", "view", "board"));
    assert_eq!(plan_of("ta", "/w/amp/README.md"), open("ta", "pa", "fm-open", "README.md"));
    // a file the viewer does not draw, in a project: the system's, as before
    assert_eq!(plan_of("ta", "/w/amp/src/main.rs"), Plan::Os("/w/amp/src/main.rs".into()));
}
#[test] fn a_design_file_in_no_project_is_drawn_in_the_thread_on_show_and_never_handed_to_the_system() {
    assert_eq!(plan_of("ta", "/x/loose.kicad_pcb"), Plan::Scoped { thread: "ta".into(), kind: "board", path: "/x/loose.kicad_pcb".into() });
    assert_eq!(plan_of("ta", "~/m.glb"), Plan::Scoped { thread: "ta".into(), kind: "mech", path: "/home/me/m.glb".into() });
    // a project with no thread yet: the same, scoped to the file
    assert_eq!(plan_of("ta", "/w/empty/e.kicad_sch"), Plan::Scoped { thread: "ta".into(), kind: "schematic", path: "/w/empty/e.kicad_sch".into() });
    // no thread on show: said, not opened elsewhere
    assert!(matches!(plan_of("", "/x/loose.kicad_pcb"), Plan::Note(_)));
    assert!(matches!(plan_of("ta", "/x/b.gbr"), Plan::Note(_)));
    // anything else in no project: the system's
    assert_eq!(plan_of("ta", "/x/notes.txt"), Plan::Os("/x/notes.txt".into()));
}
#[test] fn a_file_ref_is_from_its_project_or_thread() {
    let s = st();
    assert_eq!(file_ref(&s, &["pa".into(), "boards".into(), "main.kicad_pcb".into()]).as_deref(), Some("/w/amp/boards/main.kicad_pcb"));
    assert_eq!(file_ref(&s, &["tk".into(), "k.kicad_sch".into()]).as_deref(), Some("/w/amp/kit/k.kicad_sch"));
    assert_eq!(file_ref(&s, &["nope".into(), "x".into()]), None);
}
#[test] fn the_watch_takes_a_file_opened_by_a_link() {
    assert!(!allowed("/x/never.kicad_pcb")); allow("/x/once.kicad_pcb"); assert!(allowed("/x/once.kicad_pcb"));
    let session = json!({"sel": "ta", "rid": 3, "drafts": {}, "open": {}});
    let o = scoped_view(&st(), "h", &session, "board", "/x/loose.kicad_pcb").unwrap();
    assert!(allowed("/x/loose.kicad_pcb"));
    assert_eq!((o.patch["drafts"]["@view:ta"].as_str(), o.patch["drafts"]["@view.file:ta"].as_str(), o.patch["drafts"]["@view.at"].as_str()), (Some("board"), Some("/x/loose.kicad_pcb"), Some("0|/w/amp|/x/loose.kicad_pcb")));
    let w: Value = serde_json::from_str(o.before[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!((w["m"].as_str(), w["p"]["root"].as_str(), w["p"]["path"].as_str()), (Some("kicad.watch"), Some("/w/amp"), Some("/x/loose.kicad_pcb")));
    assert!(crate::files::kicad::allowed(&["/w/amp".to_string()], "/w/amp", "") && !crate::files::kicad::allowed(&["/w/amp".to_string()], "/w/amp", "/x/loose.kicad_pcb"), "only the link's allow lets it in");
}
#[test] fn a_design_file_named_alone_is_found_below_the_root() {
    let d = std::env::temp_dir().join(format!("bp-findnamed-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("boards/main")).unwrap(); std::fs::create_dir_all(d.join(".backplane/worktrees/w/boards/main")).unwrap();
    std::fs::write(d.join("boards/main/main.kicad_sch"), "x").unwrap(); std::fs::write(d.join(".backplane/worktrees/w/boards/main/other.kicad_sch"), "x").unwrap();
    let root = d.to_string_lossy().to_string();
    assert_eq!(find_named(&root, "main.kicad_sch"), Some(format!("{root}/boards/main/main.kicad_sch")));
    assert_eq!(find_named(&root, "other.kicad_sch"), None, "a hidden folder (a worktree's copy) is not looked in");
    assert_eq!(find_named(&root, "x.rs"), None); assert_eq!(find_named(&root, "boards/main/main.kicad_sch"), None);
    let _ = std::fs::remove_dir_all(&d);
}

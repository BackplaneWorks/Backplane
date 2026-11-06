//! The Rust plot reader against child, chunk for chunk: `FILE.plot` and `FILE.infos` beside each fixture are what
//! `backplane --plot KIND FILE INFOS` wrote (test/files/plot_corpus.py and plot_modes.py do the same over every KiCad demo).
use files_harness::kicad::{plot, plotgen, plotread};
use std::path::PathBuf;
fn dir() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/plot") }
#[test] fn kicad_plot_same_as_the_child() {
    let mut bad = vec![];
    for (kind, name) in [(0u32, "ecc83-pp.kicad_pcb"), (0, "microwave.kicad_pcb"), (1, "ecc83-pp.kicad_sch"), (1, "complex_hierarchy.kicad_sch"), (1, "ampli_ht.kicad_sch"), (1, "rectifier.kicad_sch")] {
        let f = dir().join(name);
        let want = plot::parse_child(&std::fs::read_to_string(dir().join(format!("{name}.plot"))).unwrap());
        let infos = std::fs::read_to_string(dir().join(format!("{name}.infos"))).unwrap();
        let plotgen::Read::Plot(got, ti) = plotread::read(kind, f.to_str().unwrap()) else { panic!("{name} not read") };
        let differ = want.cs.iter().zip(&got.cs).filter(|(a, b)| a != b).count() + want.cs.len().abs_diff(got.cs.len());
        assert!(!want.cs.is_empty());
        if differ != 0 || want.b != got.b || want.edge != got.edge || want.thick != got.thick || infos != ti { bad.push(format!("{name}: {differ} of {} chunks differ, box {:?}/{:?}, edge {:?}/{:?}, thick {}/{}, infos {}", want.cs.len(), want.b, got.b, want.edge, got.edge, want.thick, got.thick, infos == ti)) }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
#[test] fn kicad_unfinished_and_missing_files() {
    let t = std::env::temp_dir().join(format!("bp-plot-{}", std::process::id())); std::fs::create_dir_all(&t).unwrap();
    let cut = t.join("cut.kicad_pcb"); std::fs::write(&cut, b"(kicad_pcb (a \"b").unwrap();
    assert!(matches!(plotread::read(0, cut.to_str().unwrap()), plotgen::Read::Half));
    let e = t.join("empty.kicad_sch"); std::fs::write(&e, b"").unwrap();
    assert!(matches!(plotread::read(1, e.to_str().unwrap()), plotgen::Read::Half));
    match plotread::read(0, t.join("gone.kicad_pcb").to_str().unwrap()) { plotgen::Read::Fail(m) => assert_eq!(m, "@bp.file-missing:gone.kicad_pcb"), _ => panic!() }
    let _ = std::fs::remove_dir_all(t);
}

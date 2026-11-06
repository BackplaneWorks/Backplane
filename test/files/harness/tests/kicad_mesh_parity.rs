//! Oracle goldens: what child printed for `--plot 2` and the PNGs `backplane --render` wrote (build/gpui/backplane-serve, test/files/fixtures/mesh), held against the Rust reader and renderer byte for byte.
use files_harness::kicad::{glb, mesh, plotgen, png, render, solidread};
const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/mesh");
#[test] fn kicad_solid_line_same_as_the_child() {
    for m in ["nodes", "glass", "thin", "wrap"] {
        let p = solidread::plot_of(&std::fs::read(format!("{DIR}/{m}.glb")).unwrap(), false).unwrap();
        assert!(p.solid.is_some(), "{m}");
        let mut got = plotgen::lines(&p); got.push(b'\n');
        assert!(got == std::fs::read(format!("{DIR}/{m}.line")).unwrap(), "{m}");
    }
}
#[test] fn kicad_render_same_as_the_renderer() {
    for m in ["nodes", "glass", "thin", "wrap"] {
        let ts = glb::tris(&std::fs::read(format!("{DIR}/{m}.glb")).unwrap()).unwrap();
        let (lo, hi) = mesh::bounds(&ts); let cam = mesh::fit(lo, hi);
        for (name, yaw, pitch) in render::VIEWS {
            let got = png::rgb(&mesh::render(render::W, render::H, &ts, &cam.face(yaw, pitch), mesh::GROUND), render::W, render::H);
            assert!(got == std::fs::read(format!("{DIR}/{m}.{name}.png")).unwrap(), "{m} {name}");
        }
    }
}

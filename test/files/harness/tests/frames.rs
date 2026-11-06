//! The first plot frame the hub sends after `kicad.watch` (kind board / schematic) on the ecc83 demo, byte for byte: the child's plot lines
//! (fixtures/plot/*.plot, what the reader prints) through the hub's own parse, delta against nothing held, and frame (kicad/plot.rs, as
//! watch.rs's send_to does for a fresh view). `FRAME_OUT=dir cargo test --test frames` writes them; without it the committed files are compared.
use files_harness::kicad::plot::{delta, frame, parse_child};
use std::path::PathBuf;
fn root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fixtures/plot") }
// the key of a view of the project's own file ("<kind>|<root>|"), and its family (Hist.key.family: the same for a live file)
fn first(kind: u32, plot_file: &str) -> Vec<u8> {
    let text = String::from_utf8(std::fs::read(root().join(plot_file)).unwrap()).unwrap();
    let p = parse_child(&text);
    let key = format!("{kind}|/fixture/ecc83|");
    frame(&key, &key, "", &p, &delta(&p.cs, &[]))
}
#[test] fn the_first_frames_of_the_ecc83_demo() {
    let out = [("ecc83-board.frame.cbor", first(0, "ecc83-pp.kicad_pcb.plot")), ("ecc83-sch.frame.cbor", first(1, "ecc83-pp.kicad_sch.plot"))];
    for (name, bytes) in &out {
        assert!(bytes.len() < 100_000 && bytes.len() > 1000, "{name}: {}", bytes.len());
        match std::env::var("FRAME_OUT") {
            Ok(dir) => { std::fs::create_dir_all(&dir).unwrap(); std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap(); println!("{name} {} bytes", bytes.len()) }
            Err(_) => { let committed = std::fs::read(root().join(name)).unwrap_or_else(|_| panic!("{name} is not committed")); assert_eq!(&committed, bytes, "{name}") }
        }
    }
}

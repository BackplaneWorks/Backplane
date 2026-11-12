//! Smart Turn on WAVs (16 kHz mono PCM16), with timings: `cargo run --example turn_probe -- <runtime dir> <model> <wav>...` prints `p features_ms model_ms path`.
use backplane_voice::turn::{features, Turn};
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ort = std::path::Path::new(&a[0]).join(backplane_voice::local::onnx_library());
    let t = Turn::load(&ort, std::path::Path::new(&a[1])).unwrap();
    for w in &a[2..] {
        let b = std::fs::read(w).unwrap(); let d = b.windows(4).position(|x| x == b"data").unwrap() + 8;
        let s: Vec<f32> = b[d..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect();
        let t0 = Instant::now(); let f = features(&s); if let Ok(d) = std::env::var("DUMP") { std::fs::write(&d, f.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap(); } let t1 = Instant::now(); let p = t.run(&f).unwrap(); let t2 = Instant::now();
        println!("{p:.4} {:.1} {:.1} {w}", (t1 - t0).as_secs_f64() * 1e3, (t2 - t1).as_secs_f64() * 1e3);
    }
}

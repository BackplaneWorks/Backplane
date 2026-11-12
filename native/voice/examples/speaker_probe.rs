//! Speaker embeddings' cosine between WAVs (16 kHz mono PCM16): `cargo run --example speaker_probe -- <local dir> <model> <wav>...`; the first is
//! the enrolment, compared with each of the rest, with timings.
use backplane_voice::sherpa::{Api, Speaker};
use std::time::Instant;
fn pcm(p: &str) -> Vec<f32> { let b = std::fs::read(p).unwrap(); let d = b.windows(4).position(|x| x == b"data").unwrap() + 8; b[d..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect() }
fn cos(a: &[f32], b: &[f32]) -> f32 { let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum(); d / (a.iter().map(|x| x * x).sum::<f32>().sqrt() * b.iter().map(|x| x * x).sum::<f32>().sqrt()) }
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let api = Api::load(std::path::Path::new(&a[0])).unwrap(); let t = Instant::now();
    let s = Speaker::create(&api, std::path::Path::new(&a[1])).unwrap(); eprintln!("load {} ms, dim {}", t.elapsed().as_millis(), s.dim);
    let embs: Vec<(String, Vec<f32>, u128)> = a[2..].iter().map(|w| { let t = Instant::now(); let e = s.embed(&pcm(w)).unwrap(); (w.clone(), e, t.elapsed().as_millis()) }).collect();
    for (w, e, ms) in &embs[1..] { println!("{:.3} {ms} ms {w}", cos(&embs[0].1, e)) }
}

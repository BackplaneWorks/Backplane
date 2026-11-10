//! Times the local engine's start, piece by piece, and its memory: `cargo run --example load_probe -- <local-dir> <pcm s16le 16k> [threads]`.
use backplane_voice::sherpa::{Api, Recognizer, Vad};
use std::time::Instant;
fn rss() -> String { std::fs::read_to_string("/proc/self/status").unwrap_or_default().lines().filter(|l| l.starts_with("VmRSS") || l.starts_with("RssFile") || l.starts_with("RssAnon")).map(|l| l.split_whitespace().take(2).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(", ") }
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&a[0]);
    let pcm = std::fs::read(&a[1]).unwrap();
    let threads: i32 = a.get(2).map_or(4, |t| t.parse().unwrap());
    let f: Vec<f32> = pcm.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect();
    let t = Instant::now();
    let api = Api::load(&dir).unwrap();
    println!("runtime dlopen       {:5} ms  {}", t.elapsed().as_millis(), rss());
    let t = Instant::now();
    let rec = Recognizer::create(&api, &dir, "parakeet-v2", threads).unwrap();
    println!("recognizer create    {:5} ms  {}", t.elapsed().as_millis(), rss());
    #[cfg(target_os = "linux")] { extern "C" { fn malloc_trim(pad: usize) -> i32; } let t = Instant::now(); unsafe { malloc_trim(0) }; println!("malloc_trim          {:5} ms  {}", t.elapsed().as_millis(), rss()); }
    let t = Instant::now();
    let _vad = Vad::create(&api, &dir).unwrap();
    println!("vad create           {:5} ms  {}", t.elapsed().as_millis(), rss());
    for (n, sec) in [("first decode 1 s", 1.0), ("decode 1 s", 1.0), ("decode 3 s", 3.0), ("decode 10 s", 10.0), ("decode 1 s", 1.0)] {
        let k = ((sec * 16000.0) as usize).min(f.len());
        let t = Instant::now();
        let j = rec.decode_json(&f[..k]).unwrap();
        let txt: String = j.split("\"text\":").nth(1).unwrap_or("").chars().take(40).collect();
        #[cfg(target_os = "linux")] { extern "C" { fn malloc_trim(pad: usize) -> i32; } unsafe { malloc_trim(0) }; }
        println!("{n:20} {:5} ms  {}  {txt}", t.elapsed().as_millis(), rss());
    }
}

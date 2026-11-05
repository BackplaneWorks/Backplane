//! state-probe NOW BATCH|none CHUNK...: fold the logs with the Rust State and print {tick, meta, quiet}: Auto.changes at NOW and the
//! commit-path events for the BATCH file (the Rust side of native/bend/autosettle_probe.bend; synthetic logs only).
#[path = "../state/mod.rs"]
mod state;
#[path = "../shelf.rs"]
#[allow(dead_code)]
mod shelf;
#[path = "../signal.rs"]
#[allow(dead_code)]
mod signal;
use serde_json::Value;
use std::io::{BufRead, BufReader};
fn lines(path: &str) -> Vec<String> {
    let f = std::fs::File::open(path).unwrap_or_else(|e| { eprintln!("state-probe: {path}: {e}"); std::process::exit(2) });
    BufReader::new(f).lines().map_while(Result::ok).filter(|l| !l.is_empty()).collect()
}
fn fold(paths: &[String]) -> state::State { let mut st = state::State::new(); for p in paths { for l in lines(p) { st.apply_line(&l); } } st }
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let now: u64 = args[0].parse().unwrap_or(0);
    let batch: Vec<Value> = if args[1] == "none" { vec![] } else { lines(&args[1]).iter().filter_map(|l| serde_json::from_str(l).ok()).collect() };
    let st = fold(&args[2..]);
    // metadata sees the state before each change, advanced by the earlier ones and their own metadata (Auto.messages.metadata)
    let mut meta = vec![]; let mut st2 = fold(&args[2..]);
    for c in &batch { let one = state::fam::autosettle::metadata(&st2, c); meta.extend(one.iter().cloned()); st2.apply(c); for e in &one { st2.apply(e); } }
    let out = serde_json::json!({"tick": state::fam::autosettle::changes(&st, now), "meta": meta, "quiet": state::fam::autosettle::quiet(&st, &batch)});
    println!("{}", serde_json::to_string(&out).unwrap());
}

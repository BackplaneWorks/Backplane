//! rust-digest CHUNK1 [CHUNK2 ...]: fold events.jsonl files with the Rust State fold and print the canonical digest JSON
//! (the Rust side of the differential oracle, test/gpui/parity/oracle/DIGEST.md).
#[path = "../state.rs"]
mod state;
use std::io::{BufRead, BufReader};
fn main() {
    let mut st = state::State::new();
    for path in std::env::args().skip(1) {
        let f = std::fs::File::open(&path).unwrap_or_else(|e| { eprintln!("rust-digest: {path}: {e}"); std::process::exit(2) });
        for line in BufReader::new(f).lines().map_while(Result::ok) { if !line.is_empty() { st.apply_line(&line); } }
    }
    if !st.unmodelled.is_empty() { let mut u: Vec<_> = st.unmodelled.iter().collect(); u.sort(); eprintln!("rust-digest: events counted but not modelled: {u:?}"); }
    println!("{}", serde_json::to_string(&st.digest_json()).unwrap());
}

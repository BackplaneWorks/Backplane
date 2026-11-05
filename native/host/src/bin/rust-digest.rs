//! rust-digest CHUNK1 [CHUNK2 ...]: fold events.jsonl files with the Rust State fold and print the canonical digest JSON
//! (the Rust side of the differential oracle, test/gpui/parity/oracle/DIGEST.md).
#[path = "../state/mod.rs"]
mod state;
#[path = "../shelf.rs"]
#[allow(dead_code)]
mod shelf;
#[path = "../signal.rs"]
#[allow(dead_code)]
mod signal;
use std::io::{BufRead, BufReader};
fn main() {
    let mut st = state::State::new();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // the digest holds titles, paths and setting values: it is printed only when asked for (the oracle does, and never shows it); by default only counts
    let json = args.first().map_or(false, |a| a == "--json");
    if json { args.remove(0); }
    for path in args {
        let f = std::fs::File::open(&path).unwrap_or_else(|e| { eprintln!("rust-digest: {path}: {e}"); std::process::exit(2) });
        for line in BufReader::new(f).lines().map_while(Result::ok) { if !line.is_empty() { st.apply_line(&line); } }
    }
    if !st.unmodelled.is_empty() { let mut u: Vec<_> = st.unmodelled.iter().collect(); u.sort(); eprintln!("rust-digest: events counted but not modelled: {u:?}"); }
    if json { println!("{}", serde_json::to_string(&st.digest_json()).unwrap()); }
    else { println!("{{\"seq\":{},\"projects\":{},\"threads\":{},\"settings\":{},\"entries\":{}}}", st.seq, st.projects.len(), st.threads.len(), st.settings.len(), st.entries.values().map(|v| v.len()).sum::<usize>()); }
}

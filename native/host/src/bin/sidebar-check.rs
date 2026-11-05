//! sidebar-check [--statuses|--shelves] LOG...: fold the logs with the Rust State and print what the sidebar computes from it.
//! Default: counts per status word (safe for the store copy). --statuses: {thread id: status word} (hub-generated logs only).
#[path = "../state.rs"]
mod state;
#[path = "../shelf.rs"]
#[allow(dead_code)]
mod shelf;
#[path = "../signal.rs"]
#[allow(dead_code)]
mod signal;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mode = if args.first().map_or(false, |a| a.starts_with("--")) { args.remove(0) } else { String::new() };
    let mut st = state::State::new();
    for path in args {
        let f = std::fs::File::open(&path).unwrap_or_else(|e| { eprintln!("sidebar-check: {path}: {e}"); std::process::exit(2) });
        for line in BufReader::new(f).lines().map_while(Result::ok) { if !line.is_empty() { st.apply_line(&line); } }
    }
    let ix = signal::index(&st);
    let mut words: BTreeMap<String, &'static str> = BTreeMap::new();
    for (id, _t) in &st.threads { words.insert(id.clone(), signal::display_name(&st, ix[id], id)); }
    if mode == "--statuses" { println!("{}", serde_json::to_string(&words).unwrap()); return }
    let mut hist: BTreeMap<&str, usize> = BTreeMap::new();
    for w in words.values() { *hist.entry(w).or_default() += 1; }
    println!("{}", serde_json::to_string(&serde_json::json!({"threads": st.threads.len(), "visible": st.threads.values().filter(|t| !(t.archived || t.deleted)).count(), "statuses": hist})).unwrap());
}

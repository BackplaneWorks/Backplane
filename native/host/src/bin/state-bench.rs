//! state-bench LOG: fold a log (synthetic only) into the policy core's lean state and report fold time, RSS added, and the tick's duration.
#[path = "../state/mod.rs"]
mod state;
#[path = "../shelf.rs"]
#[allow(dead_code)]
mod shelf;
#[path = "../signal.rs"]
#[allow(dead_code)]
mod signal;
use std::io::{BufRead, BufReader};
use std::time::Instant;
fn rss_kib() -> u64 { std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok())).unwrap_or(0) }
fn main() {
    let path = std::env::args().nth(1).expect("log");
    let lean = std::env::args().nth(2).map_or(true, |a| a != "full");
    let (r0, t0) = (rss_kib(), Instant::now());
    let mut core = state::fam::policy::Core::new();
    core.st.lean = lean;
    let mut n = 0u64;
    for l in BufReader::new(std::fs::File::open(&path).unwrap()).lines().map_while(Result::ok) { core.fold_line(&l); n += 1; }
    let fold = t0.elapsed();
    let r1 = rss_kib();
    let mut ticks = vec![];
    let mut changes = 0;
    for _ in 0..7 { let t = Instant::now(); changes = core.tick(2_000_000_000).len(); ticks.push(t.elapsed().as_secs_f64() * 1000.0); }
    ticks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("lines {n} lean {lean} threads {} settings {} fold {:.2} s ({:.0} k lines/s) rss_before {} KiB rss_after {} KiB added {} KiB tick median {:.2} ms max {:.2} ms (changes {changes})",
        core.st.threads.len(), core.st.settings.len(), fold.as_secs_f64(), n as f64 / fold.as_secs_f64() / 1000.0, r0, r1, r1.saturating_sub(r0), ticks[3], ticks[6]);
}

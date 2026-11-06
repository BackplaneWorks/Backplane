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
    // derive per batch: the writer-path cost of Transform::before_commit. A mix of agents' output on random threads and on task children
    // (so the ancestor scans run): assistant messages, tool acts, user messages, request opens and closes; 40,000 single-event batches.
    let mut ds = vec![];
    let mut x: u64 = 88172645463325252;
    let kids: Vec<String> = core.st.tasks.values().map(|t| t.1.clone()).collect();
    for i in 0..40_000u64 {
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        let th = if i % 2 == 0 && !kids.is_empty() { kids[(x as usize) % kids.len()].clone() } else { format!("t{}", x % 6666) };
        let at = 1_900_000_000u64 + i / 4;
        let ev = match i % 5 {
            0 => serde_json::json!({"$": "MessagePosted", "thread": th, "msg": format!("mb{i}"), "role": "user", "text": "please do the thing", "at": at}),
            1 | 2 => serde_json::json!({"$": "MessagePosted", "thread": th, "msg": format!("ab{i}"), "role": "assistant", "text": "working on it", "at": at}),
            3 => serde_json::json!({"$": "ActivityLogged", "thread": th, "act": format!("gb{i}"), "tone": "tool", "kind": "Bash", "summary": "ls -la", "at": at}),
            _ => serde_json::json!({"$": "AskClosed", "id": format!("q{}", x % 400), "answer": "y", "at": at}),
        };
        let t = Instant::now(); let d = core.derive(&[ev]); ds.push((t.elapsed().as_secs_f64() * 1e6, d.end.len() + d.after.len())); // the events the policies add; core and a failed end's moved marks are the caller's own
    }
    let mut us: Vec<f64> = ds.iter().map(|x| x.0).collect(); us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("derive per batch (40000 single-event batches, {} tasks, {} open asks): mean {:.1} us median {:.1} us p99 {:.1} us p99.9 {:.1} us max {:.1} us, {:.1} extra events/batch",
        core.st.tasks.len(), core.st.asks.values().filter(|a| a.1).count(), us.iter().sum::<f64>() / us.len() as f64, us[us.len() / 2], us[us.len() * 99 / 100], us[us.len() * 999 / 1000], us[us.len() - 1], ds.iter().map(|x| x.1).sum::<usize>() as f64 / ds.len() as f64);
    println!("lines {n} lean {lean} threads {} settings {} fold {:.2} s ({:.0} k lines/s) rss_before {} KiB rss_after {} KiB added {} KiB tick median {:.2} ms max {:.2} ms (changes {changes})",
        core.st.threads.len(), core.st.settings.len(), fold.as_secs_f64(), n as f64 / fold.as_secs_f64() / 1000.0, r0, r1, r1.saturating_sub(r0), ticks[3], ticks[6]);
}

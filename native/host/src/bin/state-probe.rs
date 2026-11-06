//! state-probe NOW BATCH|none CHUNK...: fold the logs with the Rust State and print {tick, meta, quiet}: Auto.changes at NOW and the
//! commit-path events (LEAN=1: with the lean state the policy host runs on) for the BATCH file (the Rust side of native/bend/autosettle_probe.bend; synthetic logs only).
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
fn fold(paths: &[String]) -> state::State { let mut st = state::State::new(); st.lean = std::env::var("LEAN").map_or(false, |v| v == "1"); for p in paths { for l in lines(p) { st.apply_line(&l); } } st }
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let now: u64 = args[0].parse().unwrap_or(0);
    let batch: Vec<Value> = if args[1] == "none" { vec![] } else { lines(&args[1]).iter().filter_map(|l| serde_json::from_str(l).ok()).collect() };
    let st = fold(&args[2..]);
    // metadata sees the state before each change, advanced by the earlier ones and their own metadata (Auto.messages.metadata)
    let mut meta = vec![]; let mut st2 = fold(&args[2..]);
    for c in &batch { let one = state::fam::autosettle::metadata(&st2, c); meta.extend(one.iter().cloned()); st2.apply(c); for e in &one { st2.apply(e); } }
    // a turn's end (Ended.out, then Alerts.changes' told marks over the batch and what the end added). The agents' batch has a failed end's mark
    // inside it; Bend writes that mark after the alert, stamped with the step's now.
    let end = {
        let pre = fold(&args[2..]);
        let (core, mut marks) = state::fam::turnend::split_fail_marks(&batch);
        for m in &mut marks { m["at"] = serde_json::json!(now); }
        let mut post = fold(&args[2..]);
        for c in &core { post.apply(c); }
        let seq = pre.seq + core.len() as u64;
        let ended = state::fam::turnend::ended_events(&pre, &post, seq, now, &core);
        let ths: Vec<String> = state::fam::turnend::ended_threads(&core).into_iter().map(String::from).collect();
        let ctx = state::fam::turnend::told_ctx(&pre, &core, &ths);
        let mut list = core.clone(); list.extend(ended.iter().cloned()); list.extend(marks.iter().cloned());
        // the asks the step opens (Alerts.asks over the state after the changes so far), then the told marks over everything
        let mut post2 = fold(&args[2..]);
        for c in &list { post2.apply(c); }
        let asks = state::fam::turnend::alert_asks(&post2, pre.seq + list.len() as u64, &list);
        let mut all = list.clone(); all.extend(asks.iter().cloned());
        let mut v = ended; v.extend(marks); v.extend(asks); v.extend(state::fam::turnend::told_marks(&ctx, &all)); v
    };
    let out = serde_json::json!({"tick": state::fam::autosettle::changes(&st, now), "meta": meta, "quiet": state::fam::autosettle::quiet(&st, &batch), "live": state::fam::watchdog::after(&st, now, &batch), "end": end, "wakes": state::fam::autosettle::tick_post(&st, now), "upgrade": state::fam::autosettle::upgrade(&st, now), "reopen": state::fam::autosettle::reopen(&st, &batch)});
    println!("{}", serde_json::to_string(&out).unwrap());
}

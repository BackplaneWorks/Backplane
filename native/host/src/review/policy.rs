//! When the design review runs. On demand (the tool `design_review`, the panel's Run): the scope asked. Before an order:
//! `order_review` takes the run kept for the design as it is, else runs scope "order" first. On save: the file checks (scope "save":
//! no kicad-cli, no agent), for a project shown in the last half hour, once the design's files have been quiet for DEBOUNCE_MS, one run
//! at a time, and not while memory is short. `next` is the pure rule; `start` the loop that applies it. Laws `review_policy_*`.
use crate::hub::Hub;
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock}, time::Duration};

pub const DEBOUNCE_MS: u64 = 1500;
pub const WATCH_MS: u64 = 30 * 60_000;
/// Memory a save run needs free (MB): it reads the board and the outputs, no more.
pub const NEED_MB: u64 = 400;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct St { pub stamp: String, pub changed_at: u64, pub ran: String, pub running: bool }
#[derive(Clone, Debug, PartialEq)]
pub enum Act { Run, Wait(u64), Nothing }
/// Review.policy.next: the design's stamp now (its files' sizes and times), the time and free memory (MB; 0 unknown) -> what to do.
pub fn next(now: u64, st: &mut St, stamp: &str, free_mb: u64) -> Act {
    if st.stamp != stamp { st.stamp = stamp.to_string(); st.changed_at = now; if st.ran.is_empty() && st.changed_at == now && !st.running { /* first look: no run for what was already there */ st.ran = stamp.to_string() } }
    if st.running || st.ran == st.stamp { return Act::Nothing }
    let quiet = now.saturating_sub(st.changed_at);
    if quiet < DEBOUNCE_MS { return Act::Wait(DEBOUNCE_MS - quiet) }
    if free_mb > 0 && free_mb < NEED_MB { return Act::Wait(5000) }
    Act::Run
}

/// The design's stamp: the sizes and times of its board, schematics and fab files (cheap: no file is read).
pub fn stamp(root: &Path) -> String {
    let f = crate::files::kicad::project::read(&root.to_string_lossy());
    let mut v: Vec<String> = vec![];
    let mut add = |p: &Path| if let Ok(m) = std::fs::metadata(p) { v.push(format!("{}:{}:{:?}", p.display(), m.len(), m.modified().ok())) };
    for p in [&f.pcb, &f.sch] { if !p.is_empty() { add(Path::new(p)) } }
    for p in crate::settings::fab::find_fab(root) { add(&root.join(p)) }
    v.join("|")
}

static WATCHED: OnceLock<Mutex<HashMap<PathBuf, (u64, St)>>> = OnceLock::new();
fn watched() -> &'static Mutex<HashMap<PathBuf, (u64, St)>> { WATCHED.get_or_init(Default::default) }
/// A client shows the project (the Fab panel, the checks): keep its review live for a while.
pub fn watch(root: &Path) { watched().lock().unwrap().entry(root.to_path_buf()).or_insert((0, St::default())).0 = crate::hub::now_ms() }

fn free_mb() -> u64 {
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|t| t.lines().find(|l| l.starts_with("MemAvailable:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())).map_or(0, |kb| kb / 1024)
}
pub fn start(hub: &Arc<Hub>) {
    if crate::copy::on() { return }
    let hub = hub.clone();
    tokio::spawn(async move {
        loop {
            let now = crate::hub::now_ms();
            let roots: Vec<PathBuf> = { let mut m = watched().lock().unwrap(); m.retain(|_, (at, st)| now.saturating_sub(*at) < WATCH_MS || st.running); m.keys().cloned().collect() };
            let mut sleep = 2000u64;
            for r in roots {
                let r2 = r.clone(); let s = tokio::task::spawn_blocking(move || (stamp(&r2), free_mb())).await.unwrap_or_default();
                let act = { let mut m = watched().lock().unwrap(); let Some((_, st)) = m.get_mut(&r) else { continue }; let a = next(now, st, &s.0, s.1); if a == Act::Run { st.running = true } a };
                match act {
                    Act::Run => { let h = hub.clone(); let stamp_then = s.0.clone(); tokio::spawn(async move {
                        let _ = super::run(&h, &r, "save").await;
                        if let Some((_, st)) = watched().lock().unwrap().get_mut(&r) { st.running = false; st.ran = stamp_then }
                    }); }
                    Act::Wait(ms) => sleep = sleep.min(ms.max(200)),
                    Act::Nothing => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(sleep)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn review_policy_a_save_runs_once_quiet_and_never_twice() {
        let mut st = St::default();
        assert_eq!(next(1000, &mut st, "a", 0), Act::Nothing, "what was there when first seen is no save");
        assert_eq!(next(2000, &mut st, "b", 0), Act::Wait(DEBOUNCE_MS), "a save: wait for the burst to end");
        assert_eq!(next(2500, &mut st, "c", 0), Act::Wait(DEBOUNCE_MS), "another save restarts the wait");
        assert_eq!(next(4000, &mut st, "c", 0), Act::Run);
        st.running = true; assert_eq!(next(4100, &mut st, "c", 0), Act::Nothing, "one run at a time");
        st.running = false; st.ran = "c".into(); assert_eq!(next(9000, &mut st, "c", 0), Act::Nothing, "nothing changed since");
        assert_eq!(next(9000, &mut st, "d", 0), Act::Wait(DEBOUNCE_MS));
        assert_eq!(next(11000, &mut st, "d", 100), Act::Wait(5000), "short of memory: later");
        assert_eq!(next(11000, &mut st, "d", 4000), Act::Run);
    }
}

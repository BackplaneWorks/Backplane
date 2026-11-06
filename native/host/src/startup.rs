//! Start-up timing: one stderr line per phase, `startup: +<ms since the process began> <what>`.
use std::sync::OnceLock;
use std::time::Instant;
static T0: OnceLock<Instant> = OnceLock::new();
pub fn begin() { let _ = T0.set(Instant::now()); }
pub fn mark(what: &str) { eprintln!("startup: +{}ms {what} (rss {} MB)", T0.get_or_init(Instant::now).elapsed().as_millis(), rss_mb()); }
fn rss_mb() -> u64 { std::fs::read_to_string("/proc/self/statm").ok().and_then(|s| s.split_whitespace().nth(1).and_then(|p| p.parse::<u64>().ok())).map_or(0, |p| p * 4096 / 1_048_576) }

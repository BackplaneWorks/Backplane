//! How far an import is, for everyone who shows it: the guard (`/healthz`), anyone asking `/hello`, the desktop (the info key `import.v014`) and the phones
//! (data/1 `migrating`). The import moves the numbers as it works; a ticker in `mod.rs` publishes them every second, so an update comes even while one step is
//! long. The estimate is measured, never guessed: the rate of the step now running over its last ten seconds, and none until a second of it has passed.
use serde_json::{json, Value};
use std::{collections::VecDeque, sync::Mutex};

/// The steps, with the share of the bar each one fills: reading the old log, copying files, writing the lines, finishing.
pub const STEPS: &[(&str, &str, f64, f64)] = &[
    ("reading", "Reading the 0.14 log", 0.0, 30.0),
    ("copying", "Copying attachments, bot folders and account history", 30.0, 35.0),
    ("writing", "Writing projects and threads", 35.0, 99.0),
    ("finishing", "Finishing", 99.0, 100.0),
];

#[derive(Default)]
struct P {
    from: String, started_ms: u64, phase: &'static str, done: u64, total: u64, unit: &'static str,
    projects: (u64, u64), threads: (u64, u64), current: String, samples: VecDeque<(u64, u64)>, secrets: bool, at_ms: u64,
}

#[derive(Default)]
pub struct Progress { p: Mutex<P> }

/// The rate (per second) of `samples` (ms, done) over their span, once the span is a second or more.
pub fn rate(samples: &VecDeque<(u64, u64)>) -> Option<f64> {
    let (a, b) = (samples.front()?, samples.back()?);
    let ms = b.0.saturating_sub(a.0); if ms < 1000 || b.1 <= a.1 { return None }
    Some((b.1 - a.1) as f64 * 1000.0 / ms as f64)
}
/// Seconds left in the step at `rate`, rounded up; none without a rate or a total.
pub fn eta(done: u64, total: u64, rate: Option<f64>) -> Option<u64> { let r = rate?; (total > 0 && r > 0.0).then(|| ((total.saturating_sub(done)) as f64 / r).ceil() as u64) }
/// The whole import's percent: the step's own part of the bar, filled as far as the step is.
pub fn pct(phase: &str, done: u64, total: u64) -> u64 {
    let Some(&(_, _, a, b)) = STEPS.iter().find(|s| s.0 == phase) else { return 0 };
    let f = if total == 0 { 0.0 } else { (done as f64 / total as f64).min(1.0) };
    (a + (b - a) * f).floor() as u64
}

impl Progress {
    pub fn begin(&self, from: &str, secrets: bool, now_ms: u64) { *self.p.lock().unwrap() = P { from: from.into(), started_ms: now_ms, phase: "reading", unit: "bytes", secrets, at_ms: now_ms, ..Default::default() } }
    /// A new step: its total (0 when not known) and what it counts.
    pub fn step(&self, phase: &'static str, total: u64, unit: &'static str, now_ms: u64) { let mut p = self.p.lock().unwrap(); p.phase = phase; p.total = total; p.done = 0; p.unit = unit; p.samples.clear(); p.samples.push_back((now_ms, 0)); p.at_ms = now_ms; }
    /// The step's total, once known (reading learns it as it starts).
    pub fn step_total(&self, total: u64) { self.p.lock().unwrap().total = total; }
    /// The step moved to `done`.
    pub fn at(&self, done: u64, now_ms: u64) {
        let mut p = self.p.lock().unwrap(); p.done = done; p.at_ms = now_ms; p.samples.push_back((now_ms, done));
        while p.samples.len() > 2 && p.samples.front().map_or(false, |s| now_ms.saturating_sub(s.0) > 10_000) { p.samples.pop_front(); }
    }
    pub fn totals(&self, projects: u64, threads: u64) { let mut p = self.p.lock().unwrap(); p.projects.1 = projects; p.threads.1 = threads; }
    /// A written chunk: projects and threads made in it, and the last thread's name.
    pub fn made(&self, projects: u64, threads: u64, current: Option<String>) { let mut p = self.p.lock().unwrap(); p.projects.0 += projects; p.threads.0 += threads; if let Some(c) = current { p.current = c; } }
    /// What everyone may see, `/healthz` and `/hello` included: numbers and the step, no names.
    pub fn public(&self) -> Value {
        let p = self.p.lock().unwrap(); let r = rate(&p.samples);
        let what = STEPS.iter().find(|s| s.0 == p.phase).map_or("", |s| s.1);
        json!({"state": "running", "phase": p.phase, "what": what, "done": p.done, "total": p.total, "unit": p.unit, "pct": pct(p.phase, p.done, p.total),
            "eta_s": eta(p.done, p.total, r), "rate": r.map(|r| r.round() as u64), "at": p.at_ms / 1000, "started": p.started_ms / 1000})
    }
    /// The same with the names, for the person's own clients (the info key, data/1).
    pub fn full(&self, now_ms: u64) -> Value {
        let mut v = self.public(); let p = self.p.lock().unwrap();
        v["from"] = json!(p.from); v["projects"] = json!([p.projects.0, p.projects.1]); v["threads"] = json!([p.threads.0, p.threads.1]);
        v["current"] = json!(p.current); v["secrets"] = json!(p.secrets); v["elapsed_s"] = json!(now_ms.saturating_sub(p.started_ms) / 1000);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_estimate_is_measured_and_none_before_a_second_of_work() {
        let p = Progress::default(); p.begin("/f", false, 0); p.step("writing", 1000, "lines", 0);
        assert_eq!(p.public()["eta_s"], Value::Null, "no rate yet");
        p.at(100, 500); assert_eq!(p.public()["eta_s"], Value::Null, "half a second is not a rate");
        p.at(200, 1000); assert_eq!(p.public()["eta_s"], 4, "200 lines a second, 800 left");
        assert_eq!(p.public()["rate"], 200); assert_eq!(p.public()["pct"], 47, "35 + 64 x 20%");
        p.at(200, 13_000); assert_eq!(p.public()["eta_s"], Value::Null, "the old samples leave: a stall gives no estimate, not a stale one");
    }
    #[test] fn the_bar_only_moves_forward_through_the_steps() {
        let mut last = 0;
        for (ph, d, t) in [("reading", 0, 100), ("reading", 100, 100), ("copying", 0, 0), ("writing", 0, 10), ("writing", 10, 10), ("finishing", 0, 0)] { let x = pct(ph, d, t); assert!(x >= last, "{ph} {x} < {last}"); last = x; }
        assert_eq!(pct("writing", 10, 10), 99);
    }
    #[test] fn names_stay_off_the_public_answer() {
        let p = Progress::default(); p.begin("/home/a/.backplane-bend", false, 0); p.made(1, 1, Some("Nordic · Pick a regulator".into()));
        let pubs = p.public().to_string(); assert!(!pubs.contains("Nordic") && !pubs.contains("/home/a"));
        assert_eq!(p.full(0)["current"], "Nordic · Pick a regulator");
    }
}

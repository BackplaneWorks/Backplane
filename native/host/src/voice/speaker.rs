//! The speaker check of always-on dictation (docs/voice.md "Always on"): only the person's voice starts a message, not the TV, someone else in the
//! room or the speakers. The helper gives each segment of a second or more its embedding (NeMo TitaNet small, `voice` events); the person's
//! voiceprint is their mean, learned from their own dictation (the mic button: whoever presses it is the person), kept in
//! `<home>/voice/voiceprint.json` (0600) and never sent anywhere. Re-enrol clears it. On this desktop (2026-11-11): the person's own passages
//! 0.75 and 0.79 against 20 s of their voice, three other voices below 0.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// A segment this close to the voiceprint is the person's.
pub const MATCH: f32 = 0.5;
/// The voiceprint checks only once it has this many segments.
pub const ENROLLED: u32 = 3;
/// Each new segment weighs at most this much in the mean (later ones keep it current).
const KEEP: u32 = 200;

pub fn path(home: &Path) -> PathBuf { home.join("voice/voiceprint.json") }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Print { pub n: u32, pub mean: Vec<f32> }
fn unit(v: &[f32]) -> Vec<f32> { let n = v.iter().map(|x| x * x).sum::<f32>().sqrt(); if n == 0.0 { v.to_vec() } else { v.iter().map(|x| x / n).collect() } }
pub fn cos(a: &[f32], b: &[f32]) -> f32 { if a.len() != b.len() || a.is_empty() { return 0.0 } let (a, b) = (unit(a), unit(b)); a.iter().zip(&b).map(|(x, y)| x * y).sum() }
impl Print {
    pub fn load(home: &Path) -> Print {
        let v: Value = std::fs::read_to_string(path(home)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        Print { n: v["n"].as_u64().unwrap_or(0) as u32, mean: v["mean"].as_array().map(|a| a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect()).unwrap_or_default() }
    }
    pub fn save(&self, home: &Path) -> bool { crate::net::botnet::write_secret(&path(home), &json!({"n": self.n, "mean": self.mean}).to_string()) }
    pub fn enrolled(&self) -> bool { self.n >= ENROLLED && !self.mean.is_empty() }
    /// One more segment of the person's voice.
    pub fn add(&mut self, emb: &[f32]) {
        let e = unit(emb); if e.is_empty() { return }
        if self.mean.len() != e.len() { *self = Print::default() }
        let w = 1.0 / (self.n.min(KEEP) + 1) as f32;
        self.mean = if self.mean.is_empty() { e } else { self.mean.iter().zip(&e).map(|(m, x)| m * (1.0 - w) + x * w).collect() };
        self.n += 1;
    }
    /// How close a segment is to the person's voice.
    pub fn sim(&self, emb: &[f32]) -> f32 { cos(&self.mean, emb) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_voiceprint_learns_from_the_persons_segments_and_tells_another_voice() {
        let me = |k: f32| vec![1.0, 0.2 + k, 0.1, -0.3]; let other = vec![-0.2, 1.0, -0.5, 0.4];
        let mut p = Print::default(); assert!(!p.enrolled());
        for k in [0.0, 0.05, -0.05] { p.add(&me(k)) } assert!(p.enrolled());
        assert!(p.sim(&me(0.02)) > 0.95 && p.sim(&other) < MATCH);
        let home = crate::plat::tmp::dir("voiceprint"); std::fs::create_dir_all(home.join("voice")).unwrap(); assert!(p.save(&home)); assert_eq!(Print::load(&home).n, 3);
        p.add(&[1.0, 2.0]); assert_eq!(p.n, 1, "another model's size starts again");
    }
}

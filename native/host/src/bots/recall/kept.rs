//! A thread's memory tree as the bots world folds it from the log (RecallNoted, RecallView): read by the bots fold, so an input of its checkpoint (fold_inputs.rs).
use serde_json::Value;
use super::tree::Part;

/// A thread's tree as the log holds it: each written line's place (`line`·64 + its place in its RecallNoted, 0 none) by level and
/// index, and the view a batch merge last kept.
#[derive(Default, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kept { pub nodes: Vec<Vec<u64>>, pub parts: Vec<Part> }
impl Kept {
    /// RecallNoted at log line `line` (1-based): its lines, [[l, i, text], ...]
    pub fn noted(&mut self, line: usize, e: &Value) {
        for (k, n) in e["nodes"].as_array().into_iter().flatten().enumerate().take(64) {
            let (Some(l), Some(i)) = (n[0].as_u64(), n[1].as_u64()) else { continue };
            if l > 40 || i > (1 << 40) { continue }
            let (l, i) = (l as usize, i as usize);
            if self.nodes.len() <= l { self.nodes.resize(l + 1, vec![]) }
            if self.nodes[l].len() <= i { self.nodes[l].resize(i + 1, 0) }
            self.nodes[l][i] = ((line as u64) << 6) | k as u64;
        }
    }
    /// RecallView: the kept view, [[l, i], ...]
    pub fn viewed(&mut self, e: &Value) {
        self.parts = e["parts"].as_array().into_iter().flatten().filter_map(|p| Some((p[0].as_u64()? as u32, p[1].as_u64()?))).collect();
    }
    /// Where a written line is: (log line, 1-based; its place in that event)
    pub fn at(&self, (l, i): Part) -> Option<(usize, usize)> {
        let v = *self.nodes.get(l as usize)?.get(i as usize)?;
        if v == 0 { None } else { Some(((v >> 6) as usize, (v & 63) as usize)) }
    }
    pub fn written(&self) -> usize { self.nodes.iter().map(|l| l.iter().filter(|v| **v != 0).count()).sum() }
}

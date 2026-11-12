//! Each part's history (docs/MANAGEMENT.md, "Lifecycle"): every fresh lookup of the fab's stock (kicad/parts.rs) and every lifecycle reading
//! is a sample appended to `<home>/parts/history/<key digest>.jsonl` (one JSON object a line: {at, key, stock, class, price, breaks,
//! lifecycle, fab, model}). Append-only and bounded: past MAX + MAX/4 lines a file keeps its newest MAX. It starts empty: no part has history
//! from before Backplane looked it up, but the fab's own recorded stock changes (`source: "fab_history"`, jlc_stock_history) are added
//! once a week (kicad/parts.rs). A part's samples are read oldest first.
use super::parts::Quote;
use serde_json::{json, Value};
use std::{io::Write, path::{Path, PathBuf}};

pub const MAX: usize = 2000;

fn digest(key: &str) -> String { format!("{:016x}", key.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))) }
pub fn file(home: &Path, key: &str) -> PathBuf { home.join("parts").join("history").join(format!("{}.jsonl", digest(key))) }

/// One sample of a quote.
pub fn sample(key: &str, q: &Quote, lifecycle: &str) -> Value {
    json!({"at": q.at, "key": key, "stock": q.stock, "class": q.class, "price": q.breaks.first().map(|b| b.1), "breaks": q.breaks, "lifecycle": lifecycle, "fab": q.fab, "model": q.model})
}
/// Append a line to a key's file, trimming it to its newest MAX past the bound.
pub fn append(home: &Path, key: &str, v: &Value) {
    let p = file(home, key);
    if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) { let _ = writeln!(f, "{v}"); }
    let n = std::fs::read_to_string(&p).map(|t| t.lines().count()).unwrap_or(0);
    if n > MAX + MAX / 4 {
        let t = std::fs::read_to_string(&p).unwrap_or_default();
        let keep: Vec<&str> = t.lines().skip(n - MAX).collect();
        let tmp = p.with_extension("jsonl.tmp");
        if std::fs::write(&tmp, keep.join("\n") + "\n").is_ok() { let _ = std::fs::rename(&tmp, &p); }
    }
}
/// A fresh quote of the fab's stock, with the part's last known lifecycle.
pub fn record(home: &Path, key: &str, q: &Quote) {
    let life = read(home, key).iter().rev().find_map(|s| s["lifecycle"].as_str().filter(|l| !l.is_empty()).map(str::to_string)).unwrap_or_default();
    append(home, key, &sample(key, q, &life));
}
/// A key's samples, oldest first.
pub fn read(home: &Path, key: &str) -> Vec<Value> {
    // oldest first by time: the fab's own past (kicad/parts.rs stock_history) is appended after samples that are newer than it
    let mut v: Vec<Value> = std::fs::read_to_string(file(home, key)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|v| v["key"] == key).collect();
    v.sort_by_key(|s| s["at"].as_u64().unwrap_or(0)); v
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn samples_are_appended_and_bounded() {
        let d = crate::plat::tmp::dir("parthist");
        let q = |stock: i64, at: u64| Quote { fab: "C1".into(), stock, breaks: vec![(1, 0.5)], at, ..Default::default() };
        record(&d, "mpn:A", &q(10, 1)); record(&d, "mpn:A", &q(8, 2)); record(&d, "mpn:B", &q(3, 2));
        let a = read(&d, "mpn:A"); assert_eq!(a.len(), 2); assert_eq!((a[0]["stock"].as_i64(), a[1]["at"].as_u64(), a[1]["price"].as_f64()), (Some(10), Some(2), Some(0.5)));
        assert_eq!(read(&d, "mpn:B").len(), 1); assert!(read(&d, "mpn:C").is_empty());
        append(&d, "mpn:A", &json!({"at": 3, "key": "mpn:A", "stock": 8, "lifecycle": "NRND"}));
        record(&d, "mpn:A", &q(7, 4)); assert_eq!(read(&d, "mpn:A").last().unwrap()["lifecycle"], "NRND", "the lifecycle carries on to the next stock sample");
        for i in 0..(MAX + MAX / 4) { append(&d, "mpn:B", &json!({"at": 10 + i, "key": "mpn:B", "stock": i})) }
        let b = read(&d, "mpn:B"); assert_eq!(b.len(), MAX); assert_eq!(b.last().unwrap()["at"].as_u64(), Some((10 + MAX + MAX / 4 - 1) as u64), "the newest are kept");
    }
}

//! Start-up checkpoints (docs/STARTUP.md). The hub's folds of the log (the line index, the read model, the policies' state) are
//! deterministic functions of the first N lines, so each is saved with N and a fingerprint of the log at N, in `<home>/ckpt/`. A start
//! loads what is valid and folds only the lines past N; anything missing, torn, from another version or from a different log is
//! ignored and the fold runs over the whole log as before. Files are written beside and renamed, never in place.
use crate::store::Store;
use std::path::{Path, PathBuf};

pub const VERSION: u64 = 1;
/// One number per fold: a hash of the code it is made of (`fold_inputs.rs`, `build.rs`). `VERSION` says what a checkpoint's file looks like; this says which fold wrote what is in it.
/// A fold that changes reads the same log differently, so its old checkpoint is ignored and the log folded again. No one has to remember to bump anything.
pub mod stamps { include!(concat!(env!("OUT_DIR"), "/fold_stamps.rs")); }

/// `<home>/ckpt` for a log at `<home>/events.jsonl`.
pub fn dir(events: &Path) -> PathBuf { events.parent().unwrap_or(Path::new(".")).join("ckpt") }

pub fn fnv(bytes: &[u8]) -> u64 { let mut h = 0xcbf29ce484222325u64; for b in bytes { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); } h }

/// Written whole beside and renamed over: a reader never sees a half file.
pub fn write_atomic(path: &Path, parts: &[&[u8]]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(d) = path.parent() { std::fs::create_dir_all(d)?; }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let r = (|| { let mut f = std::fs::File::create(&tmp)?; for p in parts { f.write_all(p)?; } drop(f); std::fs::rename(&tmp, path) })();
    if r.is_err() { let _ = std::fs::remove_file(&tmp); } // a full disk must not fill further with half files
    r
}

/// The log's identity through line `n`: the count, the first line and the last one. An append-only log that still has these is the log
/// the checkpoint was made from.
pub fn fingerprint(store: &Store, n: usize) -> u64 {
    if n == 0 || n > store.len() { return 0 }
    let (a, b) = (store.get(0).unwrap_or_default(), store.get(n - 1).unwrap_or_default());
    fnv(a.as_bytes()) ^ fnv(b.as_bytes()).rotate_left(21) ^ (n as u64).wrapping_mul(0x9e3779b97f4a7c15)
}

pub fn le32(v: &[u8], i: usize) -> u32 { u32::from_le_bytes([v[i], v[i + 1], v[i + 2], v[i + 3]]) }
pub fn le64(v: &[u8], i: usize) -> u64 { u64::from_le_bytes([v[i], v[i + 1], v[i + 2], v[i + 3], v[i + 4], v[i + 5], v[i + 6], v[i + 7]]) }

type SaverFn = Box<dyn Fn(&Store) + Send + Sync>;
static SAVERS: std::sync::Mutex<Vec<SaverFn>> = std::sync::Mutex::new(Vec::new());
static LAST: std::sync::Mutex<Vec<(String, usize)>> = std::sync::Mutex::new(Vec::new());
/// A fold that keeps its own checkpoint registers how to save it; each round of the task below calls it (it decides itself whether it is due, `due`).
pub fn register(f: impl Fn(&Store) + Send + Sync + 'static) { SAVERS.lock().unwrap().push(Box::new(f)); }
/// Whether the fold `name`, now covering `lines`, has moved enough (or was never saved) to write its checkpoint again.
pub fn due(name: &str, lines: usize) -> bool { lines > 0 && LAST.lock().unwrap().iter().find(|(n, _)| n == name).map_or(true, |(_, l)| lines >= *l + 5_000) }
/// The fold `name` covers `lines` in its checkpoint (just loaded or just saved).
pub fn saved(name: &str, lines: usize) { let mut l = LAST.lock().unwrap(); match l.iter_mut().find(|(n, _)| n == name) { Some(e) => e.1 = lines, None => l.push((name.to_string(), lines)) } }

/// A map whose keys are pairs, as a list of (key, value): JSON keys are text.
pub mod pairs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;
    pub fn serialize<S: Serializer>(m: &HashMap<(String, String), usize>, s: S) -> Result<S::Ok, S::Error> { m.iter().collect::<Vec<_>>().serialize(s) }
    pub fn deserialize<'d, D: Deserializer<'d>>(d: D) -> Result<HashMap<(String, String), usize>, D::Error> { Ok(Vec::<((String, String), usize)>::deserialize(d)?.into_iter().collect()) }
}

/// A fold saved as JSON with the number of log lines it covers and the log's fingerprint there.
pub fn save_json<T: serde::Serialize>(path: &Path, store: &Store, lines: usize, stamp: u64, v: &T) {
    #[derive(serde::Serialize)] struct Snap<'a, T> { v: u64, stamp: u64, lines: usize, fp: u64, data: &'a T }
    let r = serde_json::to_vec(&Snap { v: VERSION, stamp, lines, fp: fingerprint(store, lines), data: v }).map_err(std::io::Error::from).and_then(|b| write_atomic(path, &[&b]));
    if let Err(e) = r { eprintln!("checkpoint: {} not saved: {e}", path.display()) }
}
/// `save_json`'s file, when it is of this log: the value, which `lines_of` says covers that many lines.
pub fn load_json<T: serde::de::DeserializeOwned>(path: &Path, store: &Store, stamp: u64, lines_of: impl Fn(&T) -> usize) -> Option<T> {
    #[derive(serde::Deserialize)] struct Snap<T> { v: u64, #[serde(default)] stamp: u64, lines: usize, fp: u64, data: T }
    let snap: Snap<T> = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if snap.v != VERSION || snap.stamp != stamp || snap.lines > store.len() || snap.fp != fingerprint(store, snap.lines) || lines_of(&snap.data) != snap.lines { return None }
    Some(snap.data)
}

/// `BACKPLANE_CKPT_CHECK=1`: after a checkpoint loads, the whole log is folded the old way as well and the two are compared (the differing part is named in the log).
/// For proving a checkpoint on a real log; it costs what the old start cost, so it is off.
pub fn check_on() -> bool { std::env::var("BACKPLANE_CKPT_CHECK").map_or(false, |v| v == "1") }
/// Sets serialize in hash order: every array is sorted (by its text) before two values are compared.
fn sorted(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Array(a) => { let mut a: Vec<Value> = a.into_iter().map(sorted).collect(); a.sort_by_cached_key(|x| x.to_string()); Value::Array(a) }
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, sorted(v))).collect()),
        v => v,
    }
}
pub fn check(name: &str, from_checkpoint: serde_json::Value, from_log: serde_json::Value) {
    let (from_checkpoint, from_log) = (sorted(from_checkpoint), sorted(from_log));
    if from_checkpoint == from_log { eprintln!("ckpt-check {name}: equal"); return }
    let diff = match (&from_checkpoint, &from_log) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => a.keys().chain(b.keys()).filter(|k| a.get(*k) != b.get(*k)).cloned().collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>().join(","),
        _ => "(not objects)".into(),
    };
    eprintln!("ckpt-check {name}: DIFFER in {diff}");
}
pub fn run_savers(store: &Store) { for f in SAVERS.lock().unwrap().iter() { f(store) } }

type Pre = std::thread::JoinHandle<Option<Box<dyn std::any::Any + Send>>>;
static PRE: std::sync::Mutex<Vec<(String, Pre)>> = std::sync::Mutex::new(Vec::new());
/// Start reading a checkpoint on a thread of its own, at once, so the loads that start needs overlap instead of following each other.
pub fn preload<T: Send + 'static>(name: &str, store: &Store, f: impl FnOnce() -> Option<T> + Send + 'static) {
    PRE.lock().unwrap().push((format!("{name}:{}", store.path().display()), std::thread::spawn(move || f().map(|v| Box::new(v) as Box<dyn std::any::Any + Send>))));
}
/// The checkpoint `preload` was reading (waited for), else `f` now.
pub fn take<T: 'static>(name: &str, store: &Store, f: impl FnOnce() -> Option<T>) -> Option<T> {
    let key = format!("{name}:{}", store.path().display());
    let h = { let mut p = PRE.lock().unwrap(); p.iter().position(|(n, _)| *n == key).map(|i| p.remove(i).1) };
    match h { Some(h) => h.join().ok().flatten().and_then(|b| b.downcast::<T>().ok()).map(|b| *b), None => f() }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rig(t: &str) -> (PathBuf, Store) {
        let d = std::env::temp_dir().join(format!("bp-ckpt-{}-{t}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        let p = d.join("events.jsonl"); std::fs::write(&p, "{\"$\":\"A\",\"at\":1}\n").unwrap(); let s = Store::open(&p).unwrap(); (d, s)
    }
    fn load(p: &Path, s: &Store, stamp: u64) -> Option<usize> { load_json(p, s, stamp, |n: &usize| *n) }
    #[test] fn a_checkpoint_loads_only_for_the_fold_that_wrote_it() {
        let (d, s) = rig("stamp"); let p = d.join("x.json"); save_json(&p, &s, 1, 11, &1usize);
        assert_eq!(load(&p, &s, 11), Some(1), "the same fold: it must load, or every start would refold");
        assert_eq!(load(&p, &s, 12), None, "another fold read this log differently: it folds the log itself");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_checkpoint_from_before_stamps_is_not_trusted() {
        let (d, s) = rig("old"); let p = d.join("x.json");
        std::fs::write(&p, format!("{{\"v\":{VERSION},\"lines\":1,\"fp\":{},\"data\":1}}", fingerprint(&s, 1))).unwrap();
        assert_eq!(load(&p, &s, stamps::COMMANDS), None, "this is what repairs a home a stale checkpoint already lost state on");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn every_fold_has_its_own_stamp() {
        let all = [stamps::MODEL, stamps::STATE, stamps::COMMANDS, stamps::AGENTS, stamps::BOTS, stamps::WAKES, stamps::FEED, stamps::PEERS];
        let mut u = all.to_vec(); u.sort(); u.dedup(); assert_eq!(u.len(), all.len(), "one fold changing must not refold the others"); assert!(!all.contains(&0));
    }
}

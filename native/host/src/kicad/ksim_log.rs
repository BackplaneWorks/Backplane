//! The simulation log (docs/kicad-sim.md, "The log"): one JSON line per `kicad_sim` op in `<home>/logs/sim.jsonl`, and one per turn
//! that simulated with what the turn cost in tokens, so a benchmark reads what each step took: the fork's timings, ngspice's run time,
//! failures, where each model came from, the size of the answer the agent got. Append-only; a line that does not parse is skipped.
use serde_json::{json, Map, Value};
use std::{io::Write, path::{Path, PathBuf}};

pub fn file(home: &Path) -> PathBuf { home.join("logs").join("sim.jsonl") }
/// Append one line (the folder is made on the first).
pub fn append(path: &Path, line: &Value) {
    if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) { let _ = writeln!(f, "{line}"); }
}
/// A call's op entries as log lines: each with its time, thread, turn (the time the turn started) and the document.
pub fn lines(entries: &[Value], at_ms: u64, thread: &str, turn: u64, path: &str) -> Vec<Value> {
    entries.iter().map(|e| { let mut l = json!({"at": at_ms, "thread": thread, "turn": turn, "path": path}); for (k, v) in e.as_object().into_iter().flatten() { l[k.as_str()] = v.clone() } l }).collect()
}
/// A turn's line: what it cost. `ring` is the thread's last-turn reading "tokens/cost/context" (state's `tok.*.tr.<thread>` entry).
pub fn turn_line(at_ms: u64, thread: &str, turn: u64, ring: &str) -> Value {
    let mut p = ring.split('/').map(|x| x.parse::<u64>().ok());
    json!({"at": at_ms, "thread": thread, "turn": turn, "kind": "turn", "tokens": p.next().flatten(), "cost_nusd": p.next().flatten(), "context": p.next().flatten()})
}
pub fn read(path: &Path) -> Vec<Value> { std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect() }
/// The log in totals, of one thread or of all: per op its count, runs, failures, times and answer bytes; the failed statuses; where
/// models came from; and each turn that simulated with its actions, answer bytes and tokens.
pub fn summary(entries: &[Value], thread: &str) -> Value {
    let mine: Vec<&Value> = entries.iter().filter(|e| thread.is_empty() || e["thread"] == thread).collect();
    let (mut ops, mut statuses, mut sources, mut turns): (Map<String, Value>, Map<String, Value>, Map<String, Value>, Vec<Value>) = Default::default();
    let bump = |m: &mut Map<String, Value>, k: &str, by: f64| { let v = m.entry(k.to_string()).or_insert(json!(0.0)); *v = json!(v.as_f64().unwrap_or(0.0) + by) };
    for e in mine.iter().filter(|e| e["kind"] != "turn") {
        let op = e["op"].as_str().unwrap_or("?"); let o = ops.entry(op.to_string()).or_insert(json!({"count": 0, "runs": 0, "failed": 0, "hub_ms": 0.0, "ngspice_ms": 0.0, "bytes": 0}));
        let add = |o: &mut Value, k: &str, by: f64| o[k] = json!(o[k].as_f64().unwrap_or(0.0) + by);
        add(o, "count", 1.0); add(o, "runs", e["runs"].as_f64().unwrap_or(0.0)); add(o, "hub_ms", e["hub_ms"].as_f64().unwrap_or(0.0)); add(o, "ngspice_ms", e["ngspice_ms"].as_f64().unwrap_or(0.0)); add(o, "bytes", e["bytes"].as_f64().unwrap_or(0.0));
        let bad = e["ok"] == false || e["status"].as_str().is_some_and(|s| s != "SIM_OK"); if bad { add(o, "failed", 1.0); bump(&mut statuses, e["status"].as_str().unwrap_or("error"), 1.0) }
        for l in e["lookups"].as_array().into_iter().flatten() { let from = l["from"].as_str().or(l["found"][0]["from"].as_str()).unwrap_or("none"); bump(&mut sources, from, 1.0) }
    }
    let mut keys: Vec<(String, u64)> = mine.iter().map(|e| (e["thread"].as_str().unwrap_or("").to_string(), e["turn"].as_u64().unwrap_or(0))).collect(); keys.dedup(); let mut seen = std::collections::HashSet::new(); keys.retain(|k| seen.insert(k.clone()));
    for (th, turn) in keys {
        let of: Vec<&&Value> = mine.iter().filter(|e| e["thread"] == th.as_str() && e["turn"].as_u64().unwrap_or(0) == turn).collect();
        let acts: Vec<&&&Value> = of.iter().filter(|e| e["kind"] != "turn").collect(); let cost = of.iter().find(|e| e["kind"] == "turn");
        turns.push(json!({"thread": th, "turn": turn, "actions": acts.len(), "runs": acts.iter().filter_map(|e| e["runs"].as_u64()).sum::<u64>(), "bytes": acts.iter().filter_map(|e| e["bytes"].as_u64()).sum::<u64>(),
            "ngspice_ms": acts.iter().filter_map(|e| e["ngspice_ms"].as_f64()).sum::<f64>(), "failed": acts.iter().filter(|e| e["ok"] == false || e["status"].as_str().is_some_and(|s| s != "SIM_OK")).count(),
            "tokens": cost.map_or(Value::Null, |c| c["tokens"].clone()), "cost_nusd": cost.map_or(Value::Null, |c| c["cost_nusd"].clone())}));
    }
    let more = turns.len().saturating_sub(40); let turns: Vec<Value> = turns.into_iter().skip(more).collect();
    json!({"actions": mine.iter().filter(|e| e["kind"] != "turn").count(), "ops": ops, "failures": statuses, "models_from": sources, "turns": turns})
}

/// The threads that simulated in their running turn, each with the token reading it had then (the turn's own is the next one).
fn marks() -> &'static std::sync::Mutex<std::collections::HashMap<String, (u64, String)>> {
    static S: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (u64, String)>>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}
/// A thread simulated in the turn that started at `turn`; `ring` is its token readings so far.
pub fn mark(thread: &str, turn: u64, ring: &str) { marks().lock().unwrap().entry(thread.to_string()).or_insert((turn, ring.to_string())); }
/// The mark of a thread whose turn ended, taken.
pub fn take(thread: &str) -> Option<(u64, String)> { marks().lock().unwrap().remove(thread) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_log_is_lines_and_sums() {
        let d = std::env::temp_dir().join(format!("ksim-log-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); let p = file(&d);
        assert!(read(&p).is_empty() && p.ends_with("logs/sim.jsonl"));
        let entries = [json!({"op": "models", "ok": true, "hub_ms": 12, "runs": 0, "bytes": 400, "lookups": [{"ref": "U1", "found": [{"from": "project"}]}, {"ref": "U2", "found": []}]}),
            json!({"op": "run", "ok": true, "hub_ms": 40, "runs": 1, "analysis": "AC", "status": "SIM_OK", "ngspice_ms": 8.0, "bytes": 300, "timings": {"run_ms": 5.0}}),
            json!({"op": "run", "ok": true, "hub_ms": 30, "runs": 1, "analysis": "TRAN", "status": "SIM_NO_CONVERGENCE", "ngspice_ms": 20.0, "bytes": 250, "problems": ["no convergence"]})];
        for l in lines(&entries, 1000, "t1", 900, "/p/a.kicad_sch") { append(&p, &l) }
        for l in lines(&[json!({"op": "sweep", "ok": true, "hub_ms": 90, "runs": 5, "status": "SIM_OK", "ngspice_ms": 40.0, "bytes": 200}), json!({"op": "import", "ok": true, "hub_ms": 2, "runs": 0, "bytes": 90, "lookups": [{"stored": "sim/models/x.lib", "from": "vendor"}]})], 2000, "t2", 1900, "/p/a.kicad_sch") { append(&p, &l) }
        append(&p, &turn_line(3000, "t1", 900, "52000/1300000/41000")); std::fs::OpenOptions::new().append(true).open(&p).unwrap().write_all(b"not json\n").unwrap();
        let all = read(&p); assert_eq!((all.len(), all[0]["thread"].as_str(), all[0]["turn"].as_u64(), all[0]["path"].as_str(), all[1]["analysis"].as_str()), (6, Some("t1"), Some(900), Some("/p/a.kicad_sch"), Some("AC")));
        let s = summary(&all, "");
        assert_eq!((s["actions"].as_u64(), s["ops"]["run"].clone(), s["failures"].clone(), s["models_from"].clone()), (Some(5), json!({"count": 2.0, "runs": 2.0, "failed": 1.0, "hub_ms": 70.0, "ngspice_ms": 28.0, "bytes": 550.0}), json!({"SIM_NO_CONVERGENCE": 1.0}), json!({"project": 1.0, "none": 1.0, "vendor": 1.0})));
        assert_eq!(s["turns"], json!([{"thread": "t1", "turn": 900, "actions": 3, "runs": 2, "bytes": 950, "ngspice_ms": 28.0, "failed": 1, "tokens": 52000, "cost_nusd": 1300000}, {"thread": "t2", "turn": 1900, "actions": 2, "runs": 5, "bytes": 290, "ngspice_ms": 40.0, "failed": 0, "tokens": null, "cost_nusd": null}]));
        assert_eq!((summary(&all, "t2")["actions"].as_u64(), summary(&all, "t2")["turns"].as_array().map(|t| t.len()), summary(&all, "nobody")["actions"].as_u64()), (Some(2), Some(1), Some(0)));
        assert_eq!(turn_line(1, "t", 2, ""), json!({"at": 1, "thread": "t", "turn": 2, "kind": "turn", "tokens": null, "cost_nusd": null, "context": null}));
        mark("tx", 5, "a"); mark("tx", 6, "b"); assert_eq!((take("tx"), take("tx")), (Some((5, "a".to_string())), None));
        let _ = std::fs::remove_dir_all(&d);
    }
}

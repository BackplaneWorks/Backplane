//! Local diagnostics: a bounded recorder of closed, numeric
//! measurements in `<home>/diagnostics/` (a control byte and eight 64 KiB segments, 1 MiB in all, 7 days). No network sender, no identifiers,
//! no free text. Requests `diagnostics.status` and `diagnostics.control` {action: on|off|clear}; `export()` is the sanitized document.
//! Not ported: the hub-exit record (#590, needs a process exit hook) and the minute memory samples; `note` is the entry point for the perf
//! feeds when the hub has them.
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, io::{Read, Write}, os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt}, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::{Duration, Instant}};

const DISK: u64 = 1_048_576; const SEG: u64 = 65_536; const SEGMENTS: usize = 8; const RETAIN: u64 = 7 * 86400; const RECORD: usize = 512; const EXPORT: usize = 1_048_576;
const FLUSH: Duration = Duration::from_secs(30); const RETRY: Duration = Duration::from_secs(60);
const OPS: &[&str] = &["hub.rpc", "hub.mcp", "hub.join", "hub.mobile_join", "hub.join_here", "hub.leave", "hub.thin-read", "hub.agent_line", "hub.agent_gone", "hub.tick", "hub.heart", "hub.commit", "hub.push", "hub.push_bin",
    "hub.step.busy", "hub.step.far", "hub.step.outs", "hub.step.pend", "hub.step.commit", "boot.store", "boot.replay", "boot.files", "boot.kicad", "boot.cfg", "boot.thin", "boot.mail", "boot.origin", "boot.helpers", "boot.adopt", "boot.listen", "thin.warm", "mail.warm"];
const ERRORS: &[&str] = &["heap_growth", "store_write", "mail_append", "disk_full", "io", "timeout", "protocol", "agent_exit", "log_error", "log_warn"];
const CONNS: &[&str] = &["join", "mobile_join", "leave", "drop", "connect", "disconnect", "reconnect", "failed"];
const GAUGES: &[&str] = &["hub.clients", "hub.agents", "hub.busy", "memory.rss_kib", "memory.heap_kib", "memory.external_kib"];

struct S { dir: PathBuf, ready: bool, enabled: bool, error: &'static str, pending: BTreeMap<String, Value>, bytes: u64, allocated: u64, seq: u64, dropped: u64, retry: Option<Instant>, flushed: Instant }
static ST: Mutex<Option<S>> = Mutex::new(None);
/// Held by every test that touches the process-wide recorder (this module's and the HTTP route's): `export` flushes and `control` clears it, so one
/// running beside another test's notes would take them.
#[cfg(test)] pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());
fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
fn uint(v: &Value, max: u64) -> bool { v.as_u64().map_or(false, |n| n <= max) }
fn err_code(e: &std::io::Error) -> &'static str { match e.raw_os_error() { Some(c) if c == libc::ENOSPC || c == libc::EDQUOT => "full", _ => match e.kind() { std::io::ErrorKind::Other if e.to_string() == "UNSAFE" => "unsafe_storage", std::io::ErrorKind::Other if e.to_string() == "CAP" => "capacity", _ => "io" } } }
fn fault(m: &str) -> std::io::Error { std::io::Error::new(std::io::ErrorKind::Other, m.to_string()) }
fn names() -> Vec<String> { let mut v = vec!["control".to_string()]; v.extend((0..SEGMENTS).map(|i| format!("segment-{i}.jsonl"))); v }

fn lstat(dir: &Path, name: &str) -> std::io::Result<Option<std::fs::Metadata>> {
    match std::fs::symlink_metadata(dir.join(name)) { Ok(m) => { if !m.is_file() || m.nlink() != 1 { return Err(fault("UNSAFE")) } Ok(Some(m)) } Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None), Err(e) => Err(e) }
}
fn directory(dir: &Path) -> std::io::Result<()> {
    let m = std::fs::symlink_metadata(dir)?; if !m.is_dir() { return Err(fault("UNSAFE")) }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let ok = names(); for e in std::fs::read_dir(dir)? { if !ok.contains(&e?.file_name().to_string_lossy().to_string()) { return Err(fault("UNSAFE")) } } Ok(())
}
fn usage(s: &mut S) -> std::io::Result<u64> {
    let (mut b, mut a) = (0, 0);
    for n in names() { if let Some(m) = lstat(&s.dir, &n)? { b += m.len(); a += m.len().max(m.blocks() * 512); } }
    s.bytes = b; s.allocated = a; Ok(b.max(a))
}
fn write_control(s: &mut S, on: bool) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new().write(true).create(true).custom_flags(libc::O_NOFOLLOW).mode(0o600).open(s.dir.join("control"))?;
    f.set_len(0)?; f.write_all(if on { b"1" } else { b"0" })?; f.sync_all()?;
    if usage(s)? > DISK { return Err(fault("CAP")) } Ok(())
}
fn fail(s: &mut S, e: &std::io::Error) { s.error = err_code(e); s.retry = Some(Instant::now() + RETRY); s.dropped = s.dropped.saturating_add(s.pending.len() as u64).min(u32::MAX as u64); s.pending.clear(); let _ = usage(s); }

/// Records are rebuilt through the closed schema; nothing unknown is copied, even from an edited file.
fn clean(r: &Value) -> Option<Value> {
    if r["v"] != 1 || !uint(&r["seq"], 1 << 53) || !uint(&r["at"], u32::MAX as u64) || !uint(&r["count"], u32::MAX as u64) || r["count"] == 0 { return None }
    let mut o = Map::new(); for k in ["v", "seq", "at", "event"] { o.insert(k.into(), r[k].clone()); }
    let name = r["name"].as_str().unwrap_or("");
    match r["event"].as_str()? {
        "latency" if OPS.contains(&name) && uint(&r["us"], 1 << 53) && r["maxMs"].as_f64().map_or(false, |m| (0.0..=4294967.295).contains(&m)) => { for k in ["name", "count", "us", "maxMs"] { o.insert(k.into(), r[k].clone()); } }
        "error" if ERRORS.contains(&r["code"].as_str().unwrap_or("")) => { for k in ["code", "count"] { o.insert(k.into(), r[k].clone()); } }
        "connection" if name.strip_prefix("connection.").map_or(false, |d| CONNS.contains(&d)) => { for k in ["name", "count"] { o.insert(k.into(), r[k].clone()); } }
        "sample" if GAUGES.contains(&name) && uint(&r["value"], u32::MAX as u64) => { for k in ["name", "count", "value"] { o.insert(k.into(), r[k].clone()); } }
        _ => return None,
    }
    Some(Value::Object(o))
}

struct File { i: usize, seq: u64, size: u64, rows: Vec<Value> }
/// Retention, torn tails and the capacity reservation (diagnostics_scan).
fn scan(s: &mut S) -> std::io::Result<Vec<File>> {
    directory(&s.dir)?; let now = now_s(); let mut files = vec![];
    for i in 0..SEGMENTS {
        let name = format!("segment-{i}.jsonl"); let Some(m) = lstat(&s.dir, &name)? else { continue };
        let p = s.dir.join(&name);
        if m.len() > SEG { std::fs::remove_file(&p)?; continue }
        let mut buf = vec![]; std::fs::File::open(&p)?.read_to_end(&mut buf)?;
        let size = buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1) as u64;
        if size != m.len() { std::fs::OpenOptions::new().write(true).open(&p)?.set_len(size)?; }
        let (mut rows, mut invalid) = (vec![], false);
        for line in String::from_utf8_lossy(&buf[..size as usize]).split('\n').filter(|l| !l.is_empty()) {
            if line.len() > RECORD { invalid = true; break }
            match serde_json::from_str::<Value>(line).ok().as_ref().and_then(clean) { Some(r) if r["at"].as_u64().unwrap_or(0) + RETAIN >= now && r["at"].as_u64().unwrap_or(0) <= now => rows.push(r), _ => { invalid = true; break } }
        }
        if invalid || rows.is_empty() { std::fs::remove_file(&p)?; continue }
        let seq = rows.iter().filter_map(|r| r["seq"].as_u64()).max().unwrap_or(0); s.seq = s.seq.max(seq);
        files.push(File { i, seq, size, rows });
    }
    files.sort_by_key(|f| f.seq);
    while usage(s)? > DISK - SEG - 8192 && !files.is_empty() { let f = files.remove(0); std::fs::remove_file(s.dir.join(format!("segment-{}.jsonl", f.i)))?; }
    if usage(s)? > DISK - SEG - 8192 { return Err(fault("CAP")) }
    Ok(files)
}

fn flush_inner(s: &mut S) -> bool {
    if !s.ready { return false }
    if s.error != "" { if let Some(t) = s.retry { if Instant::now() < t { return false } } }
    let mut attempted = 0u64;
    let r = (|| -> std::io::Result<bool> {
        let files = scan(s)?; s.error = "";
        if !s.enabled || s.pending.is_empty() { return Ok(true) }
        let at = now_s();
        let mut rows = vec![]; let pend: Vec<Value> = s.pending.values().cloned().collect();
        for mut r in pend { if r["at"].as_u64().unwrap_or(0) + RETAIN >= at && r["at"].as_u64().unwrap_or(0) <= at { s.seq += 1; let o = r.as_object_mut().unwrap(); let mut n = Map::new(); n.insert("v".into(), json!(1)); n.insert("seq".into(), json!(s.seq)); for (k, v) in std::mem::take(o) { n.insert(k, v); } rows.push(Value::Object(n)); } }
        attempted = rows.len() as u64; s.pending.clear();
        if rows.is_empty() { return Ok(true) }
        let data: String = rows.iter().map(|r| format!("{r}\n")).collect();
        if data.len() as u64 > SEG { return Err(fault("CAP")) }
        let (idx, size) = match files.last() {
            Some(f) if f.size + data.len() as u64 <= SEG => (f.i, f.size),
            _ => { let used: Vec<usize> = files.iter().map(|f| f.i).collect(); let i = (0..SEGMENTS).find(|k| !used.contains(k)).unwrap_or_else(|| files[0].i); if used.contains(&i) { std::fs::remove_file(s.dir.join(format!("segment-{i}.jsonl")))?; } (i, 0) }
        };
        if usage(s)? + data.len() as u64 + 8192 > DISK { return Err(fault("CAP")) }
        let f = std::fs::OpenOptions::new().read(true).write(true).create(true).custom_flags(libc::O_NOFOLLOW).mode(0o600).open(s.dir.join(format!("segment-{idx}.jsonl")))?;
        if f.metadata()?.len() != size { return Err(fault("UNSAFE")) }
        use std::os::unix::fs::FileExt;
        let w = f.write_all_at(data.as_bytes(), size).and_then(|_| if usage(s)? > DISK { Err(fault("CAP")) } else { Ok(()) });
        if let Err(e) = w { if f.set_len(size).is_err() { let _ = std::fs::remove_file(s.dir.join(format!("segment-{idx}.jsonl"))); } return Err(e) }
        usage(s)?; s.flushed = Instant::now(); Ok(true)
    })();
    match r { Ok(v) => v, Err(e) => { s.dropped = s.dropped.saturating_add(attempted).min(u32::MAX as u64); fail(s, &e); false } }
}

fn status_value(s: Option<&S>, ok: Option<bool>) -> Value {
    let ready = s.map_or(false, |s| s.ready);
    json!({"version":1,"ok":ok.unwrap_or(ready && s.map_or(false, |s| s.error.is_empty())),"started":ready,"enabled":s.map_or(false, |s| s.enabled),"error":s.map_or("", |s| s.error),
        "stored_bytes":s.map_or(0, |s| s.bytes),"allocated_bytes":s.map_or(0, |s| s.allocated),"cap_bytes":DISK,"retention_seconds":RETAIN,"pending":s.map_or(0, |s| s.pending.len()),"dropped":s.map_or(0, |s| s.dropped)})
}

/// Diagnostics.start: binds the home once (idempotent). A failed start disables collection; the hub still starts.
pub fn start(home: &Path) -> Value { let mut g = ST.lock().unwrap_or_else(|e| e.into_inner()); start_in(&mut g, home) }
fn start_in(g: &mut Option<S>, home: &Path) -> Value {
    if let Some(s) = g.as_ref() { return status_value(Some(s), None) }
    let mut s = S { dir: home.join("diagnostics"), ready: false, enabled: false, error: "", pending: BTreeMap::new(), bytes: 0, allocated: 0, seq: 0, dropped: 0, retry: None, flushed: Instant::now() };
    let r = (|| -> std::io::Result<()> {
        if !home.is_absolute() { return Err(fault("UNSAFE")) }
        match std::fs::DirBuilder::new().mode(0o700).create(&s.dir) { Ok(()) => {} Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {} Err(e) => return Err(e) }
        directory(&s.dir)?;
        match lstat(&s.dir, "control")? {
            None => { write_control(&mut s, true)?; s.enabled = true }
            Some(m) if m.len() > 1 => write_control(&mut s, false)?,
            Some(_) => { let mut b = [0u8; 1]; s.enabled = std::fs::File::open(s.dir.join("control"))?.read(&mut b)? == 1 && b[0] == b'1'; }
        }
        scan(&mut s)?; s.ready = true; Ok(())
    })();
    if let Err(e) = r { s.enabled = false; fail(&mut s, &e); }
    let v = status_value(Some(&s), Some(s.ready)); *g = Some(s); v
}

pub fn status() -> Value { let g = ST.lock().unwrap_or_else(|e| e.into_inner()); status_value(g.as_ref(), None) }
pub fn flush() -> Value { let mut g = ST.lock().unwrap_or_else(|e| e.into_inner()); let ok = g.as_mut().map(flush_inner); status_value(g.as_ref(), Some(ok.unwrap_or(false))) }
/// Once a second from the hub's tick: flush every 30 s.
pub fn tick() { let mut g = ST.lock().unwrap_or_else(|e| e.into_inner()); if let Some(s) = g.as_mut() { if s.ready && s.flushed.elapsed() >= FLUSH { flush_inner(s); s.flushed = Instant::now(); } } }

/// Diagnostics.event: only closed event/detail pairs; never pass error messages, RPC parameters or a user's text.
pub fn note(event: &str, detail: &str, value: u64) {
    let mut g = ST.lock().unwrap_or_else(|e| e.into_inner()); let Some(s) = g.as_mut() else { return };
    if !s.ready || !s.enabled || !s.error.is_empty() || value > u32::MAX as u64 { return }
    let ok = match event { "latency" => OPS.contains(&detail), "error" => ERRORS.contains(&detail), "connection" => CONNS.contains(&detail), "sample" => GAUGES.contains(&detail), _ => false };
    if !ok { return }
    let key = format!("{event}:{detail}");
    let r = s.pending.entry(key).or_insert_with(|| { let base = json!({"at":now_s(),"event":event,"count":0});
        let mut o = base.as_object().cloned().unwrap();
        match event { "latency" => { o.insert("name".into(), json!(detail)); o.insert("us".into(), json!(0)); o.insert("maxMs".into(), json!(0.0)); } "error" => { o.insert("code".into(), json!(detail)); } "connection" => { o.insert("name".into(), json!(format!("connection.{detail}"))); } _ => { o.insert("name".into(), json!(detail)); o.insert("value".into(), json!(0)); } }
        Value::Object(o) });
    let c = r["count"].as_u64().unwrap_or(0);
    match event {
        "latency" => { r["count"] = json!((c + 1).min(u32::MAX as u64)); r["us"] = json!((r["us"].as_u64().unwrap_or(0) + value).min(1 << 53)); let m = r["maxMs"].as_f64().unwrap_or(0.0).max(value as f64 / 1000.0); r["maxMs"] = json!(m); }
        "sample" => { r["count"] = json!((c + 1).min(u32::MAX as u64)); r["value"] = json!(value); }
        _ => { r["count"] = json!((c + value).min(u32::MAX as u64)); }
    }
    if r["count"] == 0 { let k = format!("{event}:{detail}"); s.pending.remove(&k); }
}

/// Diagnostics.control: on | off | clear; anything else fails closed. A caller checks `ok` before reporting a durable change.
pub fn control(action: &str) -> Value {
    let mut g = ST.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = g.as_mut() else { return status_value(None, Some(false)) };
    if !s.ready { return status_value(Some(s), Some(false)) }
    if !["on", "off", "clear"].contains(&action) { let mut v = status_value(Some(s), Some(false)); v["error"] = json!("invalid_action"); return v }
    if action != "on" { s.pending.clear(); }
    if action == "off" { s.enabled = false; }
    let r = (|| -> std::io::Result<()> {
        directory(&s.dir)?;
        if action == "clear" { for i in 0..SEGMENTS { if lstat(&s.dir, &format!("segment-{i}.jsonl"))?.is_some() { std::fs::remove_file(s.dir.join(format!("segment-{i}.jsonl")))?; } } s.seq = 0; s.dropped = 0; }
        else { write_control(s, action == "on")?; s.enabled = action == "on"; }
        usage(s)?; s.error = ""; s.retry = None; Ok(())
    })();
    match r { Ok(()) => status_value(Some(s), None), Err(e) => { if action == "on" { s.enabled = false; } fail(s, &e); status_value(Some(s), Some(false)) } }
}

/// The sanitized document: flushes, re-reads and re-allowlists the persisted bytes; no output path, no upload.
pub fn export() -> String {
    let mut g = ST.lock().unwrap_or_else(|e| e.into_inner());
    let mut events = vec![]; let mut ok = false;
    if let Some(s) = g.as_mut() { if s.ready {
        ok = flush_inner(s);
        match scan(s) { Ok(files) => { let mut rows: Vec<Value> = files.into_iter().flat_map(|f| f.rows).collect(); rows.sort_by_key(|r| r["seq"].as_u64().unwrap_or(0));
            events = rows.into_iter().map(|mut r| { let o = r.as_object_mut().unwrap(); o.remove("seq"); if r["event"] == "latency" { let m = r["us"].as_f64().unwrap_or(0.0) / r["count"].as_f64().unwrap_or(1.0) / 1000.0; r["meanMs"] = json!(m); } r }).collect() }
            Err(e) => { fail(s, &e); ok = false } }
    } }
    let text = json!({"version":1,"status":status_value(g.as_ref(), Some(ok)),"events":events}).to_string();
    if text.len() > EXPORT { let mut st = status_value(g.as_ref(), Some(false)); st["error"] = json!("capacity"); return json!({"version":1,"status":st,"events":[]}).to_string() }
    text
}

pub struct DiagnosticsRpc;
impl RpcHandler for DiagnosticsRpc {
    fn methods(&self) -> &'static [&'static str] { &["diagnostics.status", "diagnostics.control"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let action = if method == "diagnostics.status" { "status".to_string() } else { p.get("action").and_then(Value::as_str).unwrap_or("").to_string() };
            let r = tokio::task::spawn_blocking(move || if action == "status" { status() } else { control(&action) }).await.unwrap_or_else(|_| json!({"ok":false,"error":"io"}));
            vec![json!({"t":"reply","id":id,"ok":true,"result":r})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fresh(tag: &str) -> PathBuf { let h = std::env::temp_dir().join(format!("bp-diag-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap(); h }
    /// Drops the recorder and starts it on `h` under one hold of its lock. Other tests' hubs call `start` on their own homes (settings::open) in
    /// parallel and outside LOCK; cleared and started apart, one of them could bind between the two and this test would record into its home.
    fn bind(h: &Path) -> Value { let mut g = ST.lock().unwrap_or_else(|e| e.into_inner()); *g = None; start_in(&mut g, h) }
    fn bound(h: &Path) -> bool { ST.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(false, |s| s.dir == h.join("diagnostics")) }
    #[test] fn a_new_home_starts_on_with_private_storage_and_a_closed_status() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); let h = fresh("start"); let s = bind(&h);
        assert_eq!((s["ok"].clone(), s["started"].clone(), s["enabled"].clone(), s["error"].clone()), (json!(true), json!(true), json!(true), json!("")));
        assert_eq!(std::fs::metadata(h.join("diagnostics")).unwrap().permissions().mode() & 0o777, 0o700); assert_eq!(std::fs::metadata(h.join("diagnostics/control")).unwrap().permissions().mode() & 0o777, 0o600);
        let keys: Vec<&str> = s.as_object().unwrap().keys().map(String::as_str).collect(); assert_eq!(keys, ["version", "ok", "started", "enabled", "error", "stored_bytes", "allocated_bytes", "cap_bytes", "retention_seconds", "pending", "dropped"]);
        assert_eq!(start(&h), status(), "a second start changes nothing");
    }
    #[test] fn closed_measurements_are_recorded_flushed_and_exported_with_a_mean() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); let h = fresh("rec"); let b = bind(&h); assert_eq!((b["ok"].clone(), b["enabled"].clone()), (json!(true), json!(true)), "started: {b}");
        note("latency", "hub.rpc", 2000); note("latency", "hub.rpc", 4000); note("error", "io", 3); note("connection", "reconnect", 1); note("sample", "hub.clients", 7);
        note("latency", "hub.rpc.user text", 1); note("error", "free text", 1); note("sample", "other", 1);
        assert!(bound(&h), "recording into this test's home"); assert_eq!(status()["pending"], 4, "only the closed names");
        let f = flush(); assert_eq!((f["ok"].clone(), f["pending"].clone()), (json!(true), json!(0)), "flushed: {f}");
        assert!(std::fs::read_to_string(h.join("diagnostics/segment-0.jsonl")).unwrap().lines().count() >= 4, "on disk before the export reads it");
        let doc: Value = serde_json::from_str(&export()).unwrap(); let ev = doc["events"].as_array().unwrap(); assert_eq!(ev.len(), 4, "{doc}");
        let lat = ev.iter().find(|e| e["event"] == "latency").unwrap(); assert_eq!((lat["count"].clone(), lat["us"].clone(), lat["maxMs"].clone(), lat["meanMs"].clone()), (json!(2), json!(6000), json!(4.0), json!(3.0)));
        assert!(ev.iter().all(|e| e.get("seq").is_none()), "internal sequence numbers are removed");
        assert!(std::fs::read_to_string(h.join("diagnostics/segment-0.jsonl")).unwrap().lines().count() >= 4);
    }
    #[test] fn off_drops_pending_and_stops_collecting_and_is_remembered_across_a_restart() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); let h = fresh("off"); bind(&h); note("error", "io", 1); assert_eq!(status()["pending"], 1);
        let r = control("off"); assert_eq!((r["ok"].clone(), r["enabled"].clone(), r["pending"].clone()), (json!(true), json!(false), json!(0)));
        note("error", "io", 1); assert_eq!(status()["pending"], 0);
        let again = bind(&h); assert_eq!(again["enabled"], false, "still off after a restart");
        assert_eq!(control("on")["enabled"], true); assert_eq!(std::fs::read_to_string(h.join("diagnostics/control")).unwrap(), "1");
    }
    #[test] fn clear_deletes_the_segments_and_keeps_the_choice() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); let h = fresh("clear"); bind(&h); note("error", "io", 1); flush(); assert!(h.join("diagnostics/segment-0.jsonl").exists());
        let r = control("clear"); assert_eq!(r["ok"], true); assert_eq!(r["enabled"], true); assert!(!h.join("diagnostics/segment-0.jsonl").exists());
    }
    #[test] fn anything_but_on_off_clear_fails_closed() { let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); let h = fresh("bad"); bind(&h); let r = control("../etc"); assert_eq!((r["ok"].clone(), r["error"].clone()), (json!(false), json!("invalid_action"))); }
    #[test] fn a_stray_file_a_symlink_or_a_hardlink_fails_closed_and_is_never_followed() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let h = fresh("stray"); std::fs::create_dir_all(h.join("diagnostics")).unwrap(); std::fs::write(h.join("diagnostics/notes.txt"), "x").unwrap();
        let s = bind(&h); assert_eq!((s["ok"].clone(), s["error"].clone()), (json!(false), json!("unsafe_storage"))); assert!(h.join("diagnostics/notes.txt").exists(), "not deleted");
        let h = fresh("link"); std::fs::create_dir_all(h.join("diagnostics")).unwrap(); std::fs::write(h.join("outside"), "x").unwrap(); std::os::unix::fs::symlink(h.join("outside"), h.join("diagnostics/control")).unwrap();
        assert_eq!(bind(&h)["error"], "unsafe_storage");
    }
    #[test] fn a_torn_tail_is_cut_back_and_an_invalid_row_discards_the_segment() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); let h = fresh("torn"); bind(&h); note("error", "io", 2); flush();
        let seg = h.join("diagnostics/segment-0.jsonl"); let good = std::fs::read_to_string(&seg).unwrap(); std::fs::write(&seg, format!("{good}{{\"v\":1,\"seq\":")).unwrap();
        bind(&h); assert_eq!(std::fs::read_to_string(&seg).unwrap(), good, "cut to the last newline");
        std::fs::write(&seg, format!("{good}{{\"v\":1,\"event\":\"nope\"}}\n")).unwrap(); bind(&h); assert!(!seg.exists(), "an invalid complete row discards the segment");
    }
    #[test] fn an_expired_row_removes_its_whole_segment() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); let h = fresh("old"); bind(&h);
        let old = now_s() - RETAIN - 10; std::fs::write(h.join("diagnostics/segment-3.jsonl"), format!("{{\"v\":1,\"seq\":1,\"at\":{old},\"event\":\"error\",\"code\":\"io\",\"count\":1}}\n")).unwrap();
        bind(&h); assert!(!h.join("diagnostics/segment-3.jsonl").exists());
    }
    #[test] fn storage_never_exceeds_the_cap_whatever_is_recorded() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); let h = fresh("cap"); bind(&h);
        for round in 0..40 { for op in OPS { note("latency", op, 100 + round); } for g in GAUGES { note("sample", g, round); } for e in ERRORS { note("error", e, 1); } flush(); }
        let st = status(); assert!(st["stored_bytes"].as_u64().unwrap() <= DISK && st["allocated_bytes"].as_u64().unwrap() <= DISK + 8 * 4096, "{st}");
        assert!(std::fs::read_dir(h.join("diagnostics")).unwrap().count() <= 9);
    }
}

//! The alpha aggregate counters (docs/telemetry.md, "Alpha aggregate contract"): twelve fixed integers per UTC day, counted as events
//! commit while sending is allowed (settings/telemetry_send.rs `ALLOWED`), never by reading conversations back. Kept in memory and saved to
//! `<home>/telemetry/counts.json` (private, atomic) on each worker pass, at most seven days, scoped to the id's reset marker: an Off or a
//! Reset id erases them and a new consent starts from zero. A crash between passes loses at most that minute's counts (an undercount).
//! Only the numbers leave: the event's fixed field names, never a name, id, path or text from what was counted.
use super::telemetry_send::{day_of, View};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, HashSet, VecDeque}, path::Path, sync::Mutex};

/// The wire's counter names, in order. The receiver requires the first ten; the two board counters are optional there, sent always here.
pub const NAMES: [&str; 12] = ["threads_created", "user_messages", "assistant_messages", "bot_messages", "bot_wakes", "input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens", "usage_turns", "boards_created", "boards_ordered"];
pub const MAX: u64 = 281_474_976_710_655;
pub const BOARDS_CREATED: usize = 10;
pub const BOARDS_ORDERED: usize = 11;

#[derive(Default)]
struct Counts { reset: String, days: BTreeMap<String, [u64; 12]>, mail: VecDeque<String>, mail_seen: HashSet<String>, loaded: bool }
fn counts() -> &'static Mutex<Counts> { static C: std::sync::OnceLock<Mutex<Counts>> = std::sync::OnceLock::new(); C.get_or_init(Default::default) }

/// Usage.of one committed event: which counters it adds to, and by how much. Mirrored threads (`~` ids) count nothing.
pub fn of(e: &Value) -> Vec<(usize, u64)> {
    let s = |k: &str| e[k].as_str().unwrap_or(""); let n = |k: &str| e[k].as_u64().unwrap_or(0);
    let mirrored = |k: &str| s(k).contains('~');
    match s("$") {
        "ThreadCreated" if !mirrored("id") => vec![(0, 1)],
        "MessagePosted" if !mirrored("thread") => match s("role") { "user" => vec![(1, 1)], "assistant" => vec![(2, 1)], _ => vec![] },
        "MailPosted" => vec![(3, 1)],
        "BotWoke" => vec![(4, 1)],
        "TurnUsage" if !mirrored("thread") => {
            let k = s("known"); let mut v = vec![];
            for (c, f, i) in [('i', "input", 5), ('r', "cread", 6), ('w', "cwrite", 7), ('o', "output", 8)] { if k.contains(c) && n(f) > 0 { v.push((i, n(f))) } }
            if k.contains('o') { v.push((9, 1)) }
            v
        }
        _ => vec![],
    }
}
fn add(day: &mut [u64; 12], i: usize, n: u64) { day[i] = day[i].saturating_add(n).min(MAX) }
/// Seven UTC days, today included: older snapshots are past what the receiver takes.
fn keep_week(c: &mut Counts, now: u64) { let (from, to) = (day_of(now.saturating_sub(6 * 86400)), day_of(now)); c.days.retain(|d, _| d.as_str() >= from.as_str() && d.as_str() <= to.as_str()); }

/// Count a committed batch (the caller checked that sending is allowed). A mail id is counted once.
pub fn count(events: &[Value], now: u64) {
    let day = day_of(now); let mut c = counts().lock().unwrap();
    for e in events {
        if e["$"] == "MailPosted" { let id = e["id"].as_str().unwrap_or("").to_string(); if !id.is_empty() && !c.mail_seen.insert(id.clone()) { continue } c.mail.push_back(id); if c.mail.len() > 4096 { if let Some(o) = c.mail.pop_front() { c.mail_seen.remove(&o); } } }
        for (i, n) in of(e) { add(c.days.entry(day.clone()).or_insert([0; 12]), i, n) }
    }
    keep_week(&mut c, now);
}
/// A counter that is not an event in the log: a board family created (agents/design.rs) or a board order paid (settings/order.rs).
pub fn bump(i: usize, n: u64) {
    if n == 0 || !super::telemetry_send::ALLOWED.load(std::sync::atomic::Ordering::Relaxed) { return }
    let now = crate::hub::now_ms() / 1000; let day = day_of(now); let mut c = counts().lock().unwrap();
    add(c.days.entry(day).or_insert([0; 12]), i, n); keep_week(&mut c, now);
}
/// Everything forgotten (Off, Reset id, the disclosure not seen).
pub fn clear() { let mut c = counts().lock().unwrap(); *c = Counts { loaded: true, ..Default::default() }; }
/// The worker's pass: load the saved counts once, start again when the reset marker moved, and save.
pub fn scope(home: &Path, reset: &str) {
    let mut c = counts().lock().unwrap();
    if !c.loaded {
        c.loaded = true;
        let v: Value = std::fs::read_to_string(super::telemetry_send::dir(home).join("counts.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        if v["reset"].as_str() == Some(reset) {
            for (d, a) in v["days"].as_object().into_iter().flatten() {
                let mut row = [0u64; 12]; for (i, x) in a.as_array().into_iter().flatten().take(12).enumerate() { row[i] = x.as_u64().unwrap_or(0).min(MAX) }
                let cur = c.days.entry(d.clone()).or_insert([0; 12]); for i in 0..12 { cur[i] = cur[i].saturating_add(row[i]).min(MAX) }
            }
        }
        c.reset = reset.to_string();
    }
    if c.reset != reset { c.days.clear(); c.reset = reset.to_string(); }
    keep_week(&mut c, crate::hub::now_ms() / 1000);
    let v = json!({"reset": c.reset, "days": c.days.iter().map(|(d, a)| (d.clone(), json!(a.to_vec()))).collect::<serde_json::Map<String, Value>>()});
    drop(c);
    let _ = super::telemetry_send::write_private(home, "counts.json", &v.to_string());
}
/// Usage.event: one day's snapshot on the wire.
pub fn event(day: &str, row: &[u64; 12], id: &str, v: &View) -> Value {
    let counts: serde_json::Map<String, Value> = NAMES.iter().zip(row.iter()).map(|(k, n)| (k.to_string(), json!(n))).collect();
    json!({"e": "usage", "day": day, "share": true, "id": id, "ver": v.ver, "ch": v.ch, "os": v.os, "arch": v.arch, "counts": counts})
}
/// The snapshots of the last seven days that counted anything, newest last.
pub fn events(id: &str, v: &View) -> Vec<Value> {
    let c = counts().lock().unwrap();
    c.days.iter().filter(|(d, a)| a.iter().any(|n| *n > 0) && d.as_str() <= v.day.as_str()).map(|(d, a)| event(d, a, id, v)).collect()
}
/// Tests: the counts held now.
#[cfg(test)] pub fn held() -> BTreeMap<String, [u64; 12]> { counts().lock().unwrap().days.clone() }

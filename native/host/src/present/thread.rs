//! `shown.thread` entries, as the helper draws them (src/mobile/screen.bend `Mob.entries`, src/core/fold.bend, src/core/stamp.bend), from the
//! 32 page rows the hub reads out of its index. Pure: rows in, JSON out. `None` is "not ported": anything this first cut does not reproduce
//! exactly (images, chat rows, reminders, thoughts, design steps, terminals, reviews, API errors, restart rows, attachments, other time zones,
//! stop/fail marks, rows sent to another thread) makes the caller fall to the helper, so a `Some` is always the helper's answer.
use crate::{md, stamp};
use serde_json::{json, Value};
use std::collections::HashMap;

/// the page rows are [seq ord id thread src kind role akind text at], newest first
#[derive(Clone, Debug)]
pub struct Ent { pub id: String, pub msg: bool, pub role: String, pub akind: String, pub text: String, pub at: u64 }

/// the rows the page holds (the helper draws the newest 32), oldest first; None for a row this port does not draw
pub fn ents(rows: &[Value], thread: &str) -> Option<Vec<Ent>> {
    let mut v: Vec<Ent> = Vec::new();
    for r in rows.iter().take(32) {
        let c: Vec<&str> = (0..10).map(|i| r.get(i).and_then(Value::as_str).unwrap_or("")).collect();
        if c[3] != thread || !c[4].is_empty() { return None } // a row sent to another thread
        let msg = match c[5] { "msg" => true, "act" => false, _ => return None };
        v.push(Ent { id: c[2].into(), msg, role: c[6].into(), akind: c[7].into(), text: c[8].into(), at: c[9].parse().ok()? });
    }
    v.reverse();
    Some(v)
}

fn tool(e: &Ent) -> bool { !e.msg && (e.role == "tool" || e.role == "step") }
fn todo(k: &str) -> bool { matches!(k, "TodoWrite" | "TaskCreate" | "TaskUpdate") }
/// Tool.short: "mcp__backplane__viewer_show" -> "viewer_show"
fn short(k: &str) -> String {
    match k.strip_prefix("mcp__") { Some(r) => match r.find("__") { Some(i) if !r[i + 2..].is_empty() => r[i + 2..].to_string(), _ => k.to_string() }, None => k.to_string() }
}
fn shown_name(k: &str) -> String { if todo(k) { "To-do".into() } else { short(k) } }
fn calls(n: usize) -> String { format!("{} tool {}", n, if n == 1 { "call" } else { "calls" }) }

/// Fold.label / Fold.label.live (no design steps: those make this port return None)
fn label(run: &[Ent], live: bool) -> String {
    let n = run.len();
    if live {
        let last = &run[n - 1];
        let first = last.text.split('\n').next().unwrap_or("").chars().take(160).collect::<String>();
        let what = if todo(&last.akind) { shown_name(&last.akind) } else { let t = first.trim(); if t.is_empty() { shown_name(&last.akind) } else { t.to_string() } };
        return format!("{} · running: {}", calls(n), what);
    }
    let mut ks: Vec<String> = Vec::new();
    for e in run { if todo(&e.akind) { continue } let s = short(&e.akind); if !ks.contains(&s) { ks.push(s) } }
    let mut out = calls(n);
    if !ks.is_empty() { out += " · "; out += &ks.iter().take(3).cloned().collect::<Vec<_>>().join(", "); if ks.len() > 3 { out += ", …" } }
    out
}

// ---- time (UTC only: a person's zone makes the caller fall to the helper) ----
const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
/// days since 1970-01-01 -> (year, month 1..12, day)
fn civil(z: i64) -> (i64, usize, i64) {
    let z = z + 719468; let era = z.div_euclid(146097); let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as usize, d)
}
fn hm(w: u64) -> String { format!("{:02}:{:02}", (w % 86400) / 3600, (w % 3600) / 60) }
/// Stamp.clock: "14:05" on the day it is now, "Tue 14 Nov 14:05" on another
fn clock(at: u64, now: u64) -> String {
    if at / 86400 == now / 86400 { return hm(at) }
    let days = (at / 86400) as i64; let (_, m, d) = civil(days);
    format!("{} {} {} {}", DAYS[((days + 3).rem_euclid(7)) as usize], d, MONS[m - 1], hm(at))
}
fn took(s: u64) -> String {
    if s < 60 { format!("{s}s") } else if s < 3600 { format!("{}m {}s", s / 60, s % 60) } else { format!("{}h {}m", s / 3600, (s % 3600) / 60) }
}

fn md_blocks(text: &str) -> Vec<Value> { md::nodes_json(&md::render(text)) }

/// An ApiErr / restart / attachment / image / thought / design-step / emb / review / reminder row is not drawn here.
fn plain(e: &Ent) -> bool {
    if e.msg {
        let t = e.text.as_str();
        if e.id.starts_with("@live:") || t.starts_with("API Error") || t.starts_with("Backplane") || t.starts_with("[Backplane]") { return false }
        if e.role == "user" { return !t.contains("\n[attachment") && !t.contains("\u{1}") && !t.contains("attached:") }
        if e.role == "assistant" {
            let l = t.to_ascii_lowercase();
            return !t.trim().is_empty() && ![".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", "![" ].iter().any(|x| l.contains(x));
        }
        false
    } else {
        matches!(e.role.as_str(), "tool") && e.akind != "restart"
    }
}

/// `entries` of shown.thread for a plain page; `running`: the thread's turn runs; `open`: runs the person opened (the "fold:<id>" choices = "1");
/// `now_ms`: the request's clock. None when any row needs what this cut does not do.
pub fn entries(es: &[Ent], running: bool, open: &dyn Fn(&str) -> bool, now_ms: u64) -> Option<Vec<Value>> {
    if !es.iter().all(|e| tool(e) && e.role == "tool" && plain(e) || !tool(e) && plain(e)) { return None }
    let now = now_ms / 1000;
    // Stamp.ends over the page, oldest first in, walked newest first
    let sx: Vec<stamp::E> = es.iter().map(|e| if e.msg && e.role == "user" { stamp::E::User { id: e.id.clone(), at: e.at } } else if e.msg { stamp::E::Assistant { id: e.id.clone(), at: e.at } } else { stamp::E::Other }).collect();
    let ends: HashMap<String, u64> = stamp::ends(&sx, running);
    let mut out = Vec::new(); let mut i = 0;
    while i < es.len() {
        let e = &es[i];
        if tool(e) {
            let mut j = i; while j < es.len() && tool(&es[j]) { j += 1 }
            let run = &es[i..j]; let live = j == es.len() && running; let id = &run[0].id;
            let o = open(id);
            out.push(json!({"id": format!("fold:{id}"), "kind": "fold", "text": label(run, live), "open": o, "live": live, "value": format!("{}{}", if o { "0" } else { "1" }, id)}));
            if o { for r in run { out.push(act(r, id)) } }
            i = j; continue;
        }
        let c = clock(e.at, now);
        if e.role == "user" {
            out.push(json!({"id": e.id, "kind": "user", "text": e.text, "attachments": [], "stamp": c}));
        } else {
            let st = ends.get(&e.id).copied().unwrap_or(0);
            let s = if st == 0 { c } else if st > 1 { format!("Finished {c} · {}", took(e.at.saturating_sub(st))) } else { format!("Finished {c}") };
            out.push(json!({"id": e.id, "kind": "assistant", "text": e.text, "blocks": md_blocks(&e.text), "images": [], "stamp": s}));
        }
        i += 1;
    }
    Some(out)
}

/// Mob.act.k for a plain tool call (open run)
fn act(e: &Ent, run: &str) -> Value {
    let text = if e.text == e.akind { String::new() } else { e.text.clone() };
    json!({"id": e.id, "kind": "act", "tone": e.role, "label": shown_name(&e.akind), "text": text, "run": run})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value { serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-pair.json")).unwrap()).unwrap() }
    #[test] fn the_entries_of_a_plain_page_are_the_helpers() {
        let p = fixture(); let th = p["params"]["page"]["thread"].as_str().unwrap();
        let es = ents(p["params"]["page"]["rows"].as_array().unwrap(), th).unwrap();
        let got = entries(&es, false, &|_| false, p["params"]["now"].as_u64().unwrap()).unwrap();
        let want = p["out"]["shown"]["thread"]["entries"].as_array().unwrap();
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) { assert_eq!(g, w); }
    }
    #[test] fn civil_dates() { assert_eq!(civil(0), (1970, 1, 1)); assert_eq!(civil(19675), (2023, 11, 14)); assert_eq!(clock(1700002806, 0), "Tue 14 Nov 23:00"); }
    #[test] fn labels_name_three_kinds_and_the_live_one_says_what_runs() {
        let t = |k: &str, x: &str| Ent { id: "a".into(), msg: false, role: "tool".into(), akind: k.into(), text: x.into(), at: 1 };
        let run = [t("Bash", "ls"), t("Read", "f"), t("Edit", "g"), t("Grep", "h"), t("Bash", "i")];
        assert_eq!(label(&run, false), "5 tool calls · Bash, Read, Edit, …");
        assert_eq!(label(&run, true), "5 tool calls · running: i");
        assert_eq!(label(&[t("mcp__backplane__viewer_show", "")], false), "1 tool call · viewer_show");
    }
}

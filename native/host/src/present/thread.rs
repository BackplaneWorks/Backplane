//! `shown.thread` entries, as the helper draws them, from the
//! 32 page rows the hub reads out of its index. Pure: rows in, JSON out. `None` is "not ported": anything this first cut does not reproduce
//! exactly (images, chat rows, reminders, thoughts, design steps, terminals, reviews, API errors, restart rows, attachments, other time zones,
//! stop/fail marks, rows sent to another thread) makes the caller fall to the helper, so a `Some` is always the helper's answer.
use crate::{md, stamp};
use super::tz::Rule;
use serde_json::{json, Value};
use std::collections::HashMap;

/// the page rows are [seq ord id thread src kind role akind text at], newest first
#[derive(Clone, Debug)]
pub struct Ent { pub id: String, pub msg: bool, pub role: String, pub akind: String, pub text: String, pub at: u64 }

/// the rows the page holds (the helper draws the newest 32), oldest first; None for a row this port does not draw
pub fn ents(rows: &[Value], thread: &str) -> Option<Vec<Ent>> {
    let mut v: Vec<Ent> = Vec::new();
    for r in rows.iter().take(32) {
        let c: Vec<String> = (0..10).map(|i| match r.get(i) { Some(Value::String(s)) => s.clone(), Some(Value::Number(n)) => n.to_string(), _ => String::new() }).collect();
        if c[3] != thread || !c[4].is_empty() { continue } // a row this thread sent into another thread: not shown here (a gap in a key, not a refused screen)
        let msg = match c[5].as_str() { "msg" => true, "act" => false, _ => return None };
        v.push(Ent { id: c[2].clone(), msg, role: c[6].clone(), akind: c[7].clone(), text: if msg { crate::settings::order::pay_shown(&c[8]) } else { c[8].clone() }, at: c[9].parse().ok()? });
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
pub(crate) fn shown_name(k: &str) -> String { if todo(k) { "To-do".into() } else { short(k) } }
fn calls(n: usize) -> String { format!("{} tool {}", n, if n == 1 { "call" } else { "calls" }) }

fn steps_label(d: usize) -> String { format!("{} design {}", d, if d == 1 { "change" } else { "changes" }) }
fn is_step(e: &Ent) -> bool { !e.msg && e.role == "step" }
/// Fold.label / Fold.label.live: the tool calls of a run (its design steps are counted apart)
pub(crate) fn label(run: &[Ent], live: bool) -> String {
    let calls_: Vec<&Ent> = run.iter().filter(|e| !is_step(e)).collect();
    let (n, d) = (calls_.len(), run.len() - calls_.len());
    if n == 0 { return steps_label(d) }
    if live {
        let last = calls_[n - 1];
        let first = last.text.split('\n').next().unwrap_or("").chars().take(160).collect::<String>();
        let what = if todo(&last.akind) { shown_name(&last.akind) } else { let t = first.trim(); if t.is_empty() { shown_name(&last.akind) } else { t.to_string() } };
        return format!("{}{} · running: {}", calls(n), if d == 0 { String::new() } else { format!(" · {}", steps_label(d)) }, what);
    }
    let mut ks: Vec<String> = Vec::new();
    for e in &calls_ { if todo(&e.akind) { continue } let s = short(&e.akind); if !ks.contains(&s) { ks.push(s) } }
    let mut out = calls(n);
    if !ks.is_empty() { out += " · "; out += &ks.iter().take(3).cloned().collect::<Vec<_>>().join(", "); if ks.len() > 3 { out += ", …" } }
    if d > 0 { out += " · "; out += &steps_label(d) }
    out
}

/// Step.base: the design as it was before the edits (a thread's first step taken at a turn's start, or an older record that says only that it is first) is no change
pub(crate) fn step_base(e: &Ent) -> bool {
    is_step(e) && e.text.contains("\"p\":\"\"") && (e.text.contains("\"w\":\"start\"") || !e.text.contains("\"w\":"))
}
/// Hist.row: "Design change · board.kicad_pcb (new), power.kicad_sch (removed)"; a thread's first step reads "Design before the edits · "
pub(crate) fn step_row(e: &Ent) -> String {
    let Ok(v) = serde_json::from_str::<Value>(&e.text) else { return "Design change".into() };
    let files: Vec<String> = v["f"].as_array().map(|a| a.iter().filter_map(|l| l.as_str()).filter_map(|l| {
        let mut p = l.splitn(2, '\t'); let (st, rel) = (p.next()?, p.next()?); if st.is_empty() || rel.is_empty() { return None }
        Some(format!("{}{}", rel.rsplit('/').next().unwrap_or(rel), match st { "D" => " (removed)", "A" => " (new)", _ => "" })) }).collect()).unwrap_or_default();
    format!("{}{}", if v["p"].as_str().unwrap_or("").is_empty() { "Design before the edits · " } else { "Design change · " }, files.join(", "))
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
/// Stamp.clock: "14:05" on the day it is now, "Tue 14 Nov 14:05" on another, both on the wall clock of the person's zone
pub(crate) fn clock(at: u64, now: u64, zone: &Rule) -> String {
    let (w, wn) = (zone.wall(at), zone.wall(now));
    if w / 86400 == wn / 86400 { return hm(w) }
    let days = (w / 86400) as i64; let (_, m, d) = civil(days);
    format!("{} {} {} {}", DAYS[((days + 3).rem_euclid(7)) as usize], d, MONS[m - 1], hm(w))
}
fn took(s: u64) -> String {
    if s < 60 { format!("{s}s") } else if s < 3600 { format!("{}m {}s", s / 60, s % 60) } else { format!("{}h {}m", s / 3600, (s % 3600) / 60) }
}

fn md_blocks(text: &str) -> Vec<Value> { md::nodes_json(&md::render(text)) }

/// Every row draws; where its own key is not ready it degrades (images are not found: `images` is empty; attachment lines stay in the text; a reminder, a terminal
/// and a review comment show as plain activity rows; a thought shows only when the setting is on, and that setting refuses the thread). Kept for the refusal API.
fn plain(_e: &Ent) -> bool { true }

const CARRY: &str = "Backplane restarted while you were working. Continue where you left off.";
fn api_err(x: &str) -> bool { x.starts_with("API Error") }
fn white(s: &str) -> bool { s.chars().all(|c| matches!(c, ' ' | '\n' | '\r' | '\t')) }
/// A message only the agent reads: the hub's note to a buy thread's agent about the quote it posted (or could not), which the person sees as the
/// quote itself, and the note that a secret was saved (`/secret`, or an agent's request): the person saved it in a box, they posted nothing.
/// The window leaves it out; data/1 marks it `agentOnly` so the phones leave it out too, without reading its words.
/// What the agent is told once a secret is saved (settings/secret.rs), after the secret's handle `{"secret":…,"set":true}`.
pub const SAVED_NOTE: &str = " The person saved this secret in Backplane's vault. Use it by name with secret_run; you will never see its value.";
pub fn agent_only(role: &str, text: &str) -> bool {
    (role == "user" && text.starts_with("{\"secret\":") && text.ends_with(SAVED_NOTE)) || role == "system" && ["The quote is posted in this thread. ", "The quote could not be worked out: ", "The files were not taken: ", "The person chose not to send the files (Not now)."].iter().any(|h| text.starts_with(&format!("[Backplane] {h}")))
}
fn blank(e: &Ent) -> bool { e.msg && ((e.role == "assistant" && white(&e.text)) || agent_only(&e.role, &e.text)) }
/// Fold.shown: blank messages left out, an error row an API error reply already says, and the restart row its continue message stands for
fn fold_shown(es: &[Ent]) -> Vec<Ent> {
    let (mut out, mut p) = (Vec::new(), String::new());
    for e in es {
        let b = blank(e);
        let dup = !e.msg && ((e.akind == "restart" && e.text == "Continuing after the restart" && p.starts_with(CARRY))
            || (api_err(&p) && e.role == "error" && e.akind == "result" && (e.text.trim().is_empty() || e.text.trim() == p.trim())));
        if !(b || dup) { out.push(e.clone()) }
        p = if b { p } else if e.msg && e.role == "assistant" { if api_err(&e.text) { e.text.clone() } else { String::new() } } else if e.msg && e.role == "user" { if e.text.starts_with(CARRY) { e.text.clone() } else { String::new() } } else { String::new() };
    }
    out
}
/// FO.ApiErr.say: a provider's API error as one sentence (English)
fn api_say(x: &str) -> String {
    let r = x[9..].trim(); let r = r.strip_prefix(':').map(str::trim).unwrap_or(r);
    let code: String = if r.chars().take(3).count() == 3 && r.chars().take(3).all(|c| c.is_ascii_digit()) { r.chars().take(3).collect() } else { String::new() };
    let body = r[code.len()..].trim();
    let o: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let ty = o["error"]["type"].as_str().unwrap_or("");
    let kind = if ty == "overloaded_error" || code == "529" { "overloaded" } else if ty == "rate_limit_error" || code == "429" { "rate" } else if ty == "api_error" || code.starts_with('5') { "server" } else if ty == "authentication_error" || code == "401" { "auth" } else { "other" };
    let msg = o["error"]["message"].as_str().or_else(|| o["message"].as_str()).map(str::to_string).unwrap_or_else(|| if body.starts_with('{') { String::new() } else { body.to_string() });
    let msg = msg.trim();
    let why = if kind == "other" && !msg.is_empty() { format!(": {}", msg.strip_suffix('.').unwrap_or(msg)) } else { String::new() };
    let head = match kind { "overloaded" => "Claude's API was overloaded", "rate" => "Claude's API hit its rate limit", "server" => "Claude's API had an internal error", "auth" => "Claude's API refused the sign-in", _ => "Claude's API answered with an error" };
    format!("{head}{} and ended the turn{why}.", if code.is_empty() { String::new() } else { format!(" ({code})") })
}


// ---- attachments and images ----
/// Att.split: the lines at the end of a message that are `[attachment: <path>]` (blank lines between them allowed) are its attachments; the rest is the body
fn att_split(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.split('\n').collect();
    let (mut paths, mut end) = (Vec::new(), lines.len());
    while end > 0 {
        let l = lines[end - 1].trim();
        if l.starts_with("[attachment: ") && l.ends_with(']') { paths.insert(0, l[13..l.len() - 1].trim().to_string()); end -= 1 }
        else if l.is_empty() { end -= 1 }
        else { break }
    }
    if paths.is_empty() { return (text.to_string(), paths) }
    (lines[..end].join("\n"), paths)
}
fn enc(s: &str) -> String { s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'-' | b'_') { (b as char).to_string() } else { format!("%{b:02X}") }).collect() }
/// Mob.chip: an attachment as a chip (its name without the upload key, whether it is an image, where the hub serves it)
fn chip(path: &str) -> Value {
    let base = path.rsplit('/').next().unwrap_or(path);
    let parts: Vec<&str> = base.split('-').collect();
    let label = if parts.len() >= 2 { parts[1..].join("-") } else { base.to_string() };
    let ext = base.rsplit('.').next().unwrap_or("").to_lowercase();
    json!({"label": label, "path": path, "image": matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp"), "url": format!("/img?path={}", enc(path))})
}
/// Img.paths: the image files a message names, each once, in order
fn img_paths(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.split(|c: char| matches!(c, ' ' | '\n' | '\t' | '\r' | '(' | ')' | '[' | ']' | '<' | '>' | '`' | '"' | '\'' | ',' | ';' | '|')) {
        if w.is_empty() || w.chars().count() > 1024 { continue }
        let b = w.strip_prefix("file://").unwrap_or(w);
        let b = if b.ends_with(['.', ':', '!', '?']) { &b[..b.len() - 1] } else { b };
        let l = b.to_lowercase();
        if (crate::plat::fs::is_abs(b) || b.starts_with("~/")) && [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"].iter().any(|e| l.ends_with(e)) && !out.iter().any(|o| o == b) { out.push(b.to_string()) }
    }
    out
}
fn images(entry: &str, text: &str) -> Vec<Value> {
    img_paths(text).into_iter().map(|p| json!({"path": p, "url": format!("/message-image?entry={}&path={}", enc(entry), enc(&p))})).collect()
}

/// `entries` of shown.thread for a plain page; `running`: the thread's turn runs; `open`: runs the person opened (the "fold:<id>" choices = "1");
/// `now_ms`: the request's clock. None when any row needs what this cut does not do.
pub fn entries(es: &[Ent], running: bool, open: &dyn Fn(&str) -> bool, now_ms: u64, zone: Option<&Rule>, marks: &dyn Fn(&str) -> u8, end: u8) -> Option<Vec<Value>> {
    if !es.iter().all(plain) { return None }
    // Hist.seen: the design as it was before is no row
    let seen: Vec<Ent> = es.iter().filter(|e| !step_base(e)).cloned().collect();
    let es = seen.as_slice();
    let vis: Vec<Ent> = fold_shown(es);
    let now = now_ms / 1000;
    // Stamp.ends over the page, oldest first in, walked newest first
    let sx: Vec<stamp::E> = vis.iter().map(|e| if e.msg && e.role == "user" { stamp::E::User { id: e.id.clone(), at: e.at } } else if e.msg { stamp::E::Assistant { id: e.id.clone(), at: e.at } } else { stamp::E::Other }).collect();
    let ends: HashMap<String, u64> = stamp::ends(&sx, running);
    let stops = stops(&vis, running, end, marks);
    // Mob.imgs.seen: the images of the newest 30 replies are found
    let newest_replies: std::collections::HashSet<&str> = es.iter().rev().filter(|e| e.msg && e.role == "assistant").take(30).map(|e| e.id.as_str()).collect();
    let mut out = Vec::new(); let mut i = 0;
    while i < vis.len() {
        let e = &vis[i];
        if tool(e) {
            let mut j = i; while j < vis.len() && tool(&vis[j]) { j += 1 }
            let run = &vis[i..j]; let live = j == vis.len() && running; let id = &run[0].id;
            let o = open(id);
            out.push(json!({"id": format!("fold:{id}"), "kind": "fold", "text": label(run, live), "open": o, "live": live, "value": format!("{}{}", if o { "0" } else { "1" }, id)}));
            if o { for r in run { out.push(act(r, id)) } }
            i = j; continue;
        }
        if !e.msg { out.push(plain_act(e)); i += 1; continue }
        // a zone that cannot be known degrades the time stamps (empty), not the screen
        let c = zone.map(|z| clock(e.at, now, z));
        if e.role == "user" && e.text.starts_with(CARRY) {
            out.push(json!({"id": e.id, "kind": "act", "tone": "info restart", "label": "restart", "text": "Continuing after the restart"}));
        } else if e.role == "user" {
            // Rto.of: a reply's quote block is the agent's; every client shows the body and an "In reply to" line naming the message
            let (reply, shown) = match crate::session::composer::reply_split(&e.text) { Some((id, q, b)) => (Some((id, q)), b), None => (None, e.text.clone()) };
            let (body, paths) = att_split(&shown);
            let mut row = json!({"id": e.id, "kind": "user", "text": body, "attachments": paths.iter().map(|p| chip(p)).collect::<Vec<_>>(), "stamp": c.unwrap_or_default()});
            if let Some((rid, q)) = reply { row["replyTo"] = json!({"id": rid, "quote": q.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("")}); }
            out.push(row);
        } else {
            let st = ends.get(&e.id).copied().unwrap_or(0);
            // Stamp.reply.s / .f / reply: how the turn ended says the word; a stopped one says it whether or not the page holds its start
            let s = match c { None => String::new(), Some(c) => {
                let took_s = |w: &str| if st > 1 { format!("{w} {c} · {}", took(e.at.saturating_sub(st))) } else { format!("{w} {c}") };
                match stops.get(&e.id).copied().unwrap_or(0) { 1 => took_s("Stopped"), 3 => if st == 0 { c } else { took_s("Failed") }, _ => if st == 0 { c } else { took_s("Finished") } } } };
            // a synthetic "@live:" row is the agent still working; an API error reply reads as one sentence, its raw text kept for Copy
            let (text, blocks, s) = if e.id.starts_with("@live:") { ("Working".to_string(), md_blocks("Working"), String::new()) } else if api_err(&e.text) { (e.text.clone(), md_blocks(&api_say(&e.text)), s) } else { (e.text.clone(), md_blocks(&e.text), s) };
            let imgs = if newest_replies.contains(e.id.as_str()) { images(&e.id, &e.text) } else { vec![] };
            out.push(json!({"id": e.id, "kind": "assistant", "text": text, "blocks": blocks, "images": imgs, "stamp": s}));
        }
        i += 1;
    }
    Some(out)
}

/// Stamp.stops + Stamp.ended.last: the turns' last replies that were stopped (1) or failed (3): by the marks the hub set on the person's message, and
/// the thread's current turn when it ended that way (`end`: 1 stopped, 3 failed; 0 while it runs)
fn stops(es: &[Ent], running: bool, end: u8, marks: &dyn Fn(&str) -> u8) -> HashMap<String, u8> {
    let (mut open, mut held, mut m) = (!running, String::new(), HashMap::new());
    for e in es.iter().rev() {
        if !e.msg { continue }
        if e.role == "user" { let k = marks(&e.id); if k > 0 && !held.is_empty() { m.insert(held.clone(), k); } open = true; held.clear(); }
        else if e.role == "assistant" { let a = e.id.ends_with(":before"); if !a && open { held = e.id.clone(); } open = open && a; }
    }
    // Stamp.last.go: the newest message, when it is a reply
    let last = es.iter().rev().find(|e| e.msg).filter(|e| e.role == "assistant").map(|e| e.id.clone());
    let kind = if running { 0 } else { end };
    if let (true, Some(id)) = (kind > 0, last) { m.insert(id, kind); }
    m
}

/// A chat terminal's key and its entry's summary "title\nCOLSxROWS\ncommand", as term/emb.rs has them (this file is also built without the
/// term lane, by screen-audit; term/tests.rs holds the two to the same answers).
pub fn term_key(thread: &str, id: &str) -> String { format!("x:{thread}:{id}") }
pub fn term_summary(s: &str) -> (String, u32, u32, String) {
    let num = |s: &str| s.bytes().filter(u8::is_ascii_digit).fold(0u32, |a, b| (a * 10 + (b - b'0') as u32).min(100000));
    let clamp = |n: u32, lo: u32, hi: u32, d: u32| if n == 0 { d } else { n.clamp(lo, hi) };
    let mut it = s.split('\n'); let title = it.next().unwrap_or("").to_string(); let mut xy = it.next().unwrap_or("").split('x');
    let (c, r) = (clamp(num(xy.next().unwrap_or("")), 20, 200, 80), clamp(num(xy.next().unwrap_or("")), 3, 60, 24));
    (title, c, r, it.collect::<Vec<_>>().join("\n"))
}
/// Mob.act.k for an activity that does not fold: its tone, its name and what it says
pub(crate) fn plain_act(e: &Ent) -> Value {
    // a chat terminal (term/emb.rs): a card in the timeline, its screen streamed by `term` frames under `key` (set by thread_object_checked)
    if e.role == "term" && e.akind == "terminal" { let (title, cols, rows, cmd) = term_summary(&e.text);
        return json!({"id": e.id, "kind": "term", "text": title, "title": title, "cols": cols, "rows": rows, "cmd": cmd}) }
    let text = if e.akind == "result" && e.text.trim().is_empty() { "The turn ended with an error; the agent gave no reason.".to_string() } else if e.text == e.akind { String::new() } else { e.text.clone() };
    let tone = if e.akind == "restart" { "info restart".to_string() } else { e.role.clone() };
    json!({"id": e.id, "kind": "act", "tone": tone, "label": shown_name(&e.akind), "text": text})
}

/// Mob.act for a row in an open run: a design step shows as a step (its value opens it in the viewer), a tool call as an activity
fn act(e: &Ent, run: &str) -> Value {
    if is_step(e) { return json!({"id": e.id, "kind": "step", "text": step_row(e), "value": e.id, "run": run}) }
    let text = if e.text == e.akind { String::new() } else { e.text.clone() };
    json!({"id": e.id, "kind": "act", "tone": e.role, "label": shown_name(&e.akind), "text": text, "run": run})
}


// ---- the whole shown.thread of a plain, idle thread ----
use crate::state::{State, Turn};

/// The keys of `thread_static.json` are NOT ported: each is a table value that holds only under `plain_thread`'s gate (which is why the gate is strict). Per
/// PRESENTER-PORT.md a key is computed or left out, so every one of these comes out of the template as it is computed (src named):
///   tools      menu (Mob.menu.loc: snoozed, Fab.mini.tools, running)
///   permissions (PM.Permissions.json: the thread's runtime mode `rt`)                   picker (Mob.picker.ctx.loc: provider, model, effort, usage, models info)
///   state (Mob.state: turn)   working/phase (SG.Signal.display: signal)   send/sendAct (U.Follow: draft, running, attachments)   boat (UZ.Chip.of)
///   canChild (U.ThreadTree.can_route)   viewer (VW.View.json: the viewer's kind/panes/review/files/hist/mech)   voice (Mob.voice: dictation state)
///   skills (the hub's skills info; parity-session)   chunk/cap (AT.Att, setting files.max.mb)   replying (draft keys)   attaching/uploading (session)
///   parent/subs/asks/order/queued/queue/todos/live/btw/diff/term/embs/sending (null/empty only while the gate's conditions hold)
const STATIC: &str = include_str!("thread_static.json");

/// Whether this thread is the one configuration whose static keys (tools, menu, permissions, picker, viewer, voice ...) were checked against the
/// helper: idle, not pinned/archived/snoozed, Claude with the default model, no worktree, no task links, no open ask, nothing queued, and none of the
/// settings that change those keys. Anything else is None (the helper draws it) until a dump of that case is matched.
/// First refusal gate, with fixed labels only: never a thread id, title, setting value or body.
pub fn metadata_refusal(st: &State, id: &str) -> Option<&'static str> {
    let Some(t)=st.threads.get(id) else { return Some("thread.missing") };
    // thoughts are shown or hidden by the setting agent.thinking: a lean State must keep it
    if st.lean && !crate::state::fam::policy::lean_keeps("agent.thinking") { return Some("state.gate-key-not-retained") }
    if t.deleted { return Some("thread.deleted") }
    if t.title.trim().is_empty() { return Some("thread.title-empty") }
    // settings this view reads (files.max.mb for the attachment limit, the person's zone, agent.thinking): a lean State must keep them, else their absence proves nothing.
    // effort, wait, stop, fail and permissions.default no longer refuse: the effort chip reads effort.<id>; the others change nothing drawn here.
    for key in ["files.max.mb","person.tz","person.tz.device","tz.machine"] {
        if st.lean && !crate::state::fam::policy::lean_keeps(key) { return Some("state.gate-key-not-retained") }
    }
    None
}
fn row_refusal(e: &Ent) -> Option<&'static str> {
    if plain(e) { return None }
    Some(if e.msg {
        if e.id.starts_with("@live:") { "row.live" }
        else if e.text.starts_with("API Error") { "row.api-error" }
        else if e.text.starts_with("Backplane") || e.text.starts_with("[Backplane]") { "row.backplane-note" }
        else if e.role=="user" { "row.attachment" }
        else if e.role=="assistant" { "row.image-or-blank" }
        else { "row.message-role" }
    } else if e.akind=="restart" { "row.restart" }else{ "row.activity-kind" })
}

fn state_keys_refusal(st: &State,id: &str) -> &'static str {
    if st.threads.get(id).map_or(false,|t|t.pending>0) { return "thread.pending" }
    if crate::signal::stalled(st,id) { return "thread.stalled" }
    if st.lean && !crate::state::fam::policy::lean_keeps(&format!("fork.{id}")) { return "state.fork-not-retained" }
    for (key,reason) in [("bg","setting.bg"),("watch","setting.watch"),("auth.wait","setting.auth.wait"),("order.wait","setting.order.wait"),("launch","setting.launch")] {
        if st.settings.get(&format!("{key}.{id}")).map_or(false,|v|!v.is_empty()) { return reason }
    }
    "thread.signal-or-turn"
}

/// Zone.person / Sched.person: the person's zone as a rule. UTC names are built in; any other is the rule the hub stored (`tz.rule.<name>`);
/// a zone with no usable rule reads as UTC, as Sched.person.k does. None: a lean State may have dropped the rule, so the time stamps degrade.
pub(crate) fn person_zone(st: &State) -> Option<Rule> {
    let get = |k: &str| st.settings.get(k).map(String::as_str).unwrap_or("");
    let first = |a: &'static str, b: &'static str, c: &'static str| { for k in [a, b, c] { if !get(k).is_empty() { return get(k).to_string() } } "UTC".to_string() };
    let name = first("person.tz", "person.tz.device", "tz.machine");
    if matches!(name.as_str(), "UTC" | "Etc/UTC" | "Etc/UCT" | "UCT") { return Some(Rule::utc()) }
    let key = format!("tz.rule.{name}");
    if st.lean && !crate::state::fam::policy::lean_keeps(&key) { return None }
    Some(Rule::parse(get(&key)).unwrap_or_else(Rule::utc))
}

/// Sbox.asks: the secrets the agents asked for and the person has not answered: `(name, purpose, thread)`, oldest key first.
pub fn secret_asks(st: &State) -> Vec<(String, String, String)> {
    st.settings.iter().filter_map(|(k, v)| { let n = k.strip_prefix("secret.ask.")?; if v.is_empty() { return None } let b: Value = serde_json::from_str(v).ok()?;
        Some((n.to_string(), b["purpose"].as_str().unwrap_or("").to_string(), b["thread"].as_str().unwrap_or("").to_string())) }).collect()
}

/// shown.thread for a plain idle thread, from the page rows. `hub` is the request's hub key (the id is `<hub>|<thread>`), `draft` the composer's text.
/// None unless every key is derived.
pub fn thread_object(st: &State, hub: &str, thread: &str, params: &Value, draft: &str) -> Option<Value> {
    thread_object_checked(st,hub,thread,params,draft).ok()
}

/// Mob.order: a buy thread's Order card, from its open asks, the quote on its way and the quote the hub keeps; null for any other thread.
pub(crate) fn order_json(st: &State, thread: &str, running: bool) -> Value {
    if st.settings.get(&format!("buy.{thread}")).map_or(true, |v| v.is_empty()) { return Value::Null }
    let mine = || st.ask_data.iter().filter(|a| a.1 == thread);
    let (asked, files) = (mine().any(|a| a.3.starts_with("order_ask")), mine().any(|a| a.3 == "order_quote"));
    let waiting = st.settings.get(&format!("order.wait.{thread}")).map_or(false, |v| !v.is_empty());
    let name = st.threads.get(thread).and_then(|t| st.projects.get(&t.project)).map_or("", |p| p.title.as_str());
    crate::settings::order::card(name, asked, files, waiting, running, st.settings.get(&format!("order.quote.{thread}")).map_or("", String::as_str))
}
/// Mob.needs.q: a buy thread's question: its choices as buttons (the earlier answer sent without its mark), a box whose text goes as `pre` and the text, and Skip.
fn needs_json(id: &str, kind: &str, tool: &str, detail: &str, options: &str) -> Value {
    let typed = tool == "order_ask.typed";
    let chips: Vec<Value> = options.split('\n').map(str::trim).filter(|l| !l.is_empty()).enumerate().map(|(i, l)| json!({"label": l, "value": format!("{id}|{}", crate::settings::order::ask_value(l)), "primary": i == 0 && !typed})).collect();
    json!({"id": id, "kind": kind, "needs": true, "head": "Backplane needs", "detail": detail, "blocks": [], "typed": typed, "pre": format!("{id}|"), "placeholder": if typed { "Type it here" } else { "Or type an answer" },
        "go": "Continue", "skip": {"label": "Skip", "value": format!("{id}|"), "primary": false}, "buttons": chips})
}
/// Mob.asks / Mob.ask.std: the thread's open asks (newest first) as cards with their buttons (English). A buy thread's are "Backplane needs" cards (Mob.needs); a browser task's has its own card: not drawn here.
fn asks_json(st: &State, thread: &str, draft: &str) -> Result<Vec<Value>, &'static str> {
    let _ = st.lean;
    let mine: Vec<_> = st.ask_data.iter().rev().filter(|a| a.1 == thread).collect();
    if mine.is_empty() { return Ok(vec![]) }
    // a browser task's own card is drawn as an ordinary ask card (buttons by the ask's kind)
    let buying = st.settings.get(&format!("buy.{thread}")).map_or(false, |v| !v.is_empty());
    let btn = |l: &str, v: String, primary: bool| json!({"label": l, "value": v, "primary": primary});
    let mut out = Vec::new();
    for (id, _, kind, tool, detail, options) in mine {
        if buying && tool.starts_with("order_ask") { out.push(needs_json(id, kind, tool, detail, options)); continue }
        let head = match kind.as_str() {
            "plan" => "Proposed plan".to_string(), "input" => "The agent asks".to_string(),
            _ => match tool.as_str() {
                "gmail_send" => if detail.starts_with("Draft:") { "Send this draft?".into() } else { "Send this email?".into() },
                "calendar_create" => "Create this event?".into(), "calendar_update" => "Change this event?".into(), "calendar_delete" => "Delete this event?".into(),
                "order_quote" => "Send these files for a quote?".into(), t => format!("Allow {t}?") } };
        let bkind = if kind == "approval" && tool == "order_quote" { "send" } else { kind.as_str() };
        let buttons: Vec<Value> = if tool == "settings_proposal" { vec![btn("Approve", format!("{id}|accept"), true), btn("Reject", format!("{id}|decline"), false)] } else {
            match bkind {
                "plan" => vec![btn("Implement", format!("{id}|implement"), true), btn("Dismiss", format!("{id}|dismiss"), false)],
                "input" => { let mut v: Vec<Value> = options.split('\n').map(|l| btn(l, format!("{id}|{l}"), false)).collect();
                    v.push(if draft.trim().is_empty() { btn("Dismiss", format!("{id}|"), false) } else { btn("Answer with draft", format!("{id}|{}", draft.trim()), true) }); v }
                "send" => vec![btn("Send", format!("{id}|accept"), true), btn("Not now", format!("{id}|decline"), false)],
                _ => vec![btn("Allow", format!("{id}|accept"), true), btn("Allow for session", format!("{id}|session"), false), btn("Deny", format!("{id}|decline"), false)],
            } };
        let files = buying && tool == "order_quote"; // Mob.needs.files: the files' approval keeps its buttons with the needs head
        let head = if files { format!("Backplane needs · {head}") } else { head };
        let mut card = json!({"id": id, "kind": kind, "head": head, "detail": if kind == "plan" { String::new() } else { detail.clone() },
            "blocks": if kind == "plan" { md_blocks(detail) } else { vec![] }, "buttons": buttons});
        if files { card["needs"] = json!(true) }
        out.push(card);
    }
    Ok(out)
}

pub fn thread_object_checked(st: &State, hub: &str, thread: &str, params: &Value, draft: &str) -> Result<Value,&'static str> {
    if let Some(reason)=metadata_refusal(st,thread) { return Err(reason) }
    let page = params.get("page").ok_or("page.missing")?;
    if page["thread"].as_str() != Some(thread) { return Err("page.owner") }
    let rows=page["rows"].as_array().ok_or("page.rows-missing")?;
    let es = ents(rows, thread).ok_or("row.malformed-or-foreign")?;
    if let Some(reason)=es.iter().find_map(row_refusal) { return Err(reason) }
    // Mob.think.hide: a thought (tone "thinking") shows only while the setting `think` is on
    let think_on = st.settings.get("agent.thinking").map_or(false, |v| v == "on");
    let newest_id = es.last().map(|e| e.id.clone()); // U.Hold.newest over the whole page (a hidden thought included)
    let es: Vec<Ent> = es.into_iter().filter(|e| think_on || !(!e.msg && e.role == "thinking")).collect();
    let zone = person_zone(st);
    let running = st.threads.get(thread).map_or(false, |t| t.turn == Turn::Running);
    let now_ms = params["now"].as_u64().unwrap_or(0);
    let mark = |msg: &str| -> u8 { if st.settings.get(&format!("stopped.{msg}")).map_or(false, |v| v == "1") { 1 } else if st.settings.get(&format!("failed.{msg}")).map_or(false, |v| v == "1") { 3 } else { 0 } };
    let turn = st.threads.get(thread).map(|t| t.turn);
    let end = match turn { Some(Turn::Interrupted) => 1, Some(Turn::Failed) => 3, _ => 0 };
    let mut entries = entries(&es, running, &|id| super::window::fold_choice(params, id), now_ms, zone.as_ref(), &mark, end).ok_or("rows.unsupported")?;
    // Mob.pending: a thread that works shows the agent's "Working" row after its last entry
    if running { entries.push(json!({"id": format!("@working:{thread}"), "kind": "assistant", "text": "Working", "blocks": md_blocks("Working"), "images": [], "stamp": ""})) }
    // Signal.why: the first error of the thread's last turn (newest first, stopping at the person's message), at most 400 characters
    let why: String = es.iter().rev().take_while(|e| !(e.msg && e.role == "user")).find(|e| !e.msg && e.role == "error").map(|e| e.text.trim().chars().take(400).collect()).unwrap_or_default();
    let state_keys = super::keys::keys(st, thread, now_ms / 1000, &why, params["hubInfo"]["models"].as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()).as_ref()).ok_or_else(||state_keys_refusal(st,thread))?;
    let t = st.threads.get(thread).ok_or("thread.missing")?;
    let mut o: serde_json::Map<String, Value> = serde_json::from_str(STATIC).map_err(|_|"thread.template")?;
    // the viewer panel the thread has open (Board, Schematic, 3D; other pages are not ported)
    let viewer = super::viewer::viewer_json(st, &o["viewer"], &params["session"]).ok_or("viewer.page-not-ported")?;
    o.insert("viewer".into(), viewer);
    o.insert("diff".into(), crate::session::pages::diff_json(&params["session"], thread));
    o.insert("id".into(), json!(format!("{hub}|{thread}")));
    o.insert("title".into(), json!(t.title));
    o.insert("branch".into(), json!(t.branch));
    o.insert("readingSeen".into(), json!(newest_id.as_deref().unwrap_or("-")));
    // a sent message's references (@bot >thread %project #room &model) as cards the client draws under it
    let mut entries = entries;
    for e in entries.iter_mut() {
        if e["kind"] == "term" { let id = e["id"].as_str().unwrap_or("").to_string(); e["key"] = json!(term_key(thread, &id)); }
        if e["kind"] == "user" { let text = e["text"].as_str().unwrap_or("").to_string(); let mut refs = crate::session::composer::mention::refs_in(st, hub, &text);
            if text.contains('$') { let (cwd, prov, iso) = crate::session::composer::mention::thread_skill_facts(st, thread); refs.extend(crate::agents::skills::chips(&crate::agents::skills::popup_index(&cwd), &text, &prov, iso)); }
            if !refs.is_empty() { e["refs"] = Value::Array(refs); } }
    }
    o.insert("entries".into(), Value::Array(entries));
    // the agent's to-do list and its background work (null while there is none; the client opens and shuts them)
    o.insert("todos".into(), super::panels::todos(st, thread));
    o.insert("subs".into(), super::panels::subs(st, thread));
    // Render.native_screen: thread.earlier = window.remaining when the host sent a window, else the page's own hidden count (0 on the newest page)
    let earlier = if params["window"].is_object() { params["window"]["remaining"].as_u64().unwrap_or(0) } else { 0 };
    o.insert("earlier".into(), json!(earlier));
    // AT.Att.max: the attachment limit in bytes from files.max.mb (default 512 MB, between 1 and 4000)
    let mb = st.settings.get("files.max.mb").and_then(|v| v.trim().parse::<u64>().ok()).map_or(512, |n| n.clamp(1, 4000));
    o.insert("cap".into(), json!(mb * 1048576));
    // the composer's four keys: the draft, the send button and its action, the popup rows (parity-session's session::composer; ours are the `$ @ > % #` & rows, not ported:
    // a mention being typed shows no popup — see docs/slim-thread-known-missing.md). Another interface language is the helper's (None).
    let lang = st.settings.get("lang").map(String::as_str).unwrap_or("");
    // Mob.chip: the attachments waiting in `@att:<thread>` and the file still uploading in `@att.up:<thread>`
    let atts = crate::session::composer::att_paths(params["session"]["drafts"][format!("@att:{thread}")].as_str().unwrap_or(""));
    o.insert("attaching".into(), Value::Array(atts.iter().map(|p| chip(p)).collect()));
    // Rpl: the message being replied to, shown over the composer
    let rid = params["session"]["drafts"][format!("@reply:{thread}")].as_str().unwrap_or("");
    o.insert("replying".into(), if rid.is_empty() { Value::Null } else { json!({"id": rid, "text": params["session"]["drafts"][format!("@reply.q:{thread}")].as_str().unwrap_or("")}) });
    o.insert("uploading".into(), json!(params["session"]["drafts"][format!("@att.up:{thread}")].as_str().unwrap_or("")));
    // the side question over the composer, the masked box, and the words a client must not draw as a message (Draft.preview)
    o.insert("btw".into(), crate::session::composer::btw_json(&params["session"], thread));
    o.insert("secretBox".into(), crate::session::composer::secret_box(&params["session"], thread, &t.title, &secret_asks(st)));
    o.insert("commands".into(), crate::session::composer::words());
    // the reference popup (`@bot >thread %project #room`), from this hub's own state
    let refs = Value::Array(crate::session::composer::mention::rows(st, thread, draft));
    let c = crate::session::composer::fields(draft, running, !atts.is_empty(), t.provider == "claude", &refs, lang).ok_or("composer.language")?;
    for (k, v) in state_keys { o.insert(k, v); }
    o.insert("asks".into(), Value::Array(asks_json(st, thread, draft)?));
    o.insert("order".into(), order_json(st, thread, running));
    // Buy it heads the thread's tools: in the window's header, and the first of the phones' overflow menu, which lists them
    if let Some(b) = super::ready::header(st, thread, &params["session"]) { if let Some(t) = o.get_mut("tools").and_then(Value::as_array_mut) { t.insert(0, b) } }
    for k in ["draft", "skills", "send", "sendAct"] { o.insert(k.into(), c[k].clone()); }
    Ok(Value::Object(o))
}

#[cfg(test)]
mod tests {
    /// The phones fold a run themselves from what the data API sends (mobile DataFold.swift / DataFold.kt): the same table holds all three to these words.
    #[test] fn the_fold_label_table_the_phones_share() {
        let v: Value = serde_json::from_str(include_str!("../../../../test/mobile/fold_labels.json")).unwrap();
        for c in v.as_array().unwrap() {
            let run: Vec<Ent> = c["rows"].as_array().unwrap().iter().map(|r| Ent { id: "a".into(), msg: false, role: if r["step"] == true { "step".into() } else { "tool".into() }, akind: r["akind"].as_str().unwrap().into(), text: r["text"].as_str().unwrap().into(), at: 1 }).collect();
            for (e, r) in run.iter().zip(c["rows"].as_array().unwrap()) { if !is_step(e) { assert_eq!(shown_name(&e.akind), r["name"].as_str().unwrap()); } }
            assert_eq!(label(&run, c["live"] == true), c["label"].as_str().unwrap());
        }
    }

    use super::*;
    fn fixture() -> Value { serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-pair.json")).unwrap()).unwrap() }
    #[test] fn the_entries_of_a_plain_page_are_the_helpers() {
        let p = fixture(); let th = p["params"]["page"]["thread"].as_str().unwrap();
        let es = ents(p["params"]["page"]["rows"].as_array().unwrap(), th).unwrap();
        let got = entries(&es, false, &|_| false, p["params"]["now"].as_u64().unwrap(), Some(&Rule::utc()), &|_| 0, 0).unwrap();
        let want = p["out"]["shown"]["thread"]["entries"].as_array().unwrap();
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) { assert_eq!(g, w); }
    }
    #[test] fn a_plain_threads_whole_shown_thread_is_the_helpers() {
        let p = fixture(); let th = p["params"]["page"]["thread"].as_str().unwrap();
        let mut st = State::new();
        for l in std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).unwrap().lines() { st.apply_line(l); }
        let got = thread_object(&st, "h", th, &p["params"], "").expect("a plain idle thread is drawn here");
        let want = &p["out"]["shown"]["thread"];
        for (k, w) in want.as_object().unwrap() { assert_eq!(got.get(k), Some(w), "key {k}"); }
        // the Allowance chip's text (keys.rs `allowance`) and the picker's other agents (`pickerAgents`) are Backplane's own: the recorded helper reply has no such keys
        assert_eq!(got.as_object().unwrap().keys().filter(|k| !matches!(k.as_str(), "allowance" | "pickerAgents" | "secretBox" | "commands")).count(), want.as_object().unwrap().len());
    }
    // Diff opened on thread A leaves the session's panel on; a view of thread B (the + makes it and goes there) is accepted, not refused as an unported page
    #[test] fn a_view_of_another_thread_after_diff_on_one_is_accepted() {
        let p = fixture(); let th = p["params"]["page"]["thread"].as_str().unwrap();
        let mut st = State::new();
        for l in std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).unwrap().lines() { st.apply_line(l); }
        let mut params = p["params"].clone();
        params["session"]["sel"] = json!(th); params["session"]["panel"] = json!(true);
        params["session"]["info"] = json!({"view.kind:tA": "diff", "panel:tA": "1", "diff:tA": "diff --git a/x b/x\n"});
        let got = thread_object(&st, "h", th, &params, "").expect("thread B is drawn while A's Diff left the panel on");
        assert!(got["diff"].is_null(), "B has no Diff of its own on show");
    }
    // a thread that sent messages into other threads still opens: those rows are left out of its page
    #[test] fn rows_this_thread_sent_elsewhere_are_left_out_not_refused() {
        let rows = json!([["9", "9", "t:t:1", "u", "t", "msg", "user", "", "to u", "9"], ["8", "8", "m1", "t", "", "msg", "user", "", "hi", "8"]]);
        let es = ents(rows.as_array().unwrap(), "t").unwrap(); assert_eq!(es.len(), 1); assert_eq!(es[0].id, "m1");
    }
    #[test] fn rows_may_carry_numbers_or_strings() {
        let a = json!([["5", "5", "m1", "t", "", "msg", "user", "", "hi", "9"]]); let b = json!([[5, 5, "m1", "t", "", "msg", "user", "", "hi", 9]]);
        assert_eq!(ents(a.as_array().unwrap(), "t").unwrap()[0].at, ents(b.as_array().unwrap(), "t").unwrap()[0].at);
    }
    #[test] fn the_persons_zone_sets_the_stamps_and_an_unknown_one_reads_as_utc() {
        let p = fixture(); let th = p["params"]["page"]["thread"].as_str().unwrap();
        let log = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).unwrap();
        let with = |kv: &[(&str, &str)]| { let mut st = State::new(); for l in log.lines() { st.apply_line(l); } for (k, v) in kv { st.apply_line(&format!(r#"{{"$":"SettingSet","key":"{k}","value":"{v}","at":1}}"#)); } st };
        let stamp = |st: &State| thread_object(st, "h", th, &p["params"], "").unwrap()["entries"][0]["stamp"].as_str().unwrap().to_string();
        let utc = stamp(&with(&[]));
        assert_eq!(utc, "Tue 14 Nov 22:51");
        assert_eq!(stamp(&with(&[("tz.machine", "Etc/UTC")])), utc);
        let ny = with(&[("tz.machine", "America/New_York"), ("tz.rule.America/New_York", "EST5EDT,M3.2.0,M11.1.0")]);
        assert_eq!(stamp(&ny), "Tue 14 Nov 17:51", "November: EST, five hours behind");
        assert_eq!(stamp(&with(&[("person.tz", "Europe/Helsinki"), ("tz.rule.Europe/Helsinki", "EET-2EEST,M3.5.0/3,M10.5.0/4"), ("tz.machine", "America/New_York")])), "Tue 14 Nov 00:51".replace("Tue 14", "Wed 15"), "person.tz wins over the machine's");
        assert_eq!(stamp(&with(&[("person.tz", "Mars/Olympus")])), utc, "a zone with no rule reads as UTC (Sched.person.k)");
    }
    #[test] fn the_drafts_composer_keys_come_from_session_composer() {
        let p = fixture(); let th = p["params"]["page"]["thread"].as_str().unwrap();
        let mut st = State::new();
        for l in std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).unwrap().lines() { st.apply_line(l); }
        let o = thread_object(&st, "h", th, &p["params"], "hello").unwrap();
        assert_eq!((o["draft"].as_str(), o["send"].as_str(), o["sendAct"].as_str(), o["skills"].as_array().map(Vec::len)), (Some("hello"), Some("Send"), Some("send"), Some(0)));
        let o = thread_object(&st, "h", th, &p["params"], "/btw why").unwrap();
        assert_eq!(o["send"], "Ask");
        let o = thread_object(&st, "h", th, &p["params"], "/").unwrap();
        assert!(!o["skills"].as_array().unwrap().is_empty(), "a slash being typed lists the commands");
        let o = thread_object(&st, "h", th, &p["params"], "see @bob").unwrap();
        assert!(o["skills"].as_array().is_some(), "a mention being typed: the thread is still drawn");
        st.apply_line(r#"{"$":"SettingSet","key":"lang","value":"ja","at":1}"#);
        assert!(thread_object(&st, "h", th, &p["params"], "").is_none(), "another language is the helper's");
    }
    #[test] fn every_state_variant_of_a_thread_is_the_helpers() {
        let p = fixture(); let th = p["params"]["page"]["thread"].as_str().unwrap();
        let v: Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-variants.json"))).unwrap();
        let log = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/slim/fixtures/thread-log.jsonl")).unwrap();
        let mut checked = Vec::new();
        for (name, x) in v.as_object().unwrap() {
            let mut st = State::new(); for l in log.lines() { st.apply_line(l); } for e in x["extra"].as_array().unwrap() { st.apply(e); }
            // a variant whose events add rows brings its own page; one with open runs brings the session's info
            let mut params = p["params"].clone();
            if x.get("page").map_or(false, |g| !g.is_null()) { params["page"] = x["page"].clone(); }
            if let Some(info) = x.get("info").filter(|i| i.is_object()) { params["session"]["info"] = info.clone(); }
            let got = thread_object(&st, "h", th, &params, "").unwrap_or_else(|| panic!("{name} should be drawn"));
            let (diff, tail) = (x["diff"].as_object().unwrap(), x["tail"].as_array().unwrap());
            // a browser task's ask has a card of its own: here it draws as an ordinary card (a stated degrade); a buy thread's is 0.14's, key for key
            for (k, w) in diff { if name == "askbrowser" && matches!(k.as_str(), "asks" | "order") { continue } assert_eq!(got.get(k), Some(w), "{name}: {k}"); }
            let es = got["entries"].as_array().unwrap();
            assert_eq!(es.len() as u64, x["entries"].as_u64().unwrap(), "{name}: entries");
            assert_eq!(&es[es.len() - 2..], tail.as_slice(), "{name}: tail");
            if let Some(all) = x.get("all").and_then(Value::as_array) { assert_eq!(es, all.as_slice(), "{name}: every entry"); }
            checked.push(name.clone());
        }
        for n in ["base", "running", "completed", "failed", "interrupted", "pinned", "archived", "snoozed", "settledhand", "codex", "opus", "queued", "askapproval", "askinput", "askplan", "asksend", "stepchange", "stepbase", "stepfirst", "stepopen", "apierror", "apierror2", "restartcarry", "blankreply", "thought", "thoughton", "tonethink", "imagereply", "attached", "effort", "waitset", "filesmax", "permdefault", "stalled", "askbrowser", "askbuy"] { assert!(checked.iter().any(|c| c == n), "{n} not checked: {checked:?}"); }
    }
    #[test] fn civil_dates() { assert_eq!(civil(0), (1970, 1, 1)); assert_eq!(civil(19675), (2023, 11, 14)); assert_eq!(clock(1700002806, 0, &Rule::utc()), "Tue 14 Nov 23:00"); }
    #[test] fn labels_name_three_kinds_and_the_live_one_says_what_runs() {
        let t = |k: &str, x: &str| Ent { id: "a".into(), msg: false, role: "tool".into(), akind: k.into(), text: x.into(), at: 1 };
        let run = [t("Bash", "ls"), t("Read", "f"), t("Edit", "g"), t("Grep", "h"), t("Bash", "i")];
        assert_eq!(label(&run, false), "5 tool calls · Bash, Read, Edit, …");
        assert_eq!(label(&run, true), "5 tool calls · running: i");
        assert_eq!(label(&[t("mcp__backplane__viewer_show", "")], false), "1 tool call · viewer_show");
    }
}

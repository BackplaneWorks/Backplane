//! A thread page's `window` object, in Rust (src/server/thin-window `Render.window.context`, src/core/fold `Fold.roles.all`,
//! src/core/message-context, src/core/lazy-tools), from the rows `Model::page` reads out of the index: no helper, no fold of the log.
//! Pure: the page (32 ids + 17 lookahead rows + the counts below/above), the newest entries for an older page, the request. The answer is the
//! object the helper's V returns as `window`. `None`: not ported (body pages, `bodyPages` = 1).
use crate::model::{Page, Row};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// the entry a row draws (T.Row.preview): an activity shows its first 512-byte block
#[derive(Clone, Debug)]
struct E { id: String, msg: bool, role: String, kind: String, text: String, private: bool }

/// T.Blocks: the text cut into blocks of at most 512 bytes (by characters); (first block, block count)
fn blocks(s: &str) -> (String, usize) {
    let (mut first, mut cur, mut bytes, mut n) = (String::new(), String::new(), 0usize, 0usize);
    for c in s.chars() {
        let sz = c.len_utf8();
        if bytes + sz <= 512 { cur.push(c); bytes += sz } else { if n == 0 { first = cur.clone() } n += 1; cur = c.to_string(); bytes = sz }
    }
    if n == 0 { first = cur } n += 1;
    (first, n)
}
/// `bot`: the thread is a bot's conversation, where the hub's reports and notes to the bot draw nothing (`thread::bot_private`)
fn entry_in(r: &Row, bot: bool) -> E { entry_hid(r, bot, &Default::default()) }
/// the same, `backend` the replies of turns that were not the person's (`thread::backend_replies`): left out of a bot's conversation as well
fn entry_hid(r: &Row, bot: bool, backend: &std::collections::HashSet<String>) -> E {
    let msg = r.kind == "msg";
    let private = msg && bot && (super::thread::bot_private(&r.id, &r.role, &r.text) || backend.contains(&r.id));
    E { id: r.id.clone(), msg, role: r.role.clone(), kind: r.akind.clone(), text: if msg { crate::settings::order::pay_shown(&r.text) } else { blocks(&r.text).0 }, private }
}

// ---- fold ----
fn white(s: &str) -> bool { s.chars().all(|c| matches!(c, ' ' | '\n' | '\r' | '\t')) }
const NOTES: [&str; 4] = ["The quote is posted in this thread. ", "The quote could not be worked out: ", "The files were not taken: ", "The person chose not to send the files (Not now)."];
fn note(x: &str) -> bool { NOTES.iter().any(|h| x.starts_with(&format!("[Backplane] {h}"))) }
/// Fold.blank: a message that shows nothing
fn blank(e: &E) -> bool { e.msg && (e.private || (e.role == "assistant" && white(&e.text)) || (e.role == "system" && note(&e.text))) }
const CARRY: &str = "Backplane restarted while you were working. Continue where you left off.";
fn api_err(x: &str) -> bool { x.starts_with("API Error") }
fn told(x: &str) -> bool { x.starts_with(CARRY) }
fn dup(e: &E, p: &str) -> bool {
    if e.msg { return false }
    let restart = e.kind == "restart" && e.text == "Continuing after the restart" && told(p);
    let api = api_err(p) && e.role == "error" && e.kind == "result" && (e.text.trim().is_empty() || e.text.trim() == p.trim());
    restart || api
}
/// Fold.shown
fn shown(es: &[E]) -> Vec<E> {
    let (mut out, mut p) = (Vec::new(), String::new());
    for e in es {
        let b = blank(e);
        if !(b || dup(e, &p)) { out.push(e.clone()) }
        p = if b { p } else if e.msg && e.role == "assistant" { if api_err(&e.text) { e.text.clone() } else { String::new() } }
            else if e.msg && e.role == "user" { if told(&e.text) { e.text.clone() } else { String::new() } } else { String::new() };
    }
    out
}
fn tool(e: &E) -> bool { !e.msg && (e.role == "tool" || e.role == "step") }
#[derive(Clone, Debug, PartialEq)]
enum Role { Show, Head { id: String, open: bool }, In { open: bool } }
/// Fold.roles.go over Fold.groups (no live run: the window is drawn for a settled page)
fn roles(es: &[E], open: &dyn Fn(&str) -> bool) -> Vec<Role> {
    let mut out = Vec::new(); let mut i = 0;
    while i < es.len() {
        if tool(&es[i]) {
            let mut j = i; while j < es.len() && tool(&es[j]) { j += 1 }
            let o = open(&es[i].id);
            out.push(Role::Head { id: es[i].id.clone(), open: o });
            for _ in i + 1..j { out.push(Role::In { open: o }) }
            i = j;
        } else { out.push(Role::Show); i += 1 }
    }
    out
}
/// Fold.spread: one role per entry; a blank entry is hidden (an In of a shut run), the others take the roles of the entries that show
fn spread(es: &[E], rs: Vec<Role>) -> Vec<Role> {
    let mut it = rs.into_iter(); let mut out = Vec::new();
    for e in es { if blank(e) { out.push(Role::In { open: false }) } else { out.push(it.next().unwrap_or(Role::Show)) } }
    out
}
/// Context.roles.go (no roots): the run's open state is the person's choice, a hidden entry keeps the one before it
fn context_roles(es: &[E], rs: Vec<Role>, open: &dyn Fn(&str) -> bool) -> Vec<Role> {
    let (mut out, mut prev) = (Vec::new(), false);
    for (e, r) in es.iter().zip(rs) {
        let role = match r { Role::Head { id, .. } => { let o = open(&id); Role::Head { id, open: o } } Role::In { open: was } => Role::In { open: if blank(e) { was } else { prev } }, s => s };
        prev = match &role { Role::Head { open, .. } => *open, Role::In { .. } => prev, Role::Show => false };
        out.push(role);
    }
    out
}
/// Context.unit
fn units(es: &[E], rs: &[Role]) -> Vec<String> {
    let mut out = Vec::new();
    for (e, r) in es.iter().zip(rs) {
        match r {
            Role::Show => out.push(e.id.clone()),
            Role::Head { id, open, .. } => { out.push(format!("fold:{id}")); if *open { out.push(e.id.clone()) } }
            Role::In { open } => { if *open { out.push(e.id.clone()) } }
        }
    }
    out
}
/// Context.latest: the newest human prompt and the newest assistant text that shows, newest first in, ids in the order met
fn latest(newest_first: &[E]) -> Vec<String> {
    let (mut human, mut agent, mut out) = (false, false, Vec::new());
    for e in newest_first {
        if human && agent { break }
        if e.msg && e.role == "user" && !e.private && !human { out.push(e.id.clone()); human = true }
        else if e.msg && e.role == "assistant" && !agent && !blank(e) { out.push(e.id.clone()); agent = true }
    }
    out
}
/// Step.base / Hist.seen: the design as it was before a thread's first step is not a row
fn step_base(e: &E) -> bool {
    !e.msg && e.role == "step" && e.text.contains("\"p\":\"\"") && (e.text.contains("\"w\":\"start\"") || !e.text.contains("\"w\":"))
}

fn num(s: &str) -> u64 { s.parse().unwrap_or(0) }
/// W.Cursor.write
fn cursor(epoch: &str, target: &Value, query: &str, before: u64) -> String {
    format!("{{\"version\":1,\"epoch\":{},\"target\":{},\"query\":{},\"before\":\"{}\"}}", json!(epoch), target, json!(query), before)
}

/// The `fold:<id>` choices of the request: the session's info, then a `fold` action's own
pub(super) fn fold_choice(p: &Value, id: &str) -> bool {
    let key = format!("fold:{id}");
    if p["action"] == "fold" { let v = p["value"].as_str().unwrap_or(""); if v.len() > 1 && v[1..] == *id { return &v[..1] == "1" } }
    p["session"]["info"][&key].as_str() == Some("1")
}

/// The window of the page `before` (0: the newest) of `thread`. `ctx` is the thread's newest entries, newest first, for a page that is not the newest.
/// `bot`: the thread is a bot's conversation (the uberbot's chat): what the hub hands the bot is left out (`thread::bot_private`).
pub fn window(page: &Page, ctx: &[Row], epoch: &str, before: u64, p: &Value) -> Option<Value> { window_in(page, ctx, epoch, before, p, false) }
pub fn window_in(page: &Page, ctx: &[Row], epoch: &str, before: u64, p: &Value, bot: bool) -> Option<Value> {
    if p["bodyPages"].as_u64() == Some(1) { return None }
    let by: HashMap<u64, &Row> = page.rows.iter().map(|r| (r.ord, r)).collect();
    let ids = &page.ids;
    let backend = if bot { let mut rs: Vec<&Row> = page.rows.iter().collect(); rs.sort_by_key(|r| r.seq); super::thread::backend_replies(rs.into_iter().map(|r| (r.id.as_str(), r.kind == "msg", r.role.as_str(), r.text.as_str())), p["backendBefore"] == true) } else { Default::default() };
    let ents: Vec<(u64, E)> = ids.iter().rev().filter_map(|id| by.get(&num(id)).map(|r| (r.ord, entry_hid(r, bot, &backend)))).collect(); // oldest first
    let open = |id: &str| fold_choice(p, id);
    let es: Vec<E> = ents.iter().map(|(_, e)| e.clone()).filter(|e| !step_base(e)).collect();
    let rs = context_roles(&es, spread(&es, roles(&shown(&es), &open)), &open);
    let newest_first: Vec<E> = ents.iter().rev().map(|(_, e)| e.clone()).collect();
    let context: Vec<String> = if ctx.is_empty() { latest(&newest_first) } else { latest(&ctx.iter().map(|r| entry_in(r, bot)).collect::<Vec<_>>()) };
    let messages: Vec<String> = es.iter().filter(|e| e.msg && matches!(e.role.as_str(), "user" | "assistant") && !blank(e)).map(|e| e.id.clone()).collect();
    // the counts: the rows the page holds older / newer than the window's ids, plus what the store omitted
    let (first, last) = (ids.first().map(|s| num(s)), ids.last().map(|s| num(s)));
    let older_rows = |than: u64| page.rows.iter().filter(|r| r.ord < than).count() as u64;
    let newer_rows = |than: u64| page.rows.iter().filter(|r| r.ord > than).count() as u64;
    let remaining = last.map_or(0, |l| older_rows(l) + page.below);
    let newer = first.map_or(0, |f| newer_rows(f) + page.above);
    let has_older = last.map_or(false, |l| older_rows(l) > 0);
    let mid = ids.get(15).or(ids.last()).map(|s| num(s)).unwrap_or(0);
    let target = &p["target"];
    let older = if has_older { cursor(epoch, target, "", mid) } else { String::new() };
    let a = &p["anchor"];
    let anchor = if a["id"].as_str().map_or(false, |s| s.len() <= 1024) && a["offset"].as_u64().is_some() { json!({"id": a["id"], "offset": a["offset"]}) } else { json!({"id": "", "offset": 0}) };
    let partial: Vec<Value> = ids.iter().filter_map(|id| by.get(&num(id)).copied()).filter(|r| r.kind != "msg" && blocks(&r.text).1 > 1)
        .map(|r| json!({"id": r.id, "version": r.ord.to_string(), "cursor": cursor(epoch, target, &format!("message:{}", r.ord), 1), "hasMore": true, "complete": false, "reason": "byte-limit"})).collect();
    let mut w = Map::new();
    w.insert("ids".into(), json!(ids)); w.insert("anchor".into(), anchor); w.insert("older".into(), json!(older)); w.insert("olderNext".into(), json!(""));
    w.insert("hasMore".into(), json!(has_older)); w.insert("remaining".into(), json!(remaining)); w.insert("newer".into(), json!(newer));
    w.insert("total".into(), json!(remaining + ids.len() as u64 + newer)); w.insert("partial".into(), json!(partial));
    w.insert("historyUnits".into(), json!(units(&es, &rs))); w.insert("messageIds".into(), json!(messages)); w.insert("contextMessages".into(), json!(context));
    Some(Value::Object(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fx(f: &str) -> Value { serde_json::from_str(&std::fs::read_to_string(format!("{}/../../test/hub/fixtures/{f}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap() }
    fn row(c: &Value) -> Row {
        let s = |i: usize| c[i].as_str().unwrap_or("").to_string();
        Row { seq: c[0].as_u64().unwrap(), ord: c[1].as_u64().unwrap(), id: s(2), thread: s(3), src: s(4), kind: if c[5] == "msg" { "msg" } else { "act" }, role: s(6), akind: s(7), text: s(8), at: c[9].as_u64().unwrap() }
    }
    #[test] fn blocks_cut_at_512_bytes() {
        assert_eq!(blocks("abc"), ("abc".into(), 1));
        let s = "x".repeat(600); let (f, n) = blocks(&s); assert_eq!((f.len(), n), (512, 2));
        let u = "é".repeat(300); let (f, n) = blocks(&u); assert_eq!((f.len(), n), (512, 2));
    }
    /// the six reference threads of the JS hub (test/hub/fixtures/windows.json) from the newest page of their entries
    #[test] fn the_window_of_the_newest_page_is_the_js_hubs() {
        let wins = fx("windows.json"); let pages = fx("pages.json");
        let mut seen = 0;
        for w in wins.as_array().unwrap() {
            let th = w["thread"].as_str().unwrap();
            let pg = pages["pages"].as_array().unwrap().iter().find(|p| p["thread"] == th && p["before"] == 0).unwrap();
            let page = Page { ids: pg["ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect(), below: pg["below"].as_u64().unwrap(), above: pg["above"].as_u64().unwrap(), rows: pg["rows"].as_array().unwrap().iter().map(row).collect() };
            let got = window(&page, &[], w["epoch"].as_str().unwrap(), 0, &w["request"]).unwrap();
            for (k, want) in w["window"].as_object().unwrap() { assert_eq!(got.get(k), Some(want), "{th}: {k}"); }
            seen += 1;
        }
        assert_eq!(seen, 6);
    }
    /// Bot.private (Nov 10, Bub's chat on the phone): in a bot's conversation a launched thread's report is no message of the window; elsewhere it is
    #[test] fn a_bots_window_leaves_out_what_the_hub_hands_the_bot() {
        let m = |ord: u64, id: &str, role: &str, text: &str| Row { seq: ord, ord, id: id.into(), thread: "bub".into(), src: String::new(), kind: "msg", role: role.into(), akind: String::new(), text: text.into(), at: 1700000000 + ord };
        let report = "[Backplane] Launched thread \"Always-on hub\" (t9) completed:\n\nBub: the prep is done.\n\n**Prep**\n- `bp-hub` is up";
        let rows = vec![m(1, "u1", "user", "move the hub to the Boat"), m(2, "a1", "assistant", "On it."), m(3, "t:t9:lr-r1", "user", report),
            m(4, "n1", "system", "[Backplane] Watched work \"build\" ended: exit 0"), m(5, "a2", "assistant", "The Boat is ready; say GO.")];
        let page = Page { ids: (1..=5).rev().map(|n| n.to_string()).collect(), below: 0, above: 0, rows };
        let ids = |w: &Value| w["messageIds"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>();
        let bot = window_in(&page, &[], "e", 0, &json!({"target":{}}), true).unwrap();
        assert_eq!(ids(&bot), ["u1", "a1"], "the reply to the report is private too (Bot.backend): {bot}");
        assert_eq!(bot["contextMessages"], json!(["a1", "u1"]), "the person's prompt is theirs, not the report");
        let plain = window_in(&page, &[], "e", 0, &json!({"target":{}}), false).unwrap();
        assert_eq!(ids(&plain), ["u1", "a1", "t:t9:lr-r1", "a2"], "a thread that is no bot's keeps its reports");
    }
}

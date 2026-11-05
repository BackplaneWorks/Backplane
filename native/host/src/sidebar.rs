//! The sidebar's `shown.projects` for the native clients (renderer `mobile`, locale English) from the Rust read model:
//! src/mobile/screen.bend `Mob.projects`/`Mob.project`/`Mob.row`, core/client.bend `Side.*`, core/subthreads.bend `Tree.*`,
//! core/model.bend `Sidebar.at`/`Archive.of`. The Bend code stays as the differential oracle (bp-policy on the same log).
//!
//! Covered: project order and live/quiet/older grouping, the five shelves with their order, thread tree rows (parents set by
//! `thread.parent.<id>`), status words, ago, lead and trail swipes, the Settled count, qualified ids (`<hub>|<id>`).
//! Not covered (listed by the comparison, never silent): manual order keys (ThreadOrdered/ProjectOrdered), threads of other
//! machines (far), bot and mail-run threads, Scratch, sync-off machines, other locales, search.
use crate::shelf::{shelf_at, Shelf};
use crate::signal;
use crate::state::{Override, State, Thread};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub struct Opts<'a> {
    /// the hub's key: ids are qualified `<hub>|<id>`
    pub hub: &'a str,
    /// seconds since the epoch
    pub now: u64,
    /// the open thread ("" for none)
    pub selected: &'a str,
    /// projects (and `arch:<project>`) the person opened
    pub open: &'a HashSet<String>,
    /// the "old-projects" flag: show the folded Older group
    pub older_open: bool,
}

const QUIET_DAYS: u64 = 259_200;

fn q(o: &Opts, id: &str) -> String { format!("{}|{}", o.hub, id) }

/// Time.ago (I18n.ago, English): "now", "5m", "3h", "9d"
pub fn ago(now: u64, at: u64) -> String {
    let s = now.saturating_sub(at);
    if s < 60 { "now".into() } else if s < 3600 { format!("{}m", s / 60) } else if s < 86400 { format!("{}h", s / 3600) } else { format!("{}d", s / 86400) }
}
fn state_word(t: &Thread) -> &'static str { match t.turn { crate::state::Turn::Running => "run", crate::state::Turn::Failed => "fail", crate::state::Turn::Interrupted => "stop", _ => "idle" } }

fn rank(t: &Thread) -> u64 { t.created.max(t.unsettled_at) }

// ---- manual order keys (core/model.bend "Order keys"): a binary fraction 0.b1b2b3..., zeros after its last digit ----------
fn key_read(hex: &str) -> Vec<bool> {
    hex.bytes().flat_map(|c| { let v = match c { b'0'..=b'9' => c - b'0', b'a'..=b'f' => c - b'a' + 10, _ => 0 }; [v & 8 != 0, v & 4 != 0, v & 2 != 0, v & 1 != 0] }).collect()
}
/// Key.cmp: "1" and "10" are the same place
fn key_cmp(a: &[bool], b: &[bool]) -> std::cmp::Ordering {
    let n = a.len().max(b.len());
    for i in 0..n { let (x, y) = (a.get(i).copied().unwrap_or(false), b.get(i).copied().unwrap_or(false)); if x != y { return if x { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less } } }
    std::cmp::Ordering::Equal
}
fn order_key(st: &State, id: &str, section: &str) -> Vec<bool> { st.order_keys.get(id).map(|k| key_read(if section == "pinned" { &k.0 } else { &k.1 })).unwrap_or_default() }
/// Slot.before for the pinned and active shelves: keyed threads first, by key; the rest newest first
fn slot_cmp(st: &State, section: &str, a: &str, b: &str) -> std::cmp::Ordering {
    let (ka, kb) = (order_key(st, a, section), order_key(st, b, section));
    match (ka.is_empty(), kb.is_empty()) {
        (false, false) => key_cmp(&ka, &kb),
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        (true, true) => rank(&st.threads[b]).cmp(&rank(&st.threads[a])),
    }
}

// ---- the thread tree (core/subthreads.bend) -------------------------------------------------------------------------------
fn parent_raw<'a>(st: &'a State, id: &str) -> &'a str {
    let p = st.settings.get(&format!("thread.parent.{id}")).map(|s| s.as_str()).unwrap_or("");
    if p.is_empty() || p == id { return "" }
    match (st.threads.get(id), st.threads.get(p)) {
        (Some(a), Some(b)) if !a.deleted && !b.deleted && a.project == b.project => p,
        _ => "",
    }
}
/// Tree.chain: the raw ancestry is at most 64 deep and never returns to the start
fn chain_ok<'a>(st: &'a State, start: &str, mut p: &'a str) -> bool {
    for _ in 0..64 { if p.is_empty() { return true } if p == start { return false } p = parent_raw(st, p); }
    p.is_empty()
}
fn parent<'a>(st: &'a State, id: &str) -> &'a str { let p = parent_raw(st, id); if chain_ok(st, id, p) { p } else { "" } }
/// Tree.display_parent: archiving moves one conversation; live children of an archived parent become roots
fn display_parent<'a>(st: &'a State, id: &str) -> &'a str {
    let p = parent(st, id);
    if p.is_empty() { return "" }
    if st.threads[id].archived == st.threads[p].archived { p } else { "" }
}
fn display_raw<'a>(st: &'a State, id: &str) -> &'a str {
    let p = parent_raw(st, id);
    if p.is_empty() { return "" }
    if st.threads[id].archived == st.threads[p].archived { p } else { "" }
}
/// Tree.root
fn root<'a>(st: &'a State, id: &'a str) -> &'a str {
    let mut cur = id; let mut p = display_parent(st, id);
    for _ in 0..64 { if p.is_empty() { return cur } cur = p; p = display_raw(st, cur); }
    cur
}
fn ancestor(st: &State, from: &str, id: &str) -> bool {
    let mut cur = from.to_string();
    for _ in 0..64 { if cur.is_empty() { return false } let p = display_parent(st, &cur).to_string(); if p == id { return true } cur = p; }
    false
}
fn tree_open(st: &State, id: &str, selected: &str) -> bool {
    match st.settings.get(&format!("thread.tree.{id}")).map(|s| s.as_str()).unwrap_or("") { "" => ancestor(st, selected, id), v => v == "1" }
}
/// Tree.before: created, then id
fn tree_before(a: (&str, &Thread), b: (&str, &Thread)) -> std::cmp::Ordering { (a.1.created, a.0).cmp(&(b.1.created, b.0)) }

struct Row<'a> { id: &'a str, t: &'a Thread, depth: usize, expanded: bool, children: usize }

/// Tree.rows: the roots in the order given, each followed (when open) by its children, depth first
fn tree_rows<'a>(st: &'a State, roots: &[&'a str], selected: &str, kids_by: &HashMap<&'a str, Vec<&'a str>>) -> Vec<Row<'a>> {
    let mut out = vec![]; let mut stack: Vec<(&str, usize)> = roots.iter().rev().map(|r| (*r, 0)).collect();
    let mut fuel = 1 + st.threads.len();
    while let Some((id, depth)) = stack.pop() {
        if fuel == 0 { break } fuel -= 1;
        let kids: &[&str] = kids_by.get(id).map(|v| v.as_slice()).unwrap_or(&[]);
        let open = tree_open(st, id, selected);
        out.push(Row { id, t: &st.threads[id], depth, expanded: open, children: kids.len() });
        if open { for k in kids.iter().rev() { stack.push((k, depth + 1)); } }
    }
    out
}

// ---- one project ---------------------------------------------------------------------------------------------------------
fn swipe(label: &str, action: &str, value: String, tone: &str, options: Vec<Value>) -> Value { json!({"label": label, "action": action, "value": value, "tone": tone, "options": options}) }

fn thread_json(st: &State, o: &Opts, ix: &HashMap<String, u8>, r: &Row, shelf: u8, until: u64) -> Value {
    let id = r.id; let t = r.t; let qid = q(o, id);
    let when = if shelf == 2 { ago(until, o.now) } else { ago(o.now, t.activity) };
    let lead = match shelf {
        0 => vec![swipe("Settle", "row-settle", qid.clone(), "settle", vec![])],
        1 => vec![swipe("Unsettle", "row-unsettle", qid.clone(), "settle", vec![])],
        2 => vec![swipe("Wake", "row-snooze", format!("{qid}|off"), "settle", vec![])],
        _ => vec![],
    };
    let snooze = vec![json!({"label": "1 hour", "value": format!("{qid}|1h")}), json!({"label": "Tomorrow", "value": format!("{qid}|tomorrow")}), json!({"label": "1 week", "value": format!("{qid}|1w")})];
    let trail = vec![swipe("Delete", "row-delete", qid.clone(), "danger", vec![]), swipe("Snooze", "row-snooze", String::new(), "", snooze)];
    let title = if t.title == "New thread" { "New thread".to_string() } else { t.title.clone() };
    json!({"depth": r.depth, "expanded": r.expanded, "children": r.children, "id": qid, "title": title, "state": state_word(t), "ago": when,
           "agents": agents_note(st, id), "pinned": t.pinned, "status": signal::display_name(st, ix[id], id), "canChild": !t.deleted, "lead": lead, "trail": trail})
}

/// Signal.display.note: a stall says why; else the subagent badge (Subs.count.ix: unfinished child tasks + subagent rows at work)
fn agents_note(st: &State, id: &str) -> String {
    if signal::stalled(st, id) {
        let why = st.settings.get(&format!("liveness.{id}")).and_then(|v| serde_json::from_str::<Value>(v).ok()).and_then(|v| v.get("reason").and_then(|r| r.as_str().map(|s| s.to_string()))).unwrap_or_default();
        return format!("Stalled: {why}");
    }
    let kids = st.tasks.values().filter(|(p, _, term, _)| p == id && !*term).count();
    let bg = st.settings.get(&format!("bg.{id}")).map(|s| s.as_str()).unwrap_or("");
    let agents = bg.split('\n').filter(|l| !l.is_empty()).filter(|l| l.split('\t').nth(4).map_or(false, |k| k == "subagent") && !l.split('\t').nth(1).unwrap_or("").starts_with("\u{1}ended:")).count();
    let n = kids + agents;
    match n { 0 => String::new(), 1 => "1 subthread".into(), n => format!("{n} subthreads") }
}

struct Shelves<'a> { pinned: Vec<&'a str>, active: Vec<&'a str>, settled: Vec<&'a str>, hidden: Vec<&'a str>, snoozed: Vec<&'a str> }

fn shelves_of<'a>(st: &'a State, project: &str, now: u64) -> Shelves<'a> {
    let mut ids: Vec<&str> = st.threads.iter().filter(|(_, t)| t.project == project).map(|(i, _)| i.as_str()).collect();
    ids.sort();
    let mut s = Shelves { pinned: vec![], active: vec![], settled: vec![], hidden: vec![], snoozed: vec![] };
    for id in ids {
        let t = &st.threads[id]; let until = st.snooze_until.get(id).copied().unwrap_or(0);
        match shelf_at(t, until, now) { Shelf::Pinned => s.pinned.push(id), Shelf::Active => s.active.push(id), Shelf::Settled => s.settled.push(id), Shelf::Hidden => s.hidden.push(id), Shelf::Snoozed => s.snoozed.push(id) }
    }
    // Sidebar.order.at: newest first (rank), settled by settle time, snoozed soonest to wake; ties keep id order
    s.pinned.sort_by(|a, b| slot_cmp(st, "pinned", a, b));
    s.active.sort_by(|a, b| slot_cmp(st, "active", a, b));
    s.settled.sort_by(|a, b| st.threads[*b].settled_at.cmp(&st.threads[*a].settled_at));
    s.snoozed.sort_by(|a, b| st.snooze_until.get(*a).copied().unwrap_or(0).cmp(&st.snooze_until.get(*b).copied().unwrap_or(0)));
    s
}

/// Tree.shelves.k: the subtree sits on the earliest shelf of any of its conversations: each shelf lists the ROOTS of what it holds, once
fn tree_shelves<'a>(st: &'a State, s: &Shelves<'a>) -> Shelves<'a> {
    let any_kid = [&s.pinned, &s.active, &s.settled, &s.hidden, &s.snoozed].iter().any(|v| v.iter().any(|id| st.settings.contains_key(&format!("thread.parent.{id}"))));
    let alive = |v: &Vec<&'a str>| -> Vec<&'a str> { v.iter().copied().filter(|id| !st.threads[*id].deleted).collect() };
    if !any_kid { return Shelves { pinned: alive(&s.pinned), active: alive(&s.active), settled: alive(&s.settled), hidden: alive(&s.hidden), snoozed: alive(&s.snoozed) } }
    let roots = |v: &Vec<&'a str>, seen: &mut HashSet<&'a str>| -> Vec<&'a str> {
        let mut out = vec![];
        for id in v { if st.threads[*id].deleted { continue } let r = root(st, id); let r: &'a str = st.threads.get_key_value(r).map(|(k, _)| k.as_str()).unwrap(); if seen.insert(r) { out.push(r); } }
        out
    };
    let mut seen: HashSet<&str> = HashSet::new();
    let pinned = roots(&s.pinned, &mut seen); let active = roots(&s.active, &mut seen); let snoozed = roots(&s.snoozed, &mut seen); let settled = roots(&s.settled, &mut seen);
    let mut seen_h: HashSet<&str> = HashSet::new(); let hidden = roots(&s.hidden, &mut seen_h);
    Shelves { pinned, active, settled, hidden, snoozed }
}

fn project_json(st: &State, o: &Opts, ix: &HashMap<String, u8>, pid: &str, sh: &Shelves, kids_by: &HashMap<&str, Vec<&str>>, quiet: bool) -> Value {
    let p = &st.projects[pid];
    let rows = |ids: &[&str], shelf_of: &dyn Fn(&str) -> u8| -> Vec<Value> {
        let roots: Vec<&str> = ids.to_vec();
        tree_rows(st, &roots, o.selected, kids_by).iter().map(|r| {
            let until = st.snooze_until.get(r.id).copied().unwrap_or(0);
            thread_json(st, o, ix, r, shelf_of(r.id), until)
        }).collect()
    };
    let shelf_num = |id: &str| -> u8 { let t = &st.threads[id]; let until = st.snooze_until.get(id).copied().unwrap_or(0); match shelf_at(t, until, o.now) { Shelf::Settled => 1, Shelf::Snoozed => 2, Shelf::Hidden => 3, _ => 0 } };
    let mut live_ids: Vec<&str> = sh.pinned.clone(); live_ids.extend(sh.active.iter().copied());
    // Archive.of: archived (not deleted) newest first
    let mut arch: Vec<&str> = sh.hidden.iter().copied().filter(|id| st.threads[*id].archived && !st.threads[*id].deleted).collect();
    arch.sort_by(|a, b| rank(&st.threads[*b]).cmp(&rank(&st.threads[*a])));
    json!({"machine": "", "quiet": quiet, "id": q(o, pid), "title": p.title, "root": p.root, "icon": "folder", "open": o.open.contains(pid),
           "threads": rows(&live_ids, &shelf_num), "snoozedShelf": "Snoozed", "snoozed": rows(&sh.snoozed, &shelf_num), "shelf": format!("Settled {}", sh.settled.len()),
           "settled": rows(&sh.settled, &shelf_num), "value": format!("{}|arch:{}", o.hub, pid), "archOpen": o.open.contains(&format!("arch:{pid}")), "archived": rows(&arch, &shelf_num)})
}

/// The projects as the sidebar groups them: live, then quiet, then (when its shelf is open) the older ones
pub fn projects(st: &State, o: &Opts) -> Vec<Value> {
    let ix = signal::index(st);
    let mut kids_by: HashMap<&str, Vec<&str>> = HashMap::new();
    {
        let mut all: Vec<(&str, &Thread)> = st.threads.iter().map(|(i, t)| (i.as_str(), t)).collect();
        all.sort_by(|a, b| tree_before(*a, *b));
        for (id, _) in all { let p = display_parent(st, id); if !p.is_empty() { kids_by.entry(p).or_default().push(id); } }
    }
    struct P<'a> { id: &'a str, latest: u64, key: Vec<bool>, sh: Shelves<'a> }
    let mut ps: Vec<P> = vec![];
    for (id, p) in &st.projects {
        if p.removed { continue }
        let raw = shelves_of(st, id, o.now);
        // Side.latest: a thread's last move for its project's place; settling by hand counts
        let mut latest = p.created;
        for tid in st.threads.iter().filter(|(_, t)| t.project == *id).map(|(i, _)| i) { let t = &st.threads[tid]; let touched = if t.override_ == Override::ForcedSettled { t.activity.max(t.settled_at) } else { t.activity }; latest = latest.max(touched); }
        ps.push(P { id: id.as_str(), latest, key: order_key(st, &format!("project:{id}"), "active"), sh: tree_shelves(st, &raw) });
    }
    // PSlot.before: keyed projects first, by key; then latest first, then id (SRow.before); equal keys fall back to SRow.before
    ps.sort_by(|a, b| {
        let by_row = b.latest.cmp(&a.latest).then(a.id.cmp(b.id));
        match (a.key.is_empty(), b.key.is_empty()) {
            (false, false) => match key_cmp(&a.key, &b.key) { std::cmp::Ordering::Equal => by_row, o => o },
            (false, true) => std::cmp::Ordering::Less, (true, false) => std::cmp::Ordering::Greater, (true, true) => by_row,
        }
    });
    let (mut live, mut quiet, mut older) = (vec![], vec![], vec![]);
    for p in &ps {
        let is_live = !(p.sh.pinned.is_empty() && p.sh.active.is_empty() && p.sh.snoozed.is_empty());
        if is_live { live.push(p) } else if p.latest + QUIET_DAYS > o.now { quiet.push(p) } else { older.push(p) }
    }
    let mut out = vec![];
    for p in &live { out.push(project_json(st, o, &ix, p.id, &p.sh, &kids_by, false)); }
    for p in &quiet { out.push(project_json(st, o, &ix, p.id, &p.sh, &kids_by, true)); }
    if o.older_open { for p in &older { out.push(project_json(st, o, &ix, p.id, &p.sh, &kids_by, true)); } }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fold(lines: &[&str]) -> State { let mut s = State::new(); for l in lines { s.apply_line(l); } s }
    const PC: &str = r#"{"$":"ProjectCreated","id":"p1","title":"proj","root":"/r","at":10}"#;
    fn tc(id: &str, at: u64) -> String { format!(r#"{{"$":"ThreadCreated","id":"{id}","project":"p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":{at}}}"#) }
    fn opts<'a>(open: &'a HashSet<String>, now: u64) -> Opts<'a> { Opts { hub: "h", now, selected: "", open, older_open: false } }

    #[test] fn ago_words() { assert_eq!(ago(100, 100), "now"); assert_eq!(ago(100, 40), "1m"); assert_eq!(ago(10000, 0), "2h"); assert_eq!(ago(200000, 0), "2d"); assert_eq!(ago(5, 9), "now"); }
    #[test] fn new_threads_sort_newest_first_and_a_quiet_project_is_marked() {
        let s = fold(&[PC, &tc("a", 11), &tc("b", 12)]); let open = HashSet::new();
        let v = projects(&s, &opts(&open, 100));
        let ids: Vec<&str> = v[0]["threads"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["h|b", "h|a"]); assert_eq!(v[0]["quiet"], false); assert_eq!(v[0]["shelf"], "Settled 0");
        // a project with threads on live shelves stays live however old; settle both by hand and it is Older once Quiet.days have passed
        let s = fold(&[PC, &tc("a", 11), r#"{"$":"ThreadSettled","id":"a","manual":true,"at":20}"#]);
        let v = projects(&s, &opts(&open, 100)); assert_eq!(v[0]["quiet"], true, "nothing on a live shelf: quiet, shown faded");
        let v = projects(&s, &opts(&open, 10_000_000)); assert!(v.is_empty(), "past Quiet.days it is Older, and the folded shelf hides it");
    }
    #[test] fn manual_keys_put_keyed_threads_first_and_a_key_ignores_trailing_zeros() {
        assert_eq!(key_cmp(&key_read("1"), &key_read("10")), std::cmp::Ordering::Equal); assert_eq!(key_cmp(&key_read("1"), &key_read("2")), std::cmp::Ordering::Less);
        let s = fold(&[PC, &tc("a", 11), &tc("b", 12), &tc("c", 13), r#"{"$":"ThreadOrdered","id":"a","section":"active","key":"8","at":14}"#, r#"{"$":"ThreadOrdered","id":"b","section":"active","key":"4","at":15}"#]);
        let open = HashSet::new(); let v = projects(&s, &opts(&open, 100));
        let ids: Vec<&str> = v[0]["threads"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect(); assert_eq!(ids, ["h|b", "h|a", "h|c"]);
    }
    #[test] fn a_child_thread_hangs_under_its_parent_when_the_parent_is_open() {
        let s = fold(&[PC, &tc("a", 11), &tc("b", 12), r#"{"$":"SettingSet","key":"thread.parent.b","value":"a","at":13}"#, r#"{"$":"SettingSet","key":"thread.tree.a","value":"1","at":14}"#]);
        let open = HashSet::new(); let v = projects(&s, &opts(&open, 100));
        let rows = v[0]["threads"].as_array().unwrap(); assert_eq!(rows.len(), 2); assert_eq!((rows[0]["id"].as_str().unwrap(), rows[0]["children"].as_u64().unwrap()), ("h|a", 1)); assert_eq!((rows[1]["id"].as_str().unwrap(), rows[1]["depth"].as_u64().unwrap()), ("h|b", 1));
    }
}

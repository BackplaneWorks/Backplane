//! The sidebar's `shown.projects` for the native clients (renderer `mobile`, locale English) from the Rust read model:
//! src/mobile/screen `Mob.projects`/`Mob.project`/`Mob.row`, core/client `Side.*`, core/subthreads `Tree.*`,
//! core/model `Sidebar.at`/`Archive.of`. code stays as the differential oracle (bp-policy on the same log).
//!
//! Covered: project order and live/quiet/older grouping, the five shelves with their order, thread tree rows (parents set by
//! `thread.parent.<id>`), status words, ago, lead and trail swipes, the Settled count, qualified ids (`<hub>|<id>`).
//! Not covered (listed by the comparison, never silent): manual order keys (ThreadOrdered/ProjectOrdered), threads of other
//! machines (far), bot and mail-run threads, Scratch, sync-off machines, other locales, search.
use crate::shelf::{shelf_at, Shelf};
use crate::signal;
use crate::state::{Override, State, Thread, Turn};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// Fuzzy.score > 0: every character of the query is in the title, in order, whatever the case (an empty query matches all). For project names: there are few and they are short.
pub fn fuzzy_hit(title: &str, q: &str) -> bool {
    let mut t = title.chars().flat_map(char::to_lowercase);
    q.chars().flat_map(char::to_lowercase).all(|c| t.any(|x| x == c))
}
/// Palette.hit: every word of the query is somewhere in the text, whatever the case. For thread titles: they are many and long, and letters in order would match most of them.
pub fn word_hit(text: &str, q: &str) -> bool { let t = text.to_lowercase(); q.to_lowercase().split_whitespace().all(|w| t.contains(w)) }
/// The Scratch project's id: conversations that belong to no project. Nothing starts there any more; its chats stay (Conversation history).
pub const SCRATCH: &str = "scratch";
/// How many Scratch chats the palette is told of, newest first.
const JUMP_MAX: usize = 200;

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
    /// the sidebar's search: "" for none (`word_hit` on threads, `fuzzy_hit` on projects)
    pub query: &'a str,
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

// ---- manual order keys: a binary fraction 0.b1b2b3..., zeros after its last digit ----------
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

// ---- the thread tree -------------------------------------------------------------------------------
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
        // an archived row (the Archived shelf) brings itself back
        3 if t.archived => vec![swipe("Unarchive", "row-unarchive", qid.clone(), "settle", vec![])],
        _ => vec![],
    };
    let snooze = vec![json!({"label": "1 hour", "value": format!("{qid}|1h")}), json!({"label": "Tomorrow", "value": format!("{qid}|tomorrow")}), json!({"label": "1 week", "value": format!("{qid}|1w")})];
    let trail = vec![swipe("Delete", "row-delete", qid.clone(), "danger", vec![]), swipe("Snooze", "row-snooze", String::new(), "", snooze)];
    let title = if t.title == "New thread" { "New thread".to_string() } else { t.title.clone() };
    json!({"depth": r.depth, "expanded": r.expanded, "children": r.children, "id": qid, "title": title, "state": state_word(t), "ago": when,
           "agents": agents_note(st, id), "pinned": t.pinned, "status": signal::display_name(st, ix[id], id), "want": want(st, id, signal::display_name(st, ix[id], id)), "canChild": !t.deleted, "lead": lead, "trail": trail})
}

/// Side.want (0.14): what a red row wants, said where its age goes: "approval" (a permission waits), "question" (an ask is open), "failed"
/// (its turn failed and nothing waits); "" for any other row. The same word ranks the palette's threads that wait on the person (Urg.rank).
pub fn want(st: &State, id: &str, status: &str) -> &'static str {
    match status {
        "failed" => "failed",
        "blocked" => {
            if st.ask_data.iter().any(|a| a.1 == id && a.2 == "approval") { "approval" }
            else if st.asks.values().any(|(th, open)| *open && th == id) { "question" }
            else if st.threads.get(id).map_or(false, |t| t.pending > 0) { "approval" } else { "" }
        }
        _ => "",
    }
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

/// Kids.set: delegated child threads live under their parent, not in the sidebar's roots, except a bot's (a bot's delegated work is a
/// thread of its own); a child with no visible parent stays reachable
fn kids_hidden<'a>(st: &'a State) -> HashSet<&'a str> {
    let bot_thread = |p: &str| st.bot_threads_has(p);
    st.tasks.values().filter(|(parent, child, _, _)| !bot_thread(parent) && st.threads.contains_key(child.as_str()) && !display_parent(st, child).is_empty()).map(|(_, c, _, _)| c.as_str()).collect()
}

struct Shelves<'a> { pinned: Vec<&'a str>, active: Vec<&'a str>, settled: Vec<&'a str>, hidden: Vec<&'a str>, snoozed: Vec<&'a str> }

fn shelves_of<'a>(st: &'a State, project: &str, now: u64, kids: &HashSet<&str>) -> Shelves<'a> {
    let mut ids: Vec<&str> = st.threads.iter().filter(|(i, t)| t.project == project && !kids.contains(i.as_str()) && !st.run_hidden(i)).map(|(i, _)| i.as_str()).collect();
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

struct Planned<'a> { id: &'a str, created: u64, latest: u64, key: Vec<bool>, sh: Shelves<'a> }

/// A borrowed sidebar plan: existing ordering, shelves and tree metadata, with no row JSON yet.
/// The caller pages its project identities first, then pages each visible list before rendering rows.
pub struct Plan<'a> {
    st: &'a State, o: &'a Opts<'a>, ix: HashMap<String,u8>, kids_by: HashMap<&'a str,Vec<&'a str>>,
    projects: Vec<Planned<'a>>, order: Vec<(usize,bool)>, older_n: usize,
}

pub fn plan<'a>(st: &'a State, o: &'a Opts<'a>) -> Plan<'a> {
    let ix = signal::index(st);
    let mut kids_by: HashMap<&str, Vec<&str>> = HashMap::new();
    {
        let mut all: Vec<(&str, &Thread)> = st.threads.iter().map(|(i, t)| (i.as_str(), t)).collect();
        all.sort_by(|a, b| tree_before(*a, *b));
        for (id, _) in all { let p = display_parent(st, id); if !p.is_empty() { kids_by.entry(p).or_default().push(id); } }
    }
    let kids = kids_hidden(st);
    let mut ps: Vec<Planned> = vec![];
    // a removed project keeps its row while its thread is the one open (the row goes when another is chosen)
    let open_project = st.threads.get(o.selected).map(|t| t.project.as_str()).unwrap_or("");
    for (id, p) in &st.projects {
        if p.removed && id != open_project { continue }
        // Side.projs.keep: the Scratch project (conversations that belong to no project) is not listed among projects
        if id.rsplit('~').next() == Some("scratch") && !id.contains('~') || id.split_once('~').map_or(false, |(_, l)| l == "scratch") { continue }
        let raw = shelves_of(st, id, o.now, &kids);
        // Side.latest: a thread's last move for its project's place; settling by hand counts
        let mut latest = p.created;
        for tid in st.threads.iter().filter(|(i, t)| t.project == *id && !kids.contains(i.as_str()) && !st.run_hidden(i)).map(|(i, _)| i) { let t = &st.threads[tid]; let touched = if t.override_ == Override::ForcedSettled { t.activity.max(t.settled_at) } else { t.activity }; latest = latest.max(touched); }
        ps.push(Planned { id: id.as_str(), created: p.created, latest, key: order_key(st, &format!("project:{id}"), "active"), sh: tree_shelves(st, &raw) });
    }
    // PSlot.before: keyed projects first, by key; then SRow.before. (`latest` is the group rank: live, quiet or older)
    ps.sort_by(|a, b| {
        let by_row = b.created.cmp(&a.created).then(a.id.cmp(b.id)); // SRow.before: the project's creation time, newest first, then id
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
    let mut order=vec![];
    for p in &live { order.push((ps.iter().position(|x| x.id==p.id).unwrap(),false)); }
    for p in &quiet { order.push((ps.iter().position(|x| x.id==p.id).unwrap(),true)); }
    // the project holding the open thread stays in the list even among the older ones (Side.cur.held)
    let held=st.threads.get(o.selected).map(|t|t.project.as_str()).unwrap_or("");
    for p in &older { if o.older_open || p.id==held { order.push((ps.iter().position(|x| x.id==p.id).unwrap(),true)); } }
    let older_n=older.iter().filter(|p|o.older_open || p.id!=held).count();
    Plan { st, o, ix, kids_by, projects:ps, order, older_n }
}

/// The Scratch chats (Conversation history), newest first, each with whether it wants attention (Legacy.live): the open one, one with an ask or a running turn,
/// or one still on the Pinned or Active shelf. The sidebar's group and the phones' `history.list` both read this, so they list the same chats.
pub fn scratch_chats<'a>(st: &'a State, now: u64, selected: &str) -> Vec<(&'a str, &'a Thread, bool)> {
    let mut all: Vec<(&str, &Thread)> = st.threads.iter().filter(|(i, t)| t.project == SCRATCH && !t.deleted && !t.archived && display_parent(st, i).is_empty() && !st.run_hidden(i)).map(|(i, t)| (i.as_str(), t)).collect();
    all.sort_by(|a, b| b.1.activity.cmp(&a.1.activity).then(a.0.cmp(b.0)));
    let wants = |id: &str, t: &Thread| { let until = st.snooze_until.get(id).copied().unwrap_or(0); id == selected || t.pending > 0 || t.turn == Turn::Running || matches!(shelf_at(t, until, now), Shelf::Pinned | Shelf::Active) };
    all.into_iter().map(|(id, t)| (id, t, wants(id, t))).collect()
}

impl Plan<'_> {
    /// The sidebar's search on a thread: its title, or any below it in the tree (the row that shows it).
    fn tree_hit(&self, id: &str) -> bool {
        let (mut stack, mut fuel) = (vec![id], 1 + self.st.threads.len());
        while let Some(x) = stack.pop() {
            if fuel == 0 { break } fuel -= 1;
            if self.st.threads.get(x).map_or(false, |t| word_hit(&t.title, self.o.query)) { return true }
            if let Some(k) = self.kids_by.get(x) { stack.extend(k.iter().copied()); }
        }
        false
    }
    /// Whether the search finds a thread of this project on any shelf, folded ones too.
    pub fn has_hit(&self, id: &str) -> bool {
        self.find(id).map_or(false, |(p, _)| [&p.sh.pinned, &p.sh.active, &p.sh.snoozed, &p.sh.settled, &p.sh.hidden].iter().any(|v| v.iter().any(|t| self.tree_hit(t))))
    }
    /// Scratch chats (Legacy.live, Legacy.history): `(rows, history)`. A chat that wants attention always has its row; the rest show when the group is open or a search finds them.
    /// `history` is the rest as `{id, title}`, newest first: what the group counts and the palette jumps to. No row starts a thread: only what exists is listed.
    pub fn legacy(&self, open: bool) -> (Vec<Value>, Vec<Value>) {
        let (st, o) = (self.st, self.o);
        let (mut rows, mut history) = (vec![], vec![]);
        for (id, t, live) in scratch_chats(st, o.now, o.selected) {
            if !live { if history.len() < JUMP_MAX { history.push(json!({"id": q(o, id), "title": t.title})); } }
            if !word_hit(&t.title, o.query) || !(live || open) { continue }
            let r = Row { id, t, depth: 0, expanded: false, children: self.kids_by.get(id).map_or(0, |k| k.len()) };
            let mut j = thread_json(st, o, &self.ix, &r, 0, 0); j["project"] = json!("Conversation history"); rows.push(j);
        }
        (rows, history)
    }
    /// How many projects are in the Older group (no live thread, quiet for more than QUIET_DAYS): drawn only once it is opened
    pub fn older_count(&self) -> usize { self.older_n }
    pub fn ids(&self) -> Vec<&str> { self.order.iter().map(|(i,_)|self.projects[*i].id).collect() }
    /// Where a listed project stands among other machines' projects (`merge_ordered`): its group (0 live, 1 quiet, 2 older), then its
    /// last move, newest first; None when it is not listed. (Within one machine its own order stands: manual keys, then creation.)
    pub fn place(&self, id: &str) -> Option<(u8, std::cmp::Reverse<u64>)> {
        let (p, _) = self.find(id)?;
        let live = !(p.sh.pinned.is_empty() && p.sh.active.is_empty() && p.sh.snoozed.is_empty());
        Some((if live { 0 } else if p.latest + QUIET_DAYS > self.o.now { 1 } else { 2 }, std::cmp::Reverse(p.latest)))
    }
    fn find(&self,id: &str) -> Option<(&Planned<'_>,bool)> {
        self.order.iter().find(|(i,_)|self.projects[*i].id==id).map(|(i,quiet)|(&self.projects[*i],*quiet))
    }
    pub fn list_count(&self,ids: &[&str]) -> usize {
        ids.iter().filter_map(|id|self.find(id)).map(|(p,_)| {
            usize::from(!p.sh.pinned.is_empty() || !p.sh.active.is_empty()) + usize::from(!p.sh.snoozed.is_empty())
                + usize::from(self.o.open.contains(p.id) && !p.sh.settled.is_empty())
                + usize::from(self.o.open.contains(&format!("arch:{}",p.id)) && !p.sh.hidden.is_empty())
        }).sum()
    }
    /// Build only the rows selected by `choose` (indices into each list of visible tree identities).
    /// The chooser owns paging, not order/group/tree semantics. A hidden shelf offers an empty list.
    pub fn project(&self,id: &str,choose: &mut dyn FnMut(&str,&[&str])->(Vec<usize>,Value)) -> Option<Value> {
        let (p,quiet)=self.find(id)?; let st=self.st; let o=self.o; let sh=&p.sh;
        let meta=&st.projects[id];
        // a search that only the project's threads answer shows those threads, wherever they stand; one its name answers shows the project as it is
        let narrow=!o.query.is_empty() && !fuzzy_hit(&meta.title,o.query);
        let mut live=sh.pinned.clone(); live.extend(sh.active.iter().copied());
        let mut arch: Vec<&str>=sh.hidden.iter().copied().filter(|id|st.threads[*id].archived && !st.threads[*id].deleted).collect();
        arch.sort_by(|a,b|rank(&st.threads[*b]).cmp(&rank(&st.threads[*a])));
        let mut project=json!({"machine":"","quiet":quiet,"id":q(o,id),"title":meta.title,"root":meta.root,"icon":"folder","open":o.open.contains(id),
            "snoozedShelf":"Snoozed","shelf":format!("Settled {}",sh.settled.len()),"settledCount":sh.settled.len(),"value":format!("{}|arch:{id}",o.hub),
            "archOpen":o.open.contains(&format!("arch:{id}")),"archivedCount":arch.len()});
        for (kind,page,roots,enabled) in [("threads","threadsPage",&live,true),("snoozed","snoozedPage",&sh.snoozed,true),
            ("settled","settledPage",&sh.settled,o.open.contains(id)),("archived","archivedPage",&arch,o.open.contains(&format!("arch:{id}")))] {
            let mut rows=if enabled || narrow { tree_rows(st,roots,o.selected,&self.kids_by) }else{vec![]};
            if narrow { rows.retain(|r|r.id==o.selected || self.tree_hit(r.id)); }
            if narrow && !rows.is_empty() { match kind { "settled"=>project["open"]=json!(true), "archived"=>project["archOpen"]=json!(true), _=>{} } }
            // a search counts what it found on a folded shelf, not the whole shelf
            if narrow { let n=rows.iter().filter(|r|r.depth==0).count(); match kind { "settled"=>{ project["settledCount"]=json!(n); project["shelf"]=json!(format!("Settled {n}")); }, "archived"=>project["archivedCount"]=json!(n), _=>{} } }
            let ids: Vec<&str>=rows.iter().map(|r|r.id).collect();
            let (indices,pager)=choose(kind,&ids);
            let values: Vec<Value>=indices.into_iter().filter_map(|i|rows.get(i)).map(|r| {
                let until=st.snooze_until.get(r.id).copied().unwrap_or(0);
                let shelf=match shelf_at(r.t,until,o.now) { Shelf::Settled=>1,Shelf::Snoozed=>2,Shelf::Hidden=>3,_=>0 };
                thread_json(st,o,&self.ix,r,shelf,until)
            }).collect();
            project[kind]=json!(values); project[page]=pager;
        }
        Some(project)
    }
}

impl Plan<'_> {
    /// Urg.display: how much a thread waits on the person (higher first): an approval, a question, a failure, a finished or stopped turn, work, idle; pinned ahead within each; a stall above all.
    fn urgency(&self, id: &str, t: &Thread) -> u64 {
        let pin = u64::from(t.pinned);
        if signal::stalled(self.st, id) { return 12 + pin }
        let n = self.ix.get(id).copied().unwrap_or(0);
        let word = signal::shown_k(signal::dot_name(signal::name(n), t.turn, t.pending), t.turn, t.pending);
        // a waiting approval (a permission or an approval ask: Side.want's word) above a question
        let rank = match word { "blocked" => if want(self.st, id, "blocked") == "approval" { 8 } else { 7 }, "failed" => 6, "finished" => 5, "stopped" => 4, "active" | "waiting" => 2, _ => 0 };
        rank * 2 + pin
    }
    /// The Active view: every thread on a live shelf of any project, what needs the person first, newest first within each;
    /// then the threads settled within QUIET_DAYS, newest settled first (faded, for bringing back). `keep(title, project)` is the search.
    /// Rows are the roots of the thread trees, as in the project list; each names its project.
    pub fn active(&self, keep: &dyn Fn(&str, &str) -> bool) -> (Vec<Value>, Vec<Value>) {
        let (l, f) = self.active_keyed(keep); (l.into_iter().map(|x| x.1).collect(), f.into_iter().map(|x| x.1).collect())
    }
    /// `active` with each row's sort key, so the rows of several machines go in one order (`ActiveKey`, smaller first).
    pub fn active_keyed(&self, keep: &dyn Fn(&str, &str) -> bool) -> (Vec<(ActiveKey, Value)>, Vec<(ActiveKey, Value)>) {
        let (st, o) = (self.st, self.o);
        let mut live: Vec<(u64, u64, &str, &str)> = vec![]; let mut faded: Vec<(u64, &str, &str)> = vec![];
        for p in &self.projects {
            let ptitle = st.projects[p.id].title.as_str();
            for id in p.sh.pinned.iter().chain(p.sh.active.iter()) { let t = &st.threads[*id]; if keep(&t.title, ptitle) { live.push((self.urgency(id, t), t.activity, id, p.id)); } }
            for id in &p.sh.settled { let t = &st.threads[*id]; if t.settled_at + QUIET_DAYS > o.now && keep(&t.title, ptitle) { faded.push((t.settled_at, id, p.id)); } }
        }
        live.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(b.2)));
        faded.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        let row = |id: &str, pid: &str, shelf: u8| {
            let t = &st.threads[id]; let kids = self.kids_by.get(id).map_or(0, |k| k.len());
            let r = Row { id, t, depth: 0, expanded: false, children: kids };
            let mut j = thread_json(st, o, &self.ix, &r, shelf, 0);
            j["project"] = json!(st.projects[pid].title); j["projectId"] = json!(q(o, pid)); j["faded"] = json!(shelf == 1);
            j
        };
        use std::cmp::Reverse;
        (live.iter().map(|(u, a, id, pid)| ((Reverse(*u), Reverse(*a)), row(id, pid, 0))).collect(), faded.iter().map(|(at, id, pid)| ((Reverse(0), Reverse(*at)), row(id, pid, 1))).collect())
    }
}

/// What a drag needs (`Ui.reorder`, `Ui.reorder_project`): for a thread, the shelf it stands on (`pinned` or `active`) and that shelf's threads in order;
/// for a project, "" and every project of the sidebar in order. None for a thread on no reorderable shelf.
pub fn order_of(st: &State, now: u64, kind: &str, id: &str) -> Option<(String, Vec<String>)> {
    if kind == "project" {
        let open: HashSet<String> = HashSet::new();
        let o = Opts { hub: "", now, selected: "", open: &open, older_open: true, query: "" };
        let p = plan(st, &o);
        return Some((String::new(), p.ids().into_iter().map(String::from).collect()));
    }
    let t = st.threads.get(id)?;
    let kids = kids_hidden(st);
    let sh = tree_shelves(st, &shelves_of(st, &t.project, now, &kids));
    for (name, list) in [("pinned", &sh.pinned), ("active", &sh.active)] {
        if list.iter().any(|x| *x == id) { return Some((name.to_string(), list.iter().map(|s| s.to_string()).collect())) }
    }
    None
}

/// The projects as the sidebar groups them: live, then quiet, then (when its shelf is open) the older ones.
/// The full projection remains the parity oracle for the bounded API.
pub fn projects(st: &State, o: &Opts) -> Vec<Value> {
    let plan=plan(st,o);
    plan.order.iter().map(|(i,quiet)| { let p=&plan.projects[*i]; project_json(st,o,&plan.ix,p.id,&p.sh,&plan.kids_by,*quiet) }).collect()
}

// ---- one project across machines ----------------------------------------------------------------------------------------
/// Where a thread's row stands among the rows of another machine on the same shelf, higher first: `threads` pinned then newest
/// (rank), `settled` by settle time, `snoozed` soonest to wake, `archived` newest.
pub fn row_key(st: &State, kind: &str, id: &str) -> i128 {
    let Some(t) = st.threads.get(id) else { return 0 };
    match kind {
        "threads" => (i128::from(t.pinned) << 64) + i128::from(rank(t)),
        "settled" => i128::from(t.settled_at),
        "snoozed" => -i128::from(st.snooze_until.get(id).copied().unwrap_or(0)),
        _ => i128::from(rank(t)),
    }
}
/// An Active row's place: what needs the person most, then the newest (its last move; a settled row's settle time).
pub type ActiveKey = (std::cmp::Reverse<u64>, std::cmp::Reverse<u64>);
/// Several machines' lists as one: each list keeps its own order, and at each step the head with the smallest key goes first (the
/// first list's on a tie). So projects and Active rows of every machine stand by recency together, and a manual order stays.
pub fn merge_ordered<T, K: Ord>(lists: Vec<Vec<T>>, key: &dyn Fn(&T) -> K) -> Vec<T> {
    let mut qs: Vec<std::collections::VecDeque<T>> = lists.into_iter().map(Into::into).collect();
    let mut out = vec![];
    loop {
        let mut best: Option<usize> = None;
        for (i, q) in qs.iter().enumerate() { if let Some(h) = q.front() { if best.map_or(true, |b| key(h) < key(qs[b].front().unwrap())) { best = Some(i) } } }
        match best { Some(i) => out.push(qs[i].pop_front().unwrap()), None => return out }
    }
}
/// Every thread row of another machine's project carries that machine's marker (`crate::mirror::mark_of`) and name; this machine's rows carry none.
pub fn label_rows(project: &mut Value, mark: &Value) {
    if mark.is_null() { return }
    for k in ["threads", "snoozed", "settled", "archived"] { if let Some(a) = project[k].as_array_mut() { for r in a { r["machine"] = mark["name"].clone(); r["mark"] = mark.clone(); } } }
}
/// What a project of this machine that is also on linked machines (one repository, `crate::mirror::unify`) says after its name, from
/// those machines' markers: "also on desktop", "also on desktop, framework". Empty for none.
pub fn also_label(marks: &[Value]) -> String {
    let names: Vec<&str> = marks.iter().filter_map(|m| m["short"].as_str().filter(|s| !s.is_empty())).collect();
    if names.is_empty() { String::new() } else { format!("also on {}", names.join(", ")) }
}
/// Two machines' rows of one shelf as one list: each machine's own order is kept (manual keys too) and a thread tree moves as one
/// block (its root and the rows under it); at each step the block whose root `key` is higher goes first, `a`'s on a tie.
pub fn merge_rows(a: Vec<Value>, b: Vec<Value>, key: &dyn Fn(&str) -> i128) -> Vec<Value> {
    fn blocks(v: Vec<Value>) -> std::collections::VecDeque<Vec<Value>> {
        let mut out: std::collections::VecDeque<Vec<Value>> = Default::default();
        for r in v { if r["depth"].as_u64().unwrap_or(0) == 0 || out.is_empty() { out.push_back(vec![r]) } else { out.back_mut().unwrap().push(r) } }
        out
    }
    let (mut x, mut y) = (blocks(a), blocks(b)); let mut out = vec![];
    let k = |bl: &Vec<Value>| key(bl[0]["id"].as_str().unwrap_or(""));
    loop {
        let take_b = match (x.front(), y.front()) { (None, None) => break, (Some(_), None) => false, (None, Some(_)) => true, (Some(p), Some(q)) => k(q) > k(p) };
        out.extend(if take_b { y.pop_front() } else { x.pop_front() }.unwrap());
    }
    out
}
/// A linked machine's project (`far`, its rows labelled) shown in this hub's project of the same repository: its rows merged into each
/// shelf (`key(kind, row id)`), its counts added. This hub's pagers stay (the linked rows are their own first page).
pub fn join_project(local: &mut Value, far: Value, key: &dyn Fn(&str, &str) -> i128) {
    for (kind, page) in [("threads", "threadsPage"), ("snoozed", "snoozedPage"), ("settled", "settledPage"), ("archived", "archivedPage")] {
        let a = local[kind].as_array().cloned().unwrap_or_default(); let b = far[kind].as_array().cloned().unwrap_or_default();
        local[kind] = json!(merge_rows(a, b, &|id| key(kind, id)));
        if local[page].is_null() && !far[page].is_null() { local[page] = far[page].clone(); }
    }
    for n in ["settledCount", "archivedCount"] { local[n] = json!(local[n].as_u64().unwrap_or(0) + far[n].as_u64().unwrap_or(0)); }
    local["shelf"] = json!(format!("Settled {}", local["settledCount"].as_u64().unwrap_or(0)));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fold(lines: &[&str]) -> State { let mut s = State::new(); for l in lines { s.apply_line(l); } s }
    const PC: &str = r#"{"$":"ProjectCreated","id":"p1","title":"proj","root":"/r","at":10}"#;
    fn tc(id: &str, at: u64) -> String { format!(r#"{{"$":"ThreadCreated","id":"{id}","project":"p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":{at}}}"#) }
    fn opts<'a>(open: &'a HashSet<String>, now: u64) -> Opts<'a> { Opts { hub: "h", now, selected: "", open, older_open: false, query: "" } }

    // Side.want: a red row says what it wants; the rest say nothing
    #[test] fn a_red_row_says_what_it_wants() {
        let turn = |th: &str, state: &str| format!(r#"{{"$":"TurnChanged","thread":"{th}","state":"{state}","at":20}}"#);
        let ask = |id: &str, th: &str, kind: &str| format!(r#"{{"$":"AskOpened","id":"{id}","thread":"{th}","kind":"{kind}","tool":"Bash","detail":"x","options":"","at":21}}"#);
        let s = fold(&[PC, &tc("t1", 11), &tc("t2", 12), &tc("t3", 13), &tc("t4", 14), &turn("t1", "failed"), &ask("a1", "t2", "approval"), &ask("a2", "t3", "question"), &turn("t4", "running")]);
        let ix = signal::index(&s); let w = |id: &str| want(&s, id, signal::display_name(&s, ix[id], id));
        assert_eq!([w("t1"), w("t2"), w("t3"), w("t4")], ["failed", "approval", "question", ""]);
        assert_eq!(want(&s, "t1", "finished"), "", "only a red row wants something");
    }
    // law kids_bot_child_shown: a bot's delegated child is never hidden under the bot
    #[test] fn kids_bot_child_shown() {
        let d = r#"{"$":"TaskDelegated","task":"k1","parent":"t1","child":"t2","depth":1,"provider":"claude","model":"","at":12}"#;
        let set = r#"{"$":"SettingSet","key":"thread.parent.t2","value":"t1","at":13}"#;
        let s = fold(&[PC, &tc("t1", 11), &tc("t2", 12), set, d]); assert!(kids_hidden(&s).contains("t2"), "a person's thread hides its delegated child under it");
        let s = fold(&[r#"{"$":"BotSet","id":"b1","name":"Bub","thread":"t1","look":0,"persona":"","at":9}"#, PC, &tc("t1", 11), &tc("t2", 12), set, d]); assert!(!kids_hidden(&s).contains("t2"), "a bot's child is shown");
    }
    #[test] fn two_machines_rows_merge_by_shelf_order_keeping_each_ones_order_and_trees() {
        let r = |id: &str, d: u64| json!({"id": id, "depth": d});
        let keys: HashMap<&str, i128> = [("a1", 9), ("a2", 3), ("b1", 5), ("b2", 1), ("a0", 2)].into_iter().collect();
        let key = |id: &str| keys.get(id).copied().unwrap_or(0);
        let out = merge_rows(vec![r("a1", 0), r("a1k", 1), r("a0", 0), r("a2", 0)], vec![r("b1", 0), r("b1k", 1), r("b2", 0)], &key);
        let ids: Vec<&str> = out.iter().map(|v| v["id"].as_str().unwrap()).collect();
        // a0 stands before a2 here (a manual key), so it stays before it; b1's child moves with b1
        assert_eq!(ids, ["a1", "a1k", "b1", "b1k", "a0", "a2", "b2"]);
        let mut local = json!({"threads": [r("a1", 0)], "settled": [], "snoozed": [], "archived": [], "settledCount": 2, "archivedCount": 0, "threadsPage": null, "settledPage": null, "snoozedPage": null, "archivedPage": null, "shelf": "Settled 2"});
        let mut far = json!({"threads": [r("b1", 0)], "settled": [r("b2", 0)], "snoozed": [], "archived": [], "settledCount": 3, "archivedCount": 1, "threadsPage": {"total": 1}, "settledPage": null, "snoozedPage": null, "archivedPage": null});
        label_rows(&mut far, &json!({"name":"framework","short":"framework","away":false,"label":"on framework","runs":"runs on framework"})); label_rows(&mut local, &Value::Null);
        assert_eq!(also_label(&[json!({"name":"desktop.tail.ts.net","short":"desktop"}), json!({"name":"framework-1","short":"framework-1","away":true})]), "also on desktop, framework-1");
        assert_eq!(also_label(&[]), "", "a project only here says nothing");
        join_project(&mut local, far, &|_, id| key(id));
        assert_eq!(local["threads"].as_array().unwrap().iter().map(|v| (v["id"].as_str().unwrap(), v["mark"]["short"].as_str())).collect::<Vec<_>>(), [("a1", None), ("b1", Some("framework"))], "this machine's rows look as before");
        assert_eq!((local["settledCount"].as_u64(), local["archivedCount"].as_u64(), local["shelf"].as_str()), (Some(5), Some(1), Some("Settled 5")));
        assert_eq!(local["settled"][0]["machine"], "framework"); assert_eq!(local["threadsPage"]["total"], 1);
    }
    #[test] fn ago_words() { assert_eq!(ago(100, 100), "now"); assert_eq!(ago(100, 40), "1m"); assert_eq!(ago(10000, 0), "2h"); assert_eq!(ago(200000, 0), "2d"); assert_eq!(ago(5, 9), "now"); }
    #[test] fn several_machines_lists_merge_by_key_keeping_each_ones_order() {
        // (a manual order on one machine stays: its 9 before its 1)
        let m = merge_ordered(vec![vec![(1, "a9"), (1, "a1"), (3, "a3")], vec![(0, "b0"), (2, "b2"), (3, "b3")], vec![]], &|x: &(u8, &str)| x.0);
        assert_eq!(m.iter().map(|x| x.1).collect::<Vec<_>>(), ["b0", "a9", "a1", "b2", "a3", "b3"]);
        assert!(merge_ordered::<u8, u8>(vec![vec![], vec![]], &|x| *x).is_empty());
    }
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
    #[test] fn a_search_counts_only_what_it_found_on_the_folded_shelves() {
        let mut s = fold(&[PC, &tc("a", 11), &tc("b", 12), &tc("c", 13), r#"{"$":"ThreadSettled","id":"b","manual":true,"at":14}"#, r#"{"$":"ThreadArchived","id":"c","archived":true,"at":15}"#]);
        s.threads.get_mut("a").unwrap().title = "Footprint review".into();
        let open = HashSet::new(); let o = Opts { hub: "h", now: 100, selected: "", open: &open, older_open: false, query: "footprint" };
        let plan = plan(&s, &o); let p = plan.project("p1", &mut |_, ids| ((0..ids.len()).collect(), Value::Null)).unwrap();
        assert_eq!((p["threads"].as_array().unwrap().len(), p["settledCount"].as_u64(), p["archivedCount"].as_u64()), (1, Some(0), Some(0)));
        let o2 = Opts { query: "", ..o }; let plan = super::plan(&s, &o2); let p = plan.project("p1", &mut |_, ids| ((0..ids.len()).collect(), Value::Null)).unwrap();
        assert_eq!((p["settledCount"].as_u64(), p["archivedCount"].as_u64()), (Some(1), Some(1)), "no search: the whole shelves");
    }
    #[test] fn the_active_view_puts_an_approval_ask_above_a_newer_question() {
        let ask = |id: &str, th: &str, kind: &str, at: u64| format!(r#"{{"$":"AskOpened","id":"{id}","thread":"{th}","kind":"{kind}","tool":"Bash","detail":"x","options":"","at":{at}}}"#);
        let turn = |th: &str, state: &str| format!(r#"{{"$":"TurnChanged","thread":"{th}","state":"{state}","at":20}}"#);
        let s = fold(&[PC, &tc("ap", 11), &tc("qu", 12), &tc("fa", 13), &turn("ap", "running"), &turn("qu", "running"), &turn("fa", "failed"), &ask("a1", "ap", "approval", 21), &ask("a2", "qu", "input", 22)]);
        let open = HashSet::new(); let o = opts(&open, 100); let p = plan(&s, &o);
        let (live, _) = p.active(&|_, _| true);
        assert_eq!(live.iter().map(|r| r["id"].as_str().unwrap()).collect::<Vec<_>>(), ["h|ap", "h|qu", "h|fa"]);
    }
    #[test] fn an_archived_row_says_so_and_offers_to_come_back() {
        let s = fold(&[PC, &tc("a", 11), &tc("b", 12), r#"{"$":"ThreadArchived","id":"a","archived":true,"at":13}"#]);
        let mut open = HashSet::new(); open.insert("arch:p1".to_string()); let v = projects(&s, &opts(&open, 100));
        let arch = v[0]["archived"].as_array().unwrap(); assert_eq!(arch.len(), 1);
        assert_eq!((arch[0]["lead"][0]["action"].as_str(), arch[0]["lead"][0]["value"].as_str()), (Some("row-unarchive"), Some("h|a")));
        let live = &v[0]["threads"][0]; assert_eq!((live["id"].as_str(), live["lead"][0]["action"].as_str()), (Some("h|b"), Some("row-settle")));
    }
    #[test] fn a_child_thread_hangs_under_its_parent_when_the_parent_is_open() {
        let s = fold(&[PC, &tc("a", 11), &tc("b", 12), r#"{"$":"SettingSet","key":"thread.parent.b","value":"a","at":13}"#, r#"{"$":"SettingSet","key":"thread.tree.a","value":"1","at":14}"#]);
        let open = HashSet::new(); let v = projects(&s, &opts(&open, 100));
        let rows = v[0]["threads"].as_array().unwrap(); assert_eq!(rows.len(), 2); assert_eq!((rows[0]["id"].as_str().unwrap(), rows[0]["children"].as_u64().unwrap()), ("h|a", 1)); assert_eq!((rows[1]["id"].as_str().unwrap(), rows[1]["depth"].as_u64().unwrap()), ("h|b", 1));
    }
}

#[cfg(test)]
mod bounded_tests {
    use super::*;
    #[test] fn bounded_sidebar_equals_full_slices_at_12k_thread_boundaries() {
        let mut st=State::new(); st.lean=true;
        let now=1_000_000;
        for i in 0..30 {
            let pid=format!("p{i:02}"); st.apply(&json!({"$":"ProjectCreated","id":pid,"title":pid,"root":"/synthetic","at":100+i}));
            if i%4==0 { st.apply(&json!({"$":"ProjectOrdered","id":pid,"key":if i%8==0{"4"}else{"8"},"at":600})); }
            for j in 0..400 {
                let id=format!("t{i:02}-{j:03}"); st.apply(&json!({"$":"ThreadCreated","id":id,"project":pid,"title":id,"at":200+j}));
                if i%3!=0 || j%7==0 { st.apply(&json!({"$":"ThreadSettled","id":id,"manual":true,"at":if i%3==1{now-100-j}else{1000+j}})); }
                if j%11==0 { st.apply(&json!({"$":"ThreadArchived","id":id,"archived":true,"at":600})); }
                if i%3==0 && j%9==0 { st.apply(&json!({"$":"ThreadPinned","id":id,"pinned":true,"at":600})); }
                if i%3==0 && j%13==0 { st.apply(&json!({"$":"ThreadSnoozed","id":id,"until":now+500-j,"at":600})); }
                if j%17==0 { st.apply(&json!({"$":"ThreadOrdered","id":id,"section":"active","key":"4","at":600})); }
                if j==2 { st.apply(&json!({"$":"SettingSet","key":format!("thread.parent.{id}"),"value":format!("t{i:02}-001"),"at":601})); }
                if j==1 { st.apply(&json!({"$":"SettingSet","key":format!("thread.tree.{id}"),"value":"1","at":601})); }
            }
        }
        assert_eq!(st.threads.len(),12_000);
        for older_open in [false,true] {
            let mut open=HashSet::new(); for i in 0..30 { open.insert(format!("p{i:02}")); open.insert(format!("arch:p{i:02}")); }
            let opts=Opts{hub:"h",now,selected:"",open:&open,older_open,query:""};
            let full=projects(&st,&opts); let plan=plan(&st,&opts); let ids=plan.ids();
            assert_eq!(ids.len(),if older_open{30}else{20});
            assert_eq!(ids,full.iter().map(|p|p["id"].as_str().unwrap().strip_prefix("h|").unwrap()).collect::<Vec<_>>());
            // Project offsets cross manual-sort and live/quiet/older boundaries; row offsets cross each shelf's ends.
            for start in [0,7,8,9,16,19,20,24,29] {
                for row_offset in [0,1,5,47,48,399,400] {
                    for id in ids.iter().skip(start).take(8) {
                        let actual=plan.project(id,&mut |_,rows|((row_offset..(row_offset+6).min(rows.len())).collect(),Value::Null)).unwrap();
                        let mut expected=full.iter().find(|p|p["id"]==format!("h|{id}")).unwrap().clone();
                        expected["settledCount"]=json!(expected["settled"].as_array().unwrap().iter().filter(|r|r["depth"]==0).count());
                        expected["archivedCount"]=json!(expected["archived"].as_array().unwrap().iter().filter(|r|r["depth"]==0).count());
                        for (field,page) in [("threads","threadsPage"),("snoozed","snoozedPage"),("settled","settledPage"),("archived","archivedPage")] {
                            expected[field]=json!(expected[field].as_array().unwrap().iter().skip(row_offset).take(6).cloned().collect::<Vec<_>>()); expected[page]=Value::Null;
                        }
                        assert_eq!(actual,expected,"older={older_open} project={id} project_offset={start} row_offset={row_offset}");
                    }
                }
            }
        }
    }
}

//! plane_read: the status plane as text, with the ids the thread tools take: what waits on the person, what runs, the bots,
//! the rooms with unread posts, the locks, the routines and the day's idle threads. The hub reads it with no client, so nothing here is far: every thread and bot is this
//! machine's. A pure function of the state lane's fold and the bots' world (`rooms` are read from the log by the caller).
use super::clock::Zones;
use super::sched_text::row;
use super::world::World;
use crate::shelf::{shelf_at, Shelf};
use crate::signal;
use crate::state::{Override, Project, State, Thread, Turn};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// a room with news: id, name, unread posts, when the last was written
pub struct RoomNews { pub id: String, pub name: String, pub unread: usize, pub at: u64 }
#[derive(Clone, Debug)]
struct Row { kind: &'static str, id: String, title: String, sub: String, state: String, at: u64 }

const IDLE_MAX: usize = 50;
const IDLE_AGE: u64 = 86400;
const ROUTINE_WINDOW: u64 = 900;
fn lines(s: &str) -> Vec<&str> { s.split('\n').filter(|l| !l.is_empty()).collect() }
fn field(l: &str, i: usize) -> &str { l.split('\t').nth(i).unwrap_or("") }
/// M.Setting.get(st, k, "")
fn set<'a>(st: &'a State, k: &str) -> &'a str { st.settings.get(k).map_or("", String::as_str) }
/// Thread.blank: what asking after an unknown thread finds
fn blank() -> Thread {
    Thread { project: String::new(), title: String::new(), created: 0, activity: 0, turn: Turn::Idle, pending: 0, archived: true, override_: Override::None, settled_at: 0, unsettled_at: 0, pinned: false, pinned_at: 0,
        worktree_mode: false, branch: String::new(), worktree: String::new(), provider: String::new(), model: String::new(), session: String::new(), deleted: true }
}

/// Plane.rank: how much a state waits on the person (0: not at all)
fn rank(s: &str) -> u8 { match s { "approval" => 8, "question" => 7, "failed" => 6, "stalled" => 5, _ => 0 } }
/// Plane.sec: the section a state goes to: 0 need, 1 run, 2 the rest
fn sec(s: &str) -> u8 { if rank(s) > 0 { 0 } else if s == "working" || s == "waiting" { 1 } else { 2 } }

struct Ctx<'a> { st: &'a State, ix: HashMap<String, u8>, asks: HashMap<&'a str, HashSet<&'a str>>, blank: Thread }
impl<'a> Ctx<'a> {
    fn thread(&self, id: &str) -> &Thread { self.st.threads.get(id).unwrap_or(&self.blank) }
    fn asked(&self, id: &str, kind: &str) -> bool { self.asks.get(id).map_or(false, |k| k.contains(kind)) }
    fn signal(&self, id: &str) -> u8 { self.ix.get(id).copied().unwrap_or(0) }
    /// the settings of a thread's rows (Bg.of, Work.watch)
    fn rows(&self, key: &str, th: &str) -> Vec<&str> { self.st.settings.get(&format!("{key}.{th}")).map_or(vec![], |v| lines(v)) }
    /// Plane.own.phase over Side.phase.ix: an open ask or a pending tool, a question, a failed turn
    fn own_phase(&self, id: &str) -> &'static str {
        let t = self.thread(id);
        if t.pending > 0 || self.asked(id, "approval") { "approval" }
        else if self.asked(id, "input") || self.asked(id, "plan") || self.asked(id, "browser") { "question" }
        else if t.turn == Turn::Failed { "failed" } else { "" }
    }
    /// Plane.own: what blocks a thread itself: its phase, a sign-in it waits for or a held message
    fn own(&self, id: &str) -> &'static str {
        let a = self.own_phase(id); if !a.is_empty() { return a }
        let held = self.st.queue.get(id).map_or(false, |q| q.iter().any(|(m, _)| m.starts_with("fallback:held:")));
        if held || !set(self.st, &format!("auth.wait.{id}")).is_empty() { "approval" } else { "" }
    }
    fn launched(&self, id: &str) -> Vec<String> { lines(set(self.st, &format!("launched.{id}"))).into_iter().map(str::to_string).collect() }
    /// the unfinished child tasks that are not launched threads (Plane.via.tasks, Plane.await.tasks)
    fn kids(&self, id: &str) -> Vec<String> {
        let launched = self.launched(id); let mut ts: Vec<_> = self.st.tasks.iter().filter(|(_, t)| t.0 == id && !t.2 && !launched.contains(&t.1)).collect();
        ts.sort_by(|a, b| a.0.cmp(b.0)); ts.into_iter().map(|(_, t)| t.1.clone()).collect()
    }
    /// Plane.via.src: the child task that blocks this thread, "" when every source is a row of its own
    fn via(&self, id: &str) -> String {
        let mut ids: std::collections::VecDeque<String> = self.kids(id).into(); let mut fuel = 64;
        while let (true, Some(c)) = (fuel > 0, ids.pop_front()) { fuel -= 1; if !self.own(&c).is_empty() { return c } ids.extend(self.kids(&c)) }
        String::new()
    }
    /// Plane.signal.free: not blocked: working, waiting on owned work, settled or idle
    fn free(&self, n: u8, t: &Thread) -> String {
        if signal::active(n) { return if t.turn == Turn::Running { "working" } else { "waiting" }.into() }
        match if signal::blocked(n) { 0 } else { n } { 4 => "settled", 5 => "finished", _ => "idle" }.into()
    }
    /// Plane.signal.current: "stalled", what blocks it (a child's state when only a child does), else `free`
    fn current(&self, id: &str) -> String {
        if signal::stalled(self.st, id) { return "stalled".into() }
        let (n, t) = (self.signal(id), self.thread(id));
        if !signal::blocked(n) { return self.free(n, t) }
        let own = self.own(id); if !own.is_empty() { return own.into() }
        let src = self.via(id); if src.is_empty() { self.free(n, t) } else { self.own(&src).into() }
    }
    /// Plane.asking: a thread whose open asks are all questions or plans is asking, though the hub counts every ask as pending
    fn asking(&self, s: String, id: &str) -> String { if s == "approval" && !self.asked(id, "approval") && (self.asked(id, "input") || self.asked(id, "plan")) { "question".into() } else { s } }
    fn state(&self, id: &str) -> String { self.asking(self.current(id), id) }
    /// Signal.display.note: a stalled thread says why
    fn note(&self, id: &str, fallback: &str) -> String {
        if !signal::stalled(self.st, id) { return fallback.to_string() }
        let why = self.st.settings.get(&format!("liveness.{id}")).and_then(|v| serde_json::from_str::<Value>(v).ok()).and_then(|v| v["reason"].as_str().map(str::to_string)).unwrap_or_default();
        format!("Stalled: {why}")
    }
    /// Plane.observed.sub: what the agent does, and what the thread still waits on
    fn observed(&self, sub: &str, th: &str) -> String {
        let doing = |r: &str| format!("{} ({}: {})", field(r, 0), field(r, 3), field(r, 1));
        let ended = |r: &str| field(r, 1).starts_with("\u{1}ended:");
        let confirmed = |r: &str| field(r, 1).starts_with("watching") || field(r, 1) == "cancelling";
        let mut work: Vec<String> = self.rows("bg", th).into_iter().filter(|r| !ended(r)).map(doing).collect();
        work.extend(self.rows("watch", th).into_iter().filter(|r| !ended(r) && confirmed(r)).map(doing));
        let mut kids = self.launched(th); kids.extend(self.kids(th));
        work.extend(kids.iter().filter(|c| signal::active(self.signal(c))).map(|c| format!("{c} ({})", self.thread(c).title)));
        let wakes: Vec<String> = lines(set(self.st, &format!("wakes.by.{th}"))).into_iter().map(|id| {
            let o: Value = serde_json::from_str(set(self.st, &format!("wake.{id}"))).unwrap_or(Value::Null);
            format!("{id} ({} {})", o["kind"].as_str().unwrap_or(""), o["target"].as_str().unwrap_or("")) }).collect();
        let part = |head: &str, xs: &[String]| if xs.is_empty() { String::new() } else { format!(" · {head}: {}", xs.join("; ")) };
        format!("{sub} · agent: {}{}{}", self.thread(th).turn.word(), part("work", &work), part("wake on", &wakes))
    }
    fn row(&self, kind: &'static str, id: &str, th: &str, title: &str, sub: &str, state: String) -> Row { Row { kind, id: id.into(), title: title.into(), sub: self.observed(&self.note(th, sub), th), state, at: self.thread(th).activity } }
    /// Mood.moving, Mood.note over Mood.bot: what a bot at work is doing, "" for one at rest. World.talked is keyed by a post's `from`, the bot's name, and talked_at asks by id,
    /// so cat never reads "talking" from it; this keeps that.
    fn mood(&self, th: &str) -> &'static str {
        let t = self.thread(th);
        if t.pending > 0 || self.asked(th, "approval") || self.asked(th, "input") || self.asked(th, "plan") || self.asked(th, "browser") { return "needs you" }
        if t.turn == Turn::Failed { return "" }
        let starting = self.st.queue.get(th).map_or(false, |q| q.iter().any(|(m, _)| !m.starts_with("handoff:"))) && t.worktree_mode && t.worktree.is_empty();
        let out = !self.rows("bg", th).is_empty() || !self.rows("watch", th).is_empty() || !set(self.st, &format!("order.wait.{th}")).is_empty();
        if t.turn == Turn::Running || starting || out { "working" } else { "" }
    }
}

/// Tree.display_parent: a link to a live thread of the same project, on the same side of the archive
fn parent<'a>(st: &'a State, id: &str) -> Option<&'a str> {
    let p = st.settings.get(&format!("thread.parent.{id}")).map(String::as_str).filter(|p| !p.is_empty() && *p != id)?;
    let (a, b) = (st.threads.get(id)?, st.threads.get(p)?);
    (!a.deleted && !b.deleted && a.project == b.project && a.archived == b.archived).then_some(p)
}
/// Tree.* over Side.sorted (sidebar.rs keeps its own to itself): the threads on a project's pinned and active shelves, as roots of their trees
fn live_threads<'a>(st: &'a State, project: &str, now: u64, hidden: &HashSet<&str>) -> Vec<&'a str> {
    let root = |id: &'a str| -> &'a str { let mut cur = id; for _ in 0..64 { match parent(st, cur) { Some(p) if p != id => cur = p, _ => break } } cur };
    let mut ids: Vec<&'a str> = st.threads.iter().filter(|(i, t)| t.project == project && !hidden.contains(i.as_str()) && !st.run_hidden(i)).map(|(i, _)| i.as_str()).collect(); ids.sort();
    let mut seen: HashSet<&str> = HashSet::new(); let mut out = vec![];
    for id in ids {
        let t = &st.threads[id]; let until = st.snooze_until.get(id).copied().unwrap_or(0);
        if t.deleted || !matches!(shelf_at(t, until, now), Shelf::Pinned | Shelf::Active) { continue }
        let r = root(id); if seen.insert(r) { out.push(r) }
    }
    out
}
/// M.Time.ago: "now", "5m", "3h", "9d"
fn ago_short(now: u64, at: u64) -> String { let s = now.saturating_sub(at); if s < 60 { "now".into() } else if s < 3600 { format!("{}m", s / 60) } else if s < 86400 { format!("{}h", s / 3600) } else { format!("{}d", s / 86400) } }
/// Plane.text.when: the last activity of a row, or when a routine is next due
fn when(kind: &str, at: u64, now: u64) -> String {
    if at == 0 { String::new() } else if kind == "routine" { if at > now { format!("next in {}", ago_short(at, now)) } else { "due now".into() } }
    else if now.saturating_sub(at) < 60 { "just now".into() } else { format!("{} ago", ago_short(now, at)) }
}
fn part(s: &str) -> String { if s.is_empty() { String::new() } else { format!(" · {s}") } }
fn line(r: &Row, now: u64) -> String { format!("- {} · {} · {}{}{}\n", r.id, r.state, r.title, part(&r.sub), part(&when(r.kind, r.at, now))) }
fn section(head: &str, rs: &[Row], now: u64) -> String { if rs.is_empty() { String::new() } else { format!("{head} ({})\n{}", rs.len(), rs.iter().map(|r| line(r, now)).collect::<String>()) } }

/// Plane.lock.row: who holds each named lock
fn locks(c: &Ctx, now: u64) -> Vec<Row> {
    let mut out = vec![];
    for name in lines(set(c.st, "locks.names")) {
        let l: Value = serde_json::from_str(set(c.st, &format!("lock.{name}"))).unwrap_or(Value::Null);
        let h = &l["holder"]; if !h.is_object() { continue }
        let g = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
        let th = g(h, "thread"); let title = if th.is_empty() { g(h, "id") } else { c.thread(&th).title.clone() };
        let who = if l["outside"].is_object() { format!("held outside the hub by pid {} ({})", l["outside"]["pid"], g(&l["outside"], "cmd")) } else { format!("held by {title} {}", ago_short(now, h["since"].as_u64().unwrap_or(0))) };
        let waiting = l["waiters"].as_array().map_or(0, Vec::len);
        out.push(Row { kind: "lock", id: name.into(), title: format!("{name} · {who} · {waiting} waiting"), sub: g(h, "note"), state: if l["ready"].as_bool().unwrap_or(false) { "working" } else { "waiting" }.into(), at: 0 });
    }
    out
}
/// Deploy.blocking: what the restart waits for: n tool calls running, the build lock held
fn blocking(n: usize, locked: bool) -> String {
    if n == 0 { return if locked { "waiting for the build lock" } else { "waiting for a safe point" }.into() }
    format!("waiting for {n} tool call{}{}", if n == 1 { "" } else { "s" }, if locked { " and the build lock" } else { "" })
}
/// Plane.deploy.k: a restart waiting for a safe point, one row at the top of what runs
fn deploy(c: &Ctx) -> Vec<Row> {
    let line = set(c.st, "deploy.wait"); if line.is_empty() { return vec![] }
    let o: Value = serde_json::from_str(line).unwrap_or(Value::Null);
    let open = o["open"].as_str().unwrap_or(""); let n = if open.is_empty() { 0 } else { open.split(',').count() };
    let since = o["since"].as_str().and_then(|x| x.parse().ok()).unwrap_or(0);
    vec![Row { kind: "deploy", id: "deploy".into(), title: "Restart pending".into(), sub: format!("{} · Cancel", blocking(n, o["locked"].as_bool().unwrap_or(false))), state: "waiting".into(), at: since }]
}

/// Plane.read: the plane as an agent reads it. `idle` lists the day's idle threads, else their count.
pub fn read(st: &State, w: &World, rooms: &[RoomNews], now: u64, idle: bool) -> String {
    let mut asks: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (th, kind, open) in w.asks.values() { if *open { asks.entry(th.as_str()).or_default().insert(kind.as_str()); } }
    let c = Ctx { st, ix: signal::index(st), asks, blank: blank() };
    // Plane.of.f: the live threads of every project, then the bots' own threads that wait on the person or run
    let bot_thread = |p: &str| st.bot_threads_has(p);
    let hidden: HashSet<&str> = st.tasks.values().filter(|t| !bot_thread(&t.0) && st.threads.contains_key(t.1.as_str()) && parent(st, &t.1).is_some()).map(|t| t.1.as_str()).collect();
    let mut all: Vec<Row> = vec![];
    let mut projects: Vec<(&String, &Project)> = st.projects.iter().filter(|(id, p)| !p.removed && id.as_str() != "scratch").collect(); projects.sort_by(|a, b| b.1.created.cmp(&a.1.created).then(a.0.cmp(b.0)));
    for (pid, p) in projects {
        for id in live_threads(st, pid, now, &hidden) {
            let (state, t) = (c.state(id), c.thread(id));
            if sec(&state) < 2 || now <= t.activity + IDLE_AGE { all.push(c.row("thread", id, id, &t.title, &p.title, state)) } // Plane.shows
        }
    }
    let mut bots: Vec<_> = w.bots.values().filter(|b| !b.deleted).collect(); bots.sort_by_key(|b| (b.created, b.id.rsplit('-').next().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0), b.id.clone()));
    for b in bots.iter().filter(|b| !b.thread.is_empty()) { let s = c.state(&b.thread); if sec(&s) <= 1 { all.push(c.row("bot", &b.id, &b.thread, &b.name, "", s)) } }
    let by_part = |n: u8| -> Vec<Row> { all.iter().filter(|r| sec(&r.state) == n).cloned().collect() };
    // Plane.before: most urgent first, the newer first among equals
    let urgent = |mut v: Vec<Row>| { v.sort_by(|a, b| rank(&b.state).cmp(&rank(&a.state)).then(b.at.cmp(&a.at))); v };
    let newer = |mut v: Vec<Row>| { v.sort_by(|a, b| b.at.cmp(&a.at)); v };
    let need = urgent(by_part(0)); let mut run = deploy(&c); run.extend(newer(by_part(1)));
    let rest: Vec<Row> = by_part(2).into_iter().filter(|r| now <= r.at + IDLE_AGE).collect(); let rest = urgent(rest);
    // BU.rows: the bots here but the uberbot, each with what it does now
    let cats: Vec<Row> = bots.iter().filter(|b| !w.uber_is(&b.id)).map(|b| { let mut r = c.row("bot", &b.id, &b.thread, &b.name, c.mood(&b.thread), c.current(&b.thread)); if b.thread.is_empty() { r.at = 0 } r }).collect();
    let mut news: Vec<Row> = rooms.iter().filter(|r| r.unread > 0).map(|r| Row { kind: "room", id: r.id.clone(), title: r.name.clone(), sub: format!("{} unread", r.unread), state: "unread".into(), at: r.at }).collect();
    news.sort_by(|a, b| b.at.cmp(&a.at));
    // Plane.routine.t: those on and not done; a busy bot's routine that fired within the window is running
    let z = Zones { settings: &w.settings };
    let mut rs: Vec<Row> = w.routines.values().filter(|r| !r.deleted && r.on && !r.done).map(|r| {
        let s = row(&z, r, now); let th = w.bot(&r.bot).map_or(String::new(), |b| b.thread.clone());
        let run = signal::active(c.signal(&th)) && r.last > 0 && now <= r.last + ROUTINE_WINDOW;
        let sub = format!("{}{}", s.when, if s.next_text.is_empty() || s.kind != "routine" { String::new() } else { format!(" · {}", s.next_text) });
        Row { kind: "routine", id: r.id.clone(), title: r.name.clone(), sub, state: if run { "working" } else if r.kind == "remind" { "reminder" } else { "idle" }.into(), at: s.next }
    }).collect();
    rs.sort_by(|a, b| a.id.cmp(&b.id));
    // Plane.due.before: a routine running first, then the soonest due (never, 0, last)
    rs.sort_by(|a, b| (a.state != "working").cmp(&(b.state != "working")).then((a.at == 0).cmp(&(b.at == 0))).then(a.at.cmp(&b.at)));
    let shown: Vec<Row> = rest.iter().take(IDLE_MAX).cloned().collect();
    let idle_text = if idle { section("Idle in the last day", &shown, now) } else if rest.is_empty() { String::new() } else { format!("Idle in the last day: {} (plane_read with idle: true lists them).\n", rest.len()) };
    format!("Status plane of this machine\n{}{}{}{}{}{}{}{}", if need.is_empty() && run.is_empty() { "Nothing needs you and nothing is running.\n" } else { "" },
        section("Needs you", &need, now), section("Running", &run, now), section("Bots", &cats, now), section("Rooms with unread posts", &news, now), section("Locks", &locks(&c, now), now), section("Routines", &rs, now), idle_text)
}

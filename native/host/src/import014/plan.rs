//! What of a 0.14 log comes across, decided from the two logs alone (no hub, no files): `plan` reads the old `events.jsonl` up to a size and
//! answers the lines to write into the new home, the counts a person is shown and the notes on what stays behind.
//!
//! The format is the same in both versions (store.rs), so a line that comes across is the old line itself, cleaned as the store reads it. The few
//! that change are rebuilt the same way on every run (a routine switched off, a task that was running marked interrupted, the last value of a
//! setting). That makes a re-run exact: a line already in the new log (`Have::lines`, by hash of its text) is never written again, so a second
//! import adds nothing, an import cut off part way finishes where it stopped, and one made after 0.14 kept working adds only what 0.14 added.
//!
//! Kept: projects (not removed), the person's threads in them and the bots' own chats, with their messages, activity, settle, archive, worktree,
//! session, model and todo history; subthread links and the tasks between imported threads; bots with their memory, space and routines (switched
//! off); rooms with their posts; the uberbot when this home has none; phone push registrations; the settings that still mean something here.
//! Left behind, each said in a note: the mailroom's own run threads and mail, turn and queue state, asks, wakes, locks, usage meters, machine
//! links, deploys, removed projects and deleted threads.
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// A line's identity for the re-run check: a hash of its text as the store keeps it.
pub fn line_hash(l: &str) -> u64 { let mut h = DefaultHasher::new(); l.hash(&mut h); h.finish() }

/// What the new home holds already.
#[derive(Default)]
pub struct Have {
    pub lines: HashSet<u64>,
    /// setting keys with a value
    pub settings: HashSet<String>,
    /// ids of projects, threads, bots, rooms and routines, each with the hash of the line that made it
    pub made: HashMap<String, HashSet<u64>>,
}
impl Have {
    pub fn add(&mut self, line: &str) {
        let h = line_hash(line); self.lines.insert(h);
        match crate::session::event_type(line) {
            Some("SettingSet") => if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                let (k, val) = (v["key"].as_str().unwrap_or(""), v["value"].as_str().unwrap_or(""));
                if val.is_empty() { self.settings.remove(k); } else { self.settings.insert(k.to_string()); }
            },
            Some("UberSet") => { self.settings.insert("uber.bot".into()); }
            Some("ProjectCreated" | "ThreadCreated" | "BotSet" | "RoomSet" | "RoutineSet") => { let id = fld(line, "id"); if !id.is_empty() { self.made.entry(id.to_string()).or_default().insert(h); } }
            _ => {}
        }
    }
    /// An id this home already uses for something the import did not bring (a clash): that one stays out.
    fn clash(&self, id: &str, made_by: &[u64]) -> bool { self.made.get(id).map_or(false, |hs| !made_by.iter().any(|h| hs.contains(h))) }
}

#[derive(Default, Clone, Debug, PartialEq, serde::Serialize)]
pub struct Counts {
    pub projects: usize, pub threads: usize, pub messages: usize, pub bots: usize, pub memories: usize, pub rooms: usize, pub posts: usize,
    pub routines: usize, pub settings: usize,
    /// paired devices (set by the run from devices-v1.json), phone push registrations (from the log)
    pub devices: usize, pub pushes: usize, pub lines: usize,
    /// left behind: the mailroom's run threads, deleted threads, removed projects, lines already here, ids in use here for something else
    pub runs: usize, pub deleted: usize, pub removed: usize, pub already: usize, pub clashes: usize,
}

pub struct Plan {
    pub lines: Vec<String>, pub counts: Counts,
    /// the imported bots: (id, name); their webhooks when keys come too
    pub bots: Vec<(String, String)>, pub hooks: Vec<String>,
    /// whether the old log holds routines that were on (they arrive off), mail runs, machine links
    pub routines_on: usize, pub peers: usize,
    /// the size of the old log read (whole lines up to it)
    pub read: u64,
}

/// Opts: `secrets` (the person said keys may come: webhooks are kept only then), `keep` (false: count only, hold no lines).
#[derive(Clone, Copy, Default)]
pub struct Opts { pub secrets: bool, pub keep: bool }

/// The first `"key":"value"` of a line. Every id field 0.14 writes comes before any free text in its event and holds no escape.
pub fn fld<'a>(line: &'a str, key: &str) -> &'a str {
    let pat = format!("\"{key}\":\""); let Some(i) = line.find(&pat) else { return "" };
    let r = &line[i + pat.len()..]; &r[..r.find('"').unwrap_or(0)]
}

/// Events about one thread, by the field that names it.
fn thread_key(t: &str) -> Option<&'static str> {
    Some(match t {
        "ThreadCreated" | "ThreadRenamed" | "ThreadSettled" | "ThreadUnsettled" | "ThreadArchived" | "ThreadPinned" | "ThreadSnoozed" | "ThreadOrdered" | "WorktreeSet" | "WorktreeRemoved"
        | "WorktreeSkipped" | "SessionSet" | "ModesSet" | "ProviderSet" | "ThreadMerged" | "ThreadParentSet" | "ThreadLaunched" | "ThreadForked" => "id",
        "MessagePosted" | "ActivityLogged" | "MessageFrom" | "TodosSet" | "TodosChanged" => "thread",
        _ => return None,
    })
}
/// The mailroom's own working threads (mail runs and coordinator back offices): never a person's thread.
fn run_thread(id: &str) -> bool { id.starts_with("run") || id.starts_with("mailcoord-") }

/// Settings that still mean something in this version: a person's choices, and notes about the things that come across.
fn keeps_setting(k: &str) -> bool {
    const EXACT: &[&str] = &["agent.provider", "agent.model", "agent.lang", "ui.theme", "sidebar.view", "voice.device", "notify.all", "followup", "restart.continue",
        "text.provider", "text.model", "order.enabled", "kicad.ipc", "kicad.path", "freecad.path"];
    const PREFIX: &[&str] = &["ui.", "sidebar.", "notify.", "project.icon.", "fab.", "thread.tree.", "bot.parent.", "room.seen.", "seen.", "summary.", "effort.", "turn.speed.", "run.model.", "board.", "rule.", "merged."];
    EXACT.contains(&k) || PREFIX.iter().any(|p| k.starts_with(p))
}
/// The ids a setting key names (`summary.at.t17-2`, `seen.t17-2.claude`): tokens that look like a thread, project, bot or room id.
fn ids_in(k: &str) -> Vec<&str> {
    k.split(['.', ':']).filter(|t| { let b = t.as_bytes(); b.len() > 3 && (matches!(b[0], b't' | b'p' | b'b' | b'r') && b[1].is_ascii_digit() || t.starts_with("run") && b[3].is_ascii_digit() || t.starts_with("mailcoord-")) && t.contains('-') }).collect()
}

/// Whole lines of `path` up to `upto` bytes, cleaned as the store reads them; a torn tail is not a line.
fn each_line(path: &Path, upto: u64, mut f: impl FnMut(&str)) -> std::io::Result<()> { each_line_at(path, upto, &|_| {}, &mut f) }
/// `each_line`, telling `at` how many bytes it has read, about every 4 MB.
fn each_line_at(path: &Path, upto: u64, at: &dyn Fn(u64), f: &mut dyn FnMut(&str)) -> std::io::Result<()> {
    let r = BufReader::with_capacity(1 << 20, std::fs::File::open(path)?.take(upto));
    let (mut read, mut told) = (0u64, 0u64);
    for l in r.split(b'\n') {
        let l = l?; read += l.len() as u64 + 1; if read - told >= 4 << 20 { told = read; at(read) }
        if l.is_empty() { continue }
        let Ok(s) = std::str::from_utf8(&l) else { continue };
        let s = s.strip_suffix('\r').unwrap_or(s);
        f(&crate::store::clean(s));
    }
    Ok(())
}
/// The old log's size now, cut back to its last newline (0.14 may be writing it).
pub fn whole_size(path: &Path) -> std::io::Result<u64> {
    use std::io::{Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?; let n = f.metadata()?.len(); if n == 0 { return Ok(0) }
    let mut back = 0u64; let mut buf = [0u8; 4096];
    while back < n {
        let step = (n - back).min(buf.len() as u64); f.seek(SeekFrom::Start(n - back - step))?; f.read_exact(&mut buf[..step as usize])?;
        if let Some(i) = buf[..step as usize].iter().rposition(|b| *b == b'\n') { return Ok(n - back - step + i as u64 + 1) }
        back += step;
    }
    Ok(0)
}

pub fn plan(log: &Path, have: &Have, o: Opts) -> std::io::Result<Plan> { plan_with(log, have, o, &|_, _| {}) }
/// `plan`, telling `read(bytes, of)` how far it is: three passes over the old log, so `of` is three times its size.
pub fn plan_with(log: &Path, have: &Have, o: Opts, read: &dyn Fn(u64, u64)) -> std::io::Result<Plan> {
    let upto = whole_size(log)?; let of = upto * 3;
    // ---- first pass: what exists at the end of the old log ----
    let (mut projects, mut removed) = (HashMap::<String, u64>::new(), HashSet::<String>::new());
    let mut threads: HashMap<String, (String, u64)> = HashMap::new(); // id -> (project, hash of ThreadCreated)
    let (mut deleted, mut runs) = (HashSet::<String>::new(), HashSet::<String>::new());
    let mut bots: HashMap<String, (String, u64)> = HashMap::new(); let mut bot_threads: HashMap<String, String> = HashMap::new(); let mut bots_gone = HashSet::new();
    let (mut rooms, mut rooms_gone) = (HashMap::<String, u64>::new(), HashSet::<String>::new());
    let (mut routines, mut routines_gone) = (HashMap::<String, String>::new(), HashSet::<String>::new());
    let mut tasks: HashMap<String, (String, String)> = HashMap::new(); let mut task_last: HashMap<String, (String, u64)> = HashMap::new();
    let mut devices: HashMap<String, bool> = HashMap::new(); // token -> live at the end
    let mut settings: HashMap<String, (String, u64)> = HashMap::new();
    let mut uber: Option<String> = None; let mut peers = 0usize; let mut hooks: HashMap<String, String> = HashMap::new();
    each_line_at(log, upto, &|b| read(b, of), &mut |l: &str| {
        let Some(t) = crate::session::event_type(l) else { return };
        let h = || line_hash(l);
        match t {
            "ProjectCreated" => { projects.insert(fld(l, "id").into(), h()); }
            "ProjectRemoved" => { removed.insert(fld(l, "id").into()); }
            "ThreadCreated" => { threads.insert(fld(l, "id").into(), (fld(l, "project").into(), h())); }
            "ThreadDeleted" => { deleted.insert(fld(l, "id").into()); }
            "RunSet" => { runs.insert(fld(l, "thread").into()); }
            "BotSet" => { let id = fld(l, "id"); let th = fld(l, "thread"); if !th.is_empty() { bot_threads.insert(th.into(), id.into()); } bots.entry(id.into()).or_insert((fld(l, "name").into(), h())).0 = fld(l, "name").into(); bots_gone.remove(id); }
            "BotDeleted" => { bots_gone.insert(fld(l, "id").to_string()); }
            "RoomSet" => { rooms.entry(fld(l, "id").into()).or_insert(h()); }
            "RoomDeleted" => { rooms_gone.insert(fld(l, "id").to_string()); }
            "RoutineSet" => { routines.insert(fld(l, "id").into(), fld(l, "bot").into()); }
            "RoutineDeleted" => { routines_gone.insert(fld(l, "id").to_string()); }
            "TaskDelegated" => { tasks.insert(fld(l, "task").into(), (fld(l, "parent").into(), fld(l, "child").into())); }
            "TaskStateSet" => { let at = serde_json::from_str::<serde_json::Value>(l).ok().and_then(|v| v["at"].as_u64()).unwrap_or(0); task_last.insert(fld(l, "task").into(), (fld(l, "state").into(), at)); }
            "DeviceRegistered" => { devices.insert(fld(l, "token").into(), true); }
            "DeviceDropped" => { devices.insert(fld(l, "token").into(), false); }
            "HookSet" => { hooks.insert(fld(l, "id").into(), fld(l, "bot").into()); }
            "PeerSet" => { peers += 1; }
            "UberSet" => { uber = Some(fld(l, "bot").into()); }
            "SettingSet" => if let Ok(v) = serde_json::from_str::<serde_json::Value>(l) { settings.insert(v["key"].as_str().unwrap_or("").into(), (v["value"].as_str().unwrap_or("").into(), v["at"].as_u64().unwrap_or(0))); },
            _ => {}
        }
    })?;
    let mut c = Counts::default();
    // projects, bots, rooms that come: not removed, not an id used here for something else
    let pk: HashSet<String> = projects.iter().filter(|(id, h)| { if removed.contains(*id) { c.removed += 1; return false } if have.clash(id, &[**h]) { c.clashes += 1; return false } true }).map(|(id, _)| id.clone()).collect();
    let bk: HashSet<String> = bots.iter().filter(|(id, (_, h))| !bots_gone.contains(*id) && !have.clash(id, &[*h])).map(|(id, _)| id.clone()).collect();
    let rk: HashSet<String> = rooms.iter().filter(|(id, h)| !rooms_gone.contains(*id) && !have.clash(id, &[**h])).map(|(id, _)| id.clone()).collect();
    let tk: HashSet<String> = threads.iter().filter(|(id, (proj, h))| {
        if deleted.contains(*id) { c.deleted += 1; return false }
        if runs.contains(*id) || run_thread(id) { c.runs += 1; return false }
        let bot_chat = bot_threads.get(*id).map_or(false, |b| bk.contains(b));
        if !(pk.contains(proj) || bot_chat) { if removed.contains(proj) { c.deleted += 1 } return false }
        if have.clash(id, &[*h]) { c.clashes += 1; return false }
        true
    }).map(|(id, _)| id.clone()).collect();
    let kk: HashSet<String> = tasks.iter().filter(|(_, (p, ch))| tk.contains(p) && tk.contains(ch)).map(|(k, _)| k.clone()).collect();
    let ok_routine = |id: &str| !routines_gone.contains(id) && routines.get(id).map_or(false, |b| bk.contains(b));
    let hk: HashSet<String> = if o.secrets { hooks.iter().filter(|(_, b)| bk.contains(*b)).map(|(id, _)| id.clone()).collect() } else { HashSet::new() };
    let mut last_device: HashMap<String, u64> = HashMap::new(); // token -> hash of its last DeviceRegistered/DevicePrefs line (only those come)
    each_line_at(log, upto, &|b| read(upto + b, of), &mut |l: &str| { match crate::session::event_type(l) { Some("DeviceRegistered") | Some("DevicePrefs") => { let tok = fld(l, "token"); if devices.get(tok) == Some(&true) { last_device.insert(format!("{}{tok}", crate::session::event_type(l).unwrap_or("")), line_hash(l)); } } _ => {} } })?;
    let wanted_devices: HashSet<u64> = last_device.values().copied().collect();

    // ---- second pass: the lines, in the old log's order ----
    let mut out: Vec<String> = vec![]; let mut routines_on: HashSet<String> = HashSet::new();
    let mut emit = |line: String, c: &mut Counts, what: Option<&str>| {
        if have.lines.contains(&line_hash(&line)) { c.already += 1; return }
        match what { Some("project") => c.projects += 1, Some("thread") => c.threads += 1, Some("msg") => c.messages += 1, Some("bot") => c.bots += 1, Some("memory") => c.memories += 1,
            Some("room") => c.rooms += 1, Some("post") => c.posts += 1, Some("routine") => c.routines += 1, Some("push") => c.pushes += 1, Some("setting") => c.settings += 1, _ => {} }
        c.lines += 1;
        if o.keep { out.push(line) }
    };
    let mut seen_bot: HashSet<String> = HashSet::new(); let mut seen_room: HashSet<String> = HashSet::new(); let mut seen_routine: HashSet<String> = HashSet::new();
    each_line_at(log, upto, &|b| read(2 * upto + b, of), &mut |l: &str| {
        let Some(t) = crate::session::event_type(l) else { return };
        if let Some(key) = thread_key(t) {
            let th = fld(l, key); if !tk.contains(th) { return }
            let other = match t { "ThreadParentSet" => fld(l, "parent"), "ThreadLaunched" => fld(l, "by"), "ThreadForked" => fld(l, "source"), _ => "" };
            if !other.is_empty() && !tk.contains(other) { return }
            emit(l.to_string(), &mut c, match t { "ThreadCreated" => Some("thread"), "MessagePosted" => Some("msg"), _ => None });
            return
        }
        match t {
            "ProjectCreated" | "ProjectOrdered" if pk.contains(fld(l, "id")) => emit(l.to_string(), &mut c, (t == "ProjectCreated").then_some("project")),
            "TaskDelegated" | "TaskStateSet" | "TaskDelivered" if kk.contains(fld(l, "task")) => emit(l.to_string(), &mut c, None),
            "BotSet" if bk.contains(fld(l, "id")) => { let first = seen_bot.insert(fld(l, "id").to_string()); emit(l.to_string(), &mut c, first.then_some("bot")) }
            "MemorySaved" | "MemoryForgot" | "SpaceSet" | "SpacePageSet" if bk.contains(fld(l, "bot")) => emit(l.to_string(), &mut c, (t == "MemorySaved").then_some("memory")),
            "HookSet" if hk.contains(fld(l, "id")) => emit(l.to_string(), &mut c, None),
            "RoutineSet" if ok_routine(fld(l, "id")) => {
                // arrives switched off: 0.14 may still run it, and a routine must not fire twice
                let line = if l.contains("\"on\":true") { routines_on.insert(fld(l, "id").to_string()); l.replacen("\"on\":true", "\"on\":false", 1) } else { l.to_string() };
                let first = seen_routine.insert(fld(l, "id").to_string()); emit(line, &mut c, first.then_some("routine"))
            }
            "RoomSet" if rk.contains(fld(l, "id")) => { let first = seen_room.insert(fld(l, "id").to_string()); emit(l.to_string(), &mut c, first.then_some("room")) }
            "RoomPosted" if rk.contains(fld(l, "room")) => emit(l.to_string(), &mut c, Some("post")),
            "UberSet" if !have.settings.contains("uber.bot") && uber.as_deref() == Some(fld(l, "bot")) && bk.contains(fld(l, "bot")) => emit(l.to_string(), &mut c, None),
            "DeviceRegistered" | "DevicePrefs" if wanted_devices.contains(&line_hash(l)) => emit(l.to_string(), &mut c, (t == "DeviceRegistered").then_some("push")),
            _ => {}
        }
    })?;
    // tasks that were running when 0.14 was left: interrupted, so nothing here waits on them
    let mut open: Vec<(&String, &(String, u64))> = task_last.iter().filter(|(k, (st, _))| kk.contains(*k) && matches!(st.as_str(), "running" | "")).collect(); open.sort();
    for (k, (_, at)) in open { emit(serde_json::json!({"$":"TaskStateSet","task":k,"state":"interrupted","summary":"Stopped when Backplane 0.14 was left.","at":at}).to_string(), &mut c, None) }
    // settings: the last value of each one that still means something, unless this home has its own
    let mut keys: Vec<(&String, &(String, u64))> = settings.iter().filter(|(k, (v, _))| !v.is_empty() && keeps_setting(k) && !have.settings.contains(*k)
        && (k.starts_with("board.") || k.starts_with("rule.") || ids_in(k).iter().all(|id| tk.contains(*id) || pk.contains(*id) || bk.contains(*id) || rk.contains(*id)))).collect();
    keys.sort();
    for (k, (v, at)) in keys { emit(serde_json::json!({"$":"SettingSet","key":k,"value":v,"at":at}).to_string(), &mut c, Some("setting")) }
    let mut bl: Vec<(String, String)> = bk.iter().map(|id| (id.clone(), bots[id].0.clone())).collect(); bl.sort();
    let mut hl: Vec<String> = hk.into_iter().collect(); hl.sort();
    Ok(Plan { lines: out, counts: c, bots: bl, hooks: hl, routines_on: routines_on.len(), peers, read: upto })
}

//! The sidebar's sections (docs/ALWAYS-ON.md section 8, D11 and D12), drawn the same by GPUI and the phones. Built on the merged project
//! rows (present/merged.rs, one row per project key across machines); this file decides only what shows where.
//!
//! D11, a thread's visibility: it shows while it is running (or monitoring work it owns), needs the person (an ask, an approval, a question,
//! a sign-in), or stopped abnormally (failed, interrupted, cut by a restart or held by a usage limit) until the person looks at it or settles it.
//! A pinned thread always shows. Everything else is reachable from the project page and Ctrl+K (`jump`), never from the sidebar.
//!
//! D12, the order: Bub; Needs you (every machine, gone when empty); Pinned projects; Active (a visible thread or activity in the last
//! ACTIVE days, newest first); Groups (the rest, by git owner then folder, named by Haiku or the person, folded by default); Quiet (nothing for
//! QUIET, folded, never deleted, searchable).
//!
//! Settings this hub keeps (all short, all in the lean copy): `sidebar.pin.<key>` (a pinned project: when), `sidebar.group.of.<key>` (a project
//! the person moved: its group id), `sidebar.group.name.<gid>` (the person's name for a group), `sidebar.group.pick.<gid>` (Haiku's,
//! agents/groupname.rs), `sidebar.looked.<thread>` (when the person last opened a thread that stopped abnormally), `sidebar.cut.<thread>` (a
//! restart cut its turn; cleared when a turn starts), `sidebar.fold.<key>` (the person folded a project row: when; its threads are not listed under
//! it, its counts still are, and a thread of it that needs the person is still in Needs you).
use super::merged::{MergedProject, MergedThread};
use crate::shelf::{shelf_of, Shelf};
use crate::state::{State, Turn};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub use crate::state::visible::ACTIVE;
pub const QUIET: u64 = 14 * 86400;
/// thread rows under one project row in the sidebar (the rest are counted, and on the project page)
const UNDER: usize = 8;
/// rows of the project page's Recent and Done lists, and of the Ctrl+K list
const PAGE: usize = 60;
const JUMP: usize = 600;

// the status vocabulary and D11's rule are the state's own (state/visible.rs: the digest and probe binaries fold the look and cut marks too)
pub use crate::state::visible::*;
fn get<'a>(st: &'a State, k: &str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or("") }
fn num(st: &State, k: &str) -> u64 { get(st, k).parse().unwrap_or(0) }

/// One thread's facts as the sections read them, from its live state or from the registry's row.
struct Th<'a> { t: &'a MergedThread<'a>, word: &'static str, show: bool, needs: bool, kid: bool, settled: bool }
/// An archived thread is gone from every list unless it asks the person (Needs you shows it, so the ask is never hidden).
fn listed(f: &Th) -> bool { !f.t.archived || f.word == "needs" }
fn word_of(s: &str) -> &'static str {
    match s { "needs" => "needs", "running" => "running", "limit" => "limit", "failed" => "failed", "cut" => "cut", "interrupted" => "interrupted", "monitoring" => "monitoring", "done" => "done", _ => "idle" }
}
/// Another machine's thread as the sections believe it (D11 for a mirror): a machine that is away or off runs nothing the person can see, so its
/// live words (running, monitoring, a usage hold) are not believed; and what it last said is believed for ACTIVE only: a thread there quiet for
/// longer leaves the sidebar and Needs you, as a local one settles out of them (it stays on the project page and in Ctrl+K). This machine's own
/// threads are never touched. Pure.
pub fn far_word(word: &'static str, away: bool, quiet_for: u64) -> &'static str {
    if quiet_for > ACTIVE && word != "done" { return "idle" }
    if away && matches!(word, "running" | "monitoring" | "limit") { return "idle" }
    word
}
/// When the person hid another machine's thread here (`sidebar.hide.<id>`, s; 0 for never). Hide is this machine's: it asks nothing of that
/// machine (which may be away), and the thread comes back by itself when anything new happens there (its activity passes the mark).
pub fn hidden_at(st: &State, id: &str) -> u64 { num(st, &format!("sidebar.hide.{id}")) }
fn facts<'a>(st: &State, p: &MergedProject, t: &'a MergedThread<'a>, now: u64) -> Th<'a> {
    let mut f = facts_here(t);
    if !t.machine.is_empty() {
        let away = p.machines.iter().any(|m| m["name"].as_str() == Some(t.machine.as_str()) && m["dim"] == true);
        let hid = hidden_at(st, &t.id) >= t.activity.max(1);
        let w = if hid { "idle" } else { far_word(f.word, away, now.saturating_sub(t.activity)) };
        if w != f.word { f.word = w; f.show = visible_word(w, t.pinned); f.needs = false; }
    }
    f
}
fn facts_here<'a>(t: &'a MergedThread<'a>) -> Th<'a> {
    match t.src {
        Some(st) => {
            let w = status(st, &t.id); let lk = looked(st, &t.id); let se = settled(st, &t.id);
            // a delegated child hangs under its parent: it is listed alone only when it asks the person
            let kid = st.tasks.values().any(|(p, c, _, _)| c == &t.id && st.threads.contains_key(p) && !st.bot_threads_has(p));
            let member = st.swarm_of.contains_key(&t.id); // (its swarm's row carries it)
            Th { t, word: w, show: visible(st, &t.id), needs: !member && needs_you(w, own(st, &t.id), lk, se), kid: kid || member, settled: se }
        }
        None => { let w = word_of(&t.status); Th { t, word: w, show: visible_word(w, t.pinned), needs: w == "needs", kid: false, settled: w == "done" || w == "idle" } }
    }
}

/// The words a row says beside its title (one wording on every client).
pub fn say(word: &str) -> &'static str {
    match word { "needs" => "Needs you", "running" => "Running", "limit" => "Waiting for a usage reset", "failed" => "Failed", "cut" => "Cut by a restart",
        "interrupted" => "Stopped", "monitoring" => "Working in the background", "done" => "Done", _ => "" }
}
/// The dot's word of the older rows (signal.rs `name`), so every client's existing thread row draws a section's row as it is.
fn dot_word(word: &str) -> &'static str {
    match word { "needs" => "blocked", "running" => "active", "limit" | "monitoring" => "waiting", "failed" => "failed", "cut" | "interrupted" => "stopped", "done" => "finished", _ => "inactive" }
}
fn turn_word(word: &str) -> &'static str { match word { "running" => "run", "failed" => "fail", "cut" | "interrupted" => "stop", _ => "idle" } }
/// What a Needs you row wants: approval, question, sign-in, failed, stopped.
fn want(t: &MergedThread, word: &str) -> &'static str {
    match t.src {
        Some(st) if word == "needs" => {
            if st.ask_data.iter().any(|a| a.1 == t.id && a.2 == "approval") || st.threads.get(&t.id).map_or(false, |x| x.pending > 0) { "approval" }
            else if st.asks.values().any(|(th, open)| *open && th == &t.id) { "question" } else { "sign-in" }
        }
        _ => match word { "needs" => "question", "failed" => "failed", "cut" | "interrupted" => "stopped", _ => "" },
    }
}
fn mark_of(p: &MergedProject, machine: &str) -> Value {
    if machine.is_empty() { return Value::Null }
    p.machines.iter().find(|m| m["name"] == machine).cloned().unwrap_or_else(|| crate::mirror::mark_of(machine, false))
}
/// A qualified id as the window keys rows (`<hub>|<id>`); the phones' (`hub` ""), the id alone.
fn qid(hub: &str, id: &str) -> String { if hub.is_empty() { id.to_string() } else { format!("{hub}|{id}") } }
fn thread_row(hub: &str, now: u64, p: &MergedProject, f: &Th) -> Value {
    let t = f.t; let off = t.src.is_none();
    json!({"id":qid(hub,&t.id),"title":if t.title.is_empty() { "New thread" } else { t.title.as_str() },"word":f.word,"say":say(f.word),"want":want(t,f.word),
        "status":dot_word(f.word),"state":turn_word(f.word),"depth":0,"children":0,"expanded":false,"agents":"",
        "ago":crate::sidebar::ago(now,t.activity),"activity":t.activity,"pinned":t.pinned,"machine":t.machine,"mark":mark_of(p,&t.machine),
        "project":p.name,"projectKey":p.key,"off":off,"nav":if off { String::new() } else { qid(hub,&t.id) },
        // another machine's thread can be hidden here (its menu's first item on the desktop, a long press on the phones)
        "hide":hide_of(hub,t),"lead":if hide_of(hub,t).is_empty() { json!([]) } else { json!([{"label":"Hide","action":"thread-hide","value":hide_of(hub,t),"tone":"settle","options":[]}]) }})
}
fn hide_of(hub: &str, t: &MergedThread) -> String { if t.machine.is_empty() { String::new() } else { qid(hub, &t.id) } }

// ---- groups -----------------------------------------------------------------------------------------------------------------
/// A project's automatic group: its git owner (`owner:github.com/acme`), else its folder's parent (`folder:/home/h/Documents/Acme`), with the
/// raw label (owner, folder name) a name is made from. A folder right in a home (this machine's, or any `/home/<who>`, `/Users/<who>`,
/// `C:\Users\<who>` a linked machine names) is the one personal group, `folder:~`.
pub fn auto_group(origin: &str, folder: &str) -> (String, String) {
    let parts: Vec<&str> = origin.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() >= 3 { return (format!("owner:{}/{}", parts[0], parts[1].to_lowercase()), parts[1].to_string()) }
    let parent = std::path::Path::new(folder.trim_end_matches(['/', '\\'])).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let home = crate::plat::dirs::home_or_empty();
    if parent.is_empty() || parent == "/" || (!home.is_empty() && parent == home) || a_home(&parent) { return ("folder:~".into(), "Personal".into()) }
    let base = std::path::Path::new(&parent).file_name().map(|b| b.to_string_lossy().to_string()).unwrap_or_default();
    (format!("folder:{parent}"), base)
}
/// A home folder on any machine: `/home/<who>`, `/Users/<who>`, `C:\Users\<who>`.
fn a_home(dir: &str) -> bool {
    let d = dir.replace('\\', "/"); let d = d.trim_end_matches('/');
    let parts: Vec<&str> = d.split('/').collect();
    match parts.as_slice() { ["", "home" | "Users", who] => !who.is_empty(), [drive, u, who] => drive.len() == 2 && drive.ends_with(':') && u.eq_ignore_ascii_case("users") && !who.is_empty(), _ => false }
}
/// A project's group as kept: the person's move, else its automatic one. Groups of one name are one group (`Groups::id`).
fn group_raw(st: &State, p: &MergedProject) -> String {
    let moved = get(st, &format!("sidebar.group.of.{}", p.key));
    if !moved.is_empty() { moved.to_string() } else { auto_group(&p.origin, &p.folder).0 }
}
/// A group's name: the person's, else Haiku's, else the raw label.
pub fn group_name(st: &State, gid: &str, raw: &str) -> String {
    let n = get(st, &format!("sidebar.group.name.{gid}")); if !n.is_empty() { return n.to_string() }
    auto_name(st, gid, raw)
}
/// A group's name before the person names it: Haiku's, else the raw label, else its id's last word.
fn auto_name(st: &State, gid: &str, raw: &str) -> String {
    let n = get(st, &format!("sidebar.group.pick.{gid}")); if !n.is_empty() { return n.to_string() }
    if !raw.is_empty() { return raw.to_string() }
    gid.split_once(':').map_or(gid, |(_, r)| r).rsplit('/').next().unwrap_or(gid).to_string()
}
fn fold_name(n: &str) -> String { n.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase() }

/// The groups as the person sees them: each name once. Group ids come from where a project lives (its git owner, its folder's parent, a
/// linked machine's paths) and Haiku names each on its own, so `owner:github.com/acme`, `owner:github.com/acme-inc` and `folder:/home/h/Acme`
/// can all be "Acme", and every folder of a person's own work "Personal". Ids of one name (Haiku's or the person's) are one group, whose id is
/// the smallest of them; a move kept on any of them lands in it, so nothing is rewritten and no assignment is lost.
pub struct Groups { canon: HashMap<String, String>, names: HashMap<String, String>, members: HashMap<String, Vec<String>> }
impl Groups {
    pub fn of(st: &State, rows: &[MergedProject]) -> Groups {
        // every id in use, with the first raw label a member gives it
        let mut ids: Vec<(String, String)> = vec![];
        for p in rows {
            let gid = group_raw(st, p);
            let (auto, raw) = auto_group(&p.origin, &p.folder);
            let raw = if auto == gid { raw } else { String::new() };
            match ids.iter_mut().find(|x| x.0 == gid) { Some(x) => { if x.1.is_empty() { x.1 = raw } } None => ids.push((gid, raw)) }
        }
        ids.sort();
        let n = ids.len();
        let mut up: Vec<usize> = (0..n).collect();
        fn root(up: &mut [usize], i: usize) -> usize { let mut i = i; while up[i] != i { up[i] = up[up[i]]; i = up[i]; } i }
        fn join(up: &mut [usize], a: usize, b: usize) { let (a, b) = (root(up, a), root(up, b)); if a != b { up[a.max(b)] = a.min(b); } }
        // one automatic name, one group
        let autos: Vec<String> = ids.iter().map(|(g, r)| fold_name(&auto_name(st, g, r))).collect();
        for i in 0..n { for j in 0..i { if autos[i] == autos[j] { join(&mut up, i, j); } } }
        // a group's name: the person's name on its first id that has one, else the automatic name of its first id
        let named = |up: &mut Vec<usize>| -> Vec<String> {
            let mut by: HashMap<usize, String> = HashMap::new();
            for i in 0..n { let r = root(up, i); let own = get(st, &format!("sidebar.group.name.{}", ids[i].0)); if !own.is_empty() && !by.contains_key(&r) { by.insert(r, own.to_string()); } }
            (0..n).map(|i| { let r = root(up, i); by.get(&r).cloned().unwrap_or_else(|| auto_name(st, &ids[r].0, &ids[r].1)) }).collect()
        };
        // the person named two groups alike (or one like another's automatic name): one group
        let first = named(&mut up);
        for i in 0..n { for j in 0..i { if fold_name(&first[i]) == fold_name(&first[j]) { join(&mut up, i, j); } } }
        let shown = named(&mut up);
        let (mut canon, mut names, mut members): (HashMap<String, String>, HashMap<String, String>, HashMap<String, Vec<String>>) = Default::default();
        for i in 0..n {
            let c = ids[root(&mut up, i)].0.clone();
            canon.insert(ids[i].0.clone(), c.clone()); names.insert(c.clone(), shown[i].clone()); members.entry(c).or_default().push(ids[i].0.clone());
        }
        Groups { canon, names, members }
    }
    /// The group a kept id is in (an id no project uses is its own).
    pub fn id(&self, raw: &str) -> String { self.canon.get(raw).cloned().unwrap_or_else(|| raw.to_string()) }
    /// A project's group.
    pub fn of_project(&self, st: &State, p: &MergedProject) -> String { self.id(&group_raw(st, p)) }
    pub fn name(&self, st: &State, gid: &str) -> String { self.names.get(&self.id(gid)).cloned().unwrap_or_else(|| group_name(st, gid, "")) }
    /// The ids merged into a group (its own first).
    pub fn members(&self, gid: &str) -> Vec<String> { self.members.get(&self.id(gid)).cloned().unwrap_or_else(|| vec![gid.to_string()]) }
    /// Every group once, by name: (id, name).
    pub fn list(&self) -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = self.names.iter().map(|(g, n)| (g.clone(), n.clone())).collect();
        v.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()).then(a.0.cmp(&b.0))); v
    }
}
/// The groups that want a name from Haiku: (gid, raw label, member names and origins), none named by the person or picked already.
pub fn groups_due(st: &State, rows: &[MergedProject]) -> Vec<(String, String, Vec<String>)> {
    let mut by: Vec<(String, String, Vec<String>)> = vec![];
    for p in rows {
        let gid = group_raw(st, p);
        if gid.starts_with("custom:") || !get(st, &format!("sidebar.group.name.{gid}")).is_empty() || !get(st, &format!("sidebar.group.pick.{gid}")).is_empty() { continue }
        let raw = auto_group(&p.origin, &p.folder).1;
        let line = if p.origin.is_empty() { format!("{} ({})", p.name, p.folder) } else { format!("{} ({})", p.name, p.origin) };
        match by.iter_mut().find(|g| g.0 == gid) { Some(g) => { if g.2.len() < 12 { g.2.push(line) } } None => by.push((gid, raw, vec![line])) }
    }
    by
}
/// A new group by its name, with a project in it: the settings it writes (`custom:<slug>`; a name another group has joins that group).
pub fn new_group_sets(key: &str, name: &str) -> Option<Vec<(String, String)>> {
    let name: String = name.trim().chars().take(40).collect(); if key.is_empty() || name.is_empty() { return None }
    let slug: String = name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let gid = format!("custom:{slug}");
    Some(vec![(format!("sidebar.group.of.{key}"), gid.clone()), (format!("sidebar.group.name.{gid}"), name)])
}
/// An archived project (`sidebar.archived.<key>`): in Quiet whatever its activity, until it is unarchived; a thread of it that needs the
/// person is still in Needs you.
fn archived_at(st: &State, key: &str) -> u64 { num(st, &format!("sidebar.archived.{key}")) }

/// A project row's menu (right click on the desktop, long press on the phones), the same on every client: each item an `id` the clients know,
/// its words, the hub action and value GPUI sends; Move to group is a submenu of the other groups and New group…; Remove asks first
/// (`confirm`) and only takes the project off Backplane's list (`project.remove`: its folder and files stay). A project only on linked
/// machines has no Rename, Badge, Open folder or Remove here.
pub fn project_menu(st: &State, p: &MergedProject, gid: &str, groups: &Groups) -> Value {
    let key = &p.key; let pid = pid_of(p);
    let pinned = pinned_at(st, key) > 0; let archived = archived_at(st, key) > 0;
    let it = |id: &str, label: &str, action: &str, value: &str| json!({"id":id,"label":label,"action":action,"value":value});
    let mut v = vec![it("pin", if pinned { "Unpin project" } else { "Pin project" }, "project-pin", key), it("swarm", "New swarm", "swarm-new", &pid)];
    if let Some(here) = &p.here {
        v.push(it("rename", "Rename", "proj-rename", here));
        v.push(it("badge", "Badge", "look", here));
        if !p.folder.is_empty() { v.push(it("open", "Open folder", "open-folder", &p.folder)); }
    }
    let mut sub: Vec<Value> = groups.list().into_iter().filter(|(g, _)| g != gid).map(|(g, n)| it("group", &n, "group-move", &format!("{key}\u{1f}{g}"))).collect();
    sub.push(it("new-group", "New group\u{2026}", "group-new-start", key));
    v.push(json!({"id":"move","label":"Move to group","action":"","value":"","sub":sub}));
    v.push(it("archive", if archived { "Unarchive" } else { "Archive" }, "project-archive", key));
    if let Some(here) = &p.here { let mut r = it("remove", "Remove project", "proj-remove", here); r["confirm"] = json!(true); v.push(r); }
    json!(v)
}

// ---- the sections ---------------------------------------------------------------------------------------------------------
pub struct Ctx<'a> {
    pub hub: &'a str, pub now: u64, pub query: &'a str,
    /// the section folds the person opened (`sec:<id>`), and the selected thread (its project is never folded away)
    pub open: &'a HashSet<String>, pub selected: &'a str,
}
/// The project id a client opens a row by: this machine's, else the first copy's (`<link>~<id>` on a linked machine).
fn pid_of(p: &MergedProject) -> String { p.here.clone().unwrap_or_else(|| p.ids.first().map(|x| x.1.clone()).unwrap_or_default()) }
fn hit(text: &str, q: &str) -> bool { q.is_empty() || crate::sidebar::fuzzy_hit(text, q) }
fn pinned_at(st: &State, key: &str) -> u64 { num(st, &format!("sidebar.pin.{key}")) }
/// When the person folded a project row (`sidebar.fold.<key>`), 0 for open.
pub fn folded_at(st: &State, key: &str) -> u64 { num(st, &format!("sidebar.fold.{key}")) }

/// A project's live swarms (docs/SWARM.md) as rows, every copy's: `sidebar::swarm_rows`' shape (the clients' swarm rows draw it) with the sections'
/// fields: `kind` "swarm", `word` (`sidebar::live_swarms`; `needs` too when a member waits on the person), `want`, `title`, `project`, its machine.
fn swarms_of(p: &MergedProject, c: &Ctx) -> Vec<(Value, &'static str)> {
    let mut states: Vec<&State> = p.threads.iter().filter_map(|t| t.src).collect();
    if let Some((s, _)) = &p.settings_of { states.push(s); }
    states.dedup_by(|a, b| std::ptr::eq(*a, *b));
    let mut seen: Vec<*const State> = vec![]; let mut out = vec![];
    let open = HashSet::new(); let o = crate::sidebar::Opts { hub: c.hub, now: c.now, selected: "", open: &open, older_open: false, query: "" };
    for s in states {
        if seen.contains(&(s as *const State)) { continue } seen.push(s as *const State);
        let live = crate::sidebar::live_swarms(s);
        for (machine, pid) in p.ids.iter().filter(|(_, pid)| s.projects.contains_key(pid.as_str())) {
            for mut r in crate::sidebar::swarm_rows(s, &o, pid) {
                let room = text_of(&r, "nav").trim_start_matches("@room:").to_string();
                let Some(l) = live.iter().find(|l| l.id == room) else { continue };
                let asks = s.swarm_of.iter().any(|(t, sw)| sw == &room && wants_person(s, t));
                let word = if asks { "needs" } else { l.word };
                r["kind"] = json!("swarm"); r["word"] = json!(word); r["title"] = json!(l.title); r["say"] = json!(say(word));
                r["want"] = json!(match word { "needs" => "question", "failed" => "failed", _ => "" });
                r["status"] = json!(dot_word(word)); r["machine"] = json!(machine); r["mark"] = mark_of(p, machine);
                r["project"] = json!(p.name); r["projectKey"] = json!(p.key); r["off"] = json!(false); r["activity"] = json!(l.activity);
                out.push((r, word));
            }
        }
    }
    out
}
fn text_of<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

/// One project row: the merged row (`merged::row_json`) with its counts and its visible threads (Needs you's are not repeated under it).
fn project_row(st: &State, c: &Ctx, p: &MergedProject, ths: &[Th], gid: &str, groups: &Groups) -> Value {
    let mut r = super::merged::row_json(p);
    // what runs: a turn running (one that also asks the person counts here and in Needs you), work it owns, a turn held for a reset
    let running = ths.iter().filter(|f| matches!(f.word, "running" | "monitoring" | "limit") || (f.word == "needs" && f.t.src.map_or(false, |s| s.threads.get(&f.t.id).map_or(false, |x| x.turn == Turn::Running)))).count();
    let needs = ths.iter().filter(|f| f.needs).count();
    let q = c.query;
    let shown: Vec<&Th> = ths.iter().filter(|f| if q.is_empty() { f.show && !f.needs && !f.kid } else { crate::sidebar::word_hit(&f.t.title, q) }).collect();
    // folded by the person: no rows under it (a search opens it, as it opens the section folds); its counts stay on the row
    let folded = folded_at(st, &p.key) > 0 && q.is_empty();
    let under: Vec<Value> = shown.iter().take(UNDER).map(|f| thread_row(c.hub, c.now, p, f)).collect();
    let rows: Vec<Value> = if folded { vec![] } else { under.clone() };
    r["id"] = json!(qid(c.hub, &pid_of(p)));
    r["title"] = json!(p.name);
    r["pinned"] = json!(pinned_at(st, &p.key) > 0);
    r["threads"] = json!(rows); r["more"] = json!(ths.len().saturating_sub(rows.len()));
    // its live swarms, above its threads (a swarm that needs the person is in Needs you, not repeated here)
    let sw = swarms_of(p, c);
    r["running"] = json!(running + sw.iter().filter(|x| x.1 == "running").count());
    r["needsCount"] = json!(needs + sw.iter().filter(|x| x.1 != "running").count());
    let live: Vec<Value> = sw.into_iter().filter(|x| x.1 == "running").map(|x| x.0).collect();
    // the fold: `fold` the action a chevron sends (with `nav`), `folded` its state, `under` how many rows it hides or would hide (no chevron at 0, open)
    r["fold"] = json!("project-fold"); r["folded"] = json!(folded); r["under"] = json!(under.len() + live.len());
    r["swarms"] = json!(if folded { vec![] } else { live });
    r["group"] = json!(gid);
    r["archived"] = json!(archived_at(st, &p.key) > 0);
    r["menu"] = project_menu(st, p, gid, groups);
    r["nav"] = json!(p.key);
    // the machines' dots: one per machine that has it, dim when away or off
    // every machine that has it is off or away: the row is drawn dimmed (lane I's `off`), and still opens its page (the registry lists its threads)
    r["off"] = json!(!p.machines.is_empty() && p.machines.iter().all(|m| m["dim"] == true));
    r["dots"] = json!(p.machines.iter().map(|m| json!({"name":m["name"],"short":m["short"],"here":m["here"],"dim":m["dim"],"note":m["note"]})).collect::<Vec<_>>());
    r
}
fn fold_section(id: &str, title: &str, rows: Vec<Value>, c: &Ctx, extra: Value) -> Value {
    let open = c.open.contains(&format!("sec:{id}")) || !c.query.is_empty() || rows.iter().any(|r| r["threads"].as_array().map_or(false, |t| t.iter().any(|x| x["id"] == json!(qid(c.hub, c.selected)))));
    let mut s = json!({"id":id,"kind":"projects","title":title,"count":rows.len(),"fold":true,"open":open,"toggle":format!("sec:{id}"),"rows":if open { rows } else { vec![] }});
    if let (Some(o), Some(x)) = (s.as_object_mut(), extra.as_object()) { for (k, v) in x { o.insert(k.clone(), v.clone()); } }
    s
}

/// The sections, top to bottom. `bub` is the uberbot's sidebar row (screen.rs `bots`), or null.
pub fn build(st: &State, rows: &[MergedProject], bub: &Value, c: &Ctx) -> Value {
    let facts_of: Vec<Vec<Th>> = rows.iter().map(|p| p.threads.iter().map(|t| facts(st, p, t, c.now)).filter(listed).collect()).collect();
    let mut out = vec![];
    if bub.is_object() { out.push(json!({"id":"bub","kind":"bub","title":"","rows":[bub]})); }
    // Needs you: every machine's, approvals first, then questions, then stops, newest first within each
    let mut needs: Vec<(u8, u64, Value)> = vec![];
    for (p, ths) in rows.iter().zip(&facts_of) { for f in ths.iter().filter(|f| f.needs) {
        if !c.query.is_empty() && !hit(&f.t.title, c.query) && !hit(&p.name, c.query) { continue }
        let r = thread_row(c.hub, c.now, p, f);
        let rank = match r["want"].as_str().unwrap_or("") { "approval" => 0, "question" | "sign-in" => 1, "failed" => 2, _ => 3 };
        needs.push((rank, f.t.activity, r));
    } }
    // a live swarm that waits on the person (a guard paused it, a member asks) or whose member failed: in Needs you, opening its page
    for p in rows { for (r, w) in swarms_of(p, c) { if w == "running" { continue }
        if !c.query.is_empty() && !hit(text_of(&r, "title"), c.query) && !hit(&p.name, c.query) { continue }
        let at = r["activity"].as_u64().unwrap_or(0); needs.push((if w == "needs" { 1 } else { 2 }, at, r)); } }
    // an ask outside every project (a bot's chat, a room's run, a scratch chat; a linked machine's too) is in Needs you like any other
    { let listed: HashSet<&str> = rows.iter().flat_map(|p| p.threads.iter().map(|t| t.id.as_str())).collect();
      let mut states: Vec<(&State, String)> = vec![(st, String::new())];
      for p in rows { for t in &p.threads { if let Some(s) = t.src { if !t.machine.is_empty() && !states.iter().any(|(x, _)| std::ptr::eq(*x, s)) { states.push((s, t.machine.clone())); } } } }
      for (s, machine) in states { // (another machine's ask is believed for ACTIVE, as `far_word` has it)
      for (id, t) in s.threads.iter().filter(|(id, t)| !t.deleted && !listed.contains(id.as_str()) && wants_person(s, id) && (machine.is_empty() || (far_word("needs", false, c.now.saturating_sub(t.activity)) == "needs" && hidden_at(st, id) < t.activity.max(1)))) {
          let place = s.bots_nav.values().find(|b| &b.thread == id).map(|b| b.name.clone()).unwrap_or_else(|| "Conversation history".into());
          if !c.query.is_empty() && !hit(&t.title, c.query) && !hit(&place, c.query) { continue }
          let mt = MergedThread { machine: machine.clone(), id: id.clone(), title: t.title.clone(), activity: t.activity, pinned: t.pinned, archived: t.archived, src: Some(s), status: String::new() };
          let w = want(&mt, "needs");
          let mark = if machine.is_empty() { Value::Null } else { crate::mirror::mark_of(&machine, false) };
          needs.push((if w == "approval" { 0 } else { 1 }, t.activity, json!({"id":qid(c.hub,id),"title":t.title,"word":"needs","say":say("needs"),"want":w,"status":"blocked","state":"idle","depth":0,"children":0,"expanded":false,"agents":"",
              "ago":crate::sidebar::ago(c.now,t.activity),"activity":t.activity,"pinned":t.pinned,"machine":machine,"mark":mark,"project":place,"projectKey":"","off":false,"nav":qid(c.hub,id)})));
      } } }
    needs.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    if !needs.is_empty() { out.push(json!({"id":"needs","kind":"threads","title":"Needs you","count":needs.len(),"rows":needs.into_iter().map(|x| x.2).collect::<Vec<_>>()})); }
    // the projects: pinned, active, by group, quiet
    let (mut pinned, mut active, mut quiet): (Vec<(u64, Value)>, Vec<(u64, Value)>, Vec<Value>) = (vec![], vec![], vec![]);
    let mut groups: Vec<(String, String, Vec<Value>)> = vec![];
    let all = Groups::of(st, rows);
    let held = rows.iter().zip(&facts_of).find(|(_, ths)| ths.iter().any(|f| f.t.id == c.selected)).map(|(p, _)| p.key.clone()).unwrap_or_default();
    for (p, ths) in rows.iter().zip(&facts_of) {
        if !c.query.is_empty() && !hit(&p.name, c.query) && !ths.iter().any(|f| crate::sidebar::word_hit(&f.t.title, c.query)) { continue }
        let gid = all.of_project(st, p);
        let row = project_row(st, c, p, ths, &gid, &all);
        let pin = pinned_at(st, &p.key);
        let shows = ths.iter().any(|f| f.show && !f.kid) || row["swarms"].as_array().map_or(false, |a| !a.is_empty()) || row["needsCount"].as_u64().unwrap_or(0) > 0;
        if pin > 0 { pinned.push((pin, row)); continue }
        // archived: in Quiet, unless the person is held by it (its asks are in Needs you all the same)
        if archived_at(st, &p.key) > 0 && p.key != held { quiet.push(row); continue }
        if shows || p.last_activity + ACTIVE > c.now || p.key == held { active.push((p.last_activity, row)); continue }
        if p.last_activity + QUIET <= c.now { quiet.push(row); continue }
        match groups.iter_mut().find(|g| g.0 == gid) { Some(g) => g.2.push(row), None => groups.push((gid.clone(), all.name(st, &gid), vec![row])) }
    }
    pinned.sort_by(|a, b| a.0.cmp(&b.0));
    active.sort_by(|a, b| b.0.cmp(&a.0));
    if !pinned.is_empty() { out.push(json!({"id":"pinned","kind":"projects","title":"Pinned","count":pinned.len(),"rows":pinned.into_iter().map(|x| x.1).collect::<Vec<_>>()})); }
    out.push(json!({"id":"active","kind":"projects","title":"Active","count":active.len(),"rows":active.into_iter().map(|x| x.1).collect::<Vec<_>>()}));
    groups.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    for (gid, name, rs) in groups { out.push(fold_section(&format!("group:{gid}"), &name, rs, c, json!({"group":gid,"rename":true}))); }
    if !quiet.is_empty() { out.push(fold_section("quiet", "Quiet", quiet, c, json!({"hint":"Nothing for two weeks"}))); }
    json!(out)
}

/// Every group once, by name (`Groups`): what an older client's menu lists to move a project to. A row's own `menu` leaves out its group.
pub fn move_targets(st: &State, rows: &[MergedProject]) -> Value {
    json!(Groups::of(st, rows).list().into_iter().map(|(g, n)| json!({"value":g,"label":n})).collect::<Vec<_>>())
}

// ---- a project's page -----------------------------------------------------------------------------------------------------
/// A project's page: running, then needs you, then recent, then done behind "Done (n)". `done_open`: the person opened Done.
pub fn page(st: &State, rows: &[MergedProject], key: &str, c: &Ctx, done_open: bool) -> Value {
    let Some(p) = rows.iter().find(|p| p.key == key) else { return Value::Null };
    let ths: Vec<Th> = p.threads.iter().map(|t| facts(st, p, t, c.now)).filter(listed).collect();
    let (mut running, mut needs, mut recent, mut done) = (vec![], vec![], vec![], vec![]);
    for (r, w) in swarms_of(p, c) { if w == "running" { running.push(r) } else { needs.push(r) } } // its live swarms, as on the sidebar
    for f in &ths {
        let r = thread_row(c.hub, c.now, p, f);
        if f.needs { needs.push(r) } else if matches!(f.word, "running" | "monitoring" | "limit") { running.push(r) }
        else if f.settled || (f.word == "done" && !f.show) { done.push(r) } else { recent.push(r) }
    }
    let all = Groups::of(st, rows); let gid = all.of_project(st, p);
    let n = done.len(); recent.truncate(PAGE); done.truncate(PAGE);
    let mut head = super::merged::row_json(p);
    head["pinned"] = json!(pinned_at(st, &p.key) > 0); head["group"] = json!(gid); head["groupName"] = json!(all.name(st, &gid)); head["menu"] = project_menu(st, p, &gid, &all);
    head["newThread"] = json!(p.here.clone().unwrap_or_default());
    // what a client's page head draws (the shape of a sidebar project row): its id here, else on the newest machine
    head["id"] = json!(qid(c.hub, &pid_of(p))); head["title"] = json!(p.name); head["root"] = json!(p.folder); head["nav"] = json!(p.key);
    json!({"key":p.key,"project":head,"sections":[
        {"id":"running","title":"Running","rows":running},
        {"id":"needs","title":"Needs you","rows":needs},
        {"id":"recent","title":"Recent","rows":recent},
        {"id":"done","title":format!("Done ({n})"),"count":n,"fold":true,"open":done_open,"toggle":"page-done","rows":if done_open { done } else { vec![] }}],
        "moveTo":move_targets(st, rows)})
}

// ---- Ctrl+K ---------------------------------------------------------------------------------------------------------------
/// Every project and thread on every machine (the registry's too), for the palette: projects open their page, threads open (one whose machine
/// is off says so and opens nothing). What needs the person first, then the newest. Bounded.
pub fn jump(rows: &[MergedProject], c: &Ctx) -> Value {
    let mut v: Vec<(u8, u64, Value)> = vec![];
    for p in rows {
        let machines: Vec<String> = p.machines.iter().filter_map(|m| m["short"].as_str().map(String::from)).collect();
        v.push((2, p.last_activity, json!({"kind":"project","label":p.name,"place":machines.join(" · "),"action":"project-page","value":p.key,"pid":pid_of(p)})));
        // its live swarms: a jump opens the swarm's page (its room)
        for (r, w) in swarms_of(p, c) { v.push((if w == "running" { 1 } else { 0 }, r["activity"].as_u64().unwrap_or(0), json!({"kind":"swarm","label":r["title"],"place":p.name,"action":"room","value":qid(c.hub, text_of(&r, "nav").trim_start_matches("@room:")),"word":w,"status":r["status"],"want":r["want"]}))); }
        for t in &p.threads {
            let f = facts(&State::new(), p, t, c.now); if !listed(&f) { continue } // (a thread hidden here is still found)
            let place = match &t.src { Some(_) if t.machine.is_empty() => p.name.clone(), _ => format!("{} · {}", p.name, crate::mirror::short_name(&t.machine)) };
            let rank = if f.needs { 0 } else if f.show { 1 } else { 3 };
            v.push((rank, t.activity, json!({"kind":"thread","label":if t.title.is_empty() { "New thread" } else { t.title.as_str() },"place":place,"word":f.word,"status":dot_word(f.word),"state":turn_word(f.word),"want":want(t,f.word),
                "action":if t.src.is_some() { "select" } else { "" },"value":qid(c.hub,&t.id),"off":t.src.is_none()})));
        }
    }
    v.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    v.truncate(JUMP);
    json!(v.into_iter().map(|x| x.2).collect::<Vec<_>>())
}

/// What a sidebar action of the sections does to the session and which settings it writes (`setting.set` sends). None: not one of these.
pub struct Act { pub open_toggle: Option<String>, pub page: Option<String>, pub sets: Vec<(String, String)>, pub clear: Vec<String>, pub drafts: Vec<(String, String)> }
pub fn act(st: &State, rows: &[MergedProject], action: &str, value: &str, drafts: &Value, now: u64) -> Option<Act> {
    let none = || Act { open_toggle: None, page: None, sets: vec![], clear: vec![], drafts: vec![] };
    let known = |key: &str| rows.iter().any(|p| p.key == key);
    match action {
        // a section's fold (Groups, Quiet) and the project page's Done (n)
        "side-section" => { if !(value.starts_with("sec:group:") || value == "sec:quiet" || value == "page-done") { return None } Some(Act { open_toggle: Some(value.into()), ..none() }) }
        "project-page" => { if !value.is_empty() && !known(value) { return None } Some(Act { page: Some(value.into()), ..none() }) }
        // another machine's thread, hidden here until something new happens there (`hidden_at`); a local thread has Settle and Archive
        "thread-hide" => {
            let id = value.rsplit_once('|').map_or(value, |(_, r)| r);
            if !crate::net::far::is(id) || !rows.iter().any(|p| p.threads.iter().any(|t| t.id == id && !t.machine.is_empty())) { return None }
            Some(Act { sets: vec![(format!("sidebar.hide.{id}"), now.to_string())], ..none() })
        }
        // a project row's chevron: fold its threads away, or unfold them (kept per project in the hub, so every client and restart agrees)
        "project-fold" => { if !known(value) { return None } let on = folded_at(st, value) > 0; Some(Act { sets: vec![(format!("sidebar.fold.{value}"), if on { String::new() } else { now.max(1).to_string() })], ..none() }) }
        "project-pin" => { if !known(value) { return None } let on = pinned_at(st, value) > 0; Some(Act { sets: vec![(format!("sidebar.pin.{value}"), if on { String::new() } else { now.to_string() })], ..none() }) }
        // a group's name, from its rename field (`@f.group.<gid>`); an empty name gives the group back its automatic one
        "group-rename" => {
            // "<gid>\u{1f}<name>" (the phones), else the field `@f.group.<gid>`
            let (gid, name) = match value.split_once('\u{1f}') { Some((g, n)) => (g, n.to_string()), None => (value, drafts[format!("@f.group.{value}")].as_str().unwrap_or("").to_string()) };
            let name = name.trim().chars().take(40).collect::<String>();
            if gid.is_empty() { return None }
            // a merged group's name goes on each of its ids, so it stays one group under the new name
            let sets = Groups::of(st, rows).members(gid).into_iter().map(|g| (format!("sidebar.group.name.{g}"), name.clone())).collect();
            Some(Act { sets, clear: vec![format!("@f.group.{gid}")], ..none() })
        }
        // GPUI's Rename group: the hub's rename bar (kind "group"; commands/actions.rs `rename-save` writes the name)
        "group-rename-start" => {
            if value.is_empty() { return None }
            let all = Groups::of(st, rows); let gid = all.id(value); let name = all.name(st, &gid);
            Some(Act { drafts: vec![("@ren".into(), gid), ("@ren.kind".into(), "group".into()), ("@ren.inline".into(), String::new()), ("@ren.owner".into(), String::new()), ("@f.title".into(), name)], ..none() })
        }
        // a project moved: "<key>\u{1f}<gid>", or "<key>\u{1f}+<new group's name>"; "<key>\u{1f}" puts it back in its automatic group
        "group-move" => {
            let (key, to) = value.split_once('\u{1f}')?; if !known(key) { return None }
            if let Some(name) = to.strip_prefix('+') { return Some(Act { sets: new_group_sets(key, name)?, ..none() }) }
            let to = if to.is_empty() { String::new() } else { Groups::of(st, rows).id(to) };
            Some(Act { sets: vec![(format!("sidebar.group.of.{key}"), to)], ..none() })
        }
        // New group… (GPUI): the rename bar asks its name (kind "group-new"; commands/actions.rs `rename-save` moves the project into it)
        "group-new-start" => {
            if !known(value) { return None }
            Some(Act { drafts: vec![("@ren".into(), value.into()), ("@ren.kind".into(), "group-new".into()), ("@ren.inline".into(), String::new()), ("@ren.owner".into(), String::new()), ("@f.title".into(), String::new())], ..none() })
        }
        // Archive: the project goes to Quiet (and is unpinned) until Unarchive; nothing of it is deleted
        "project-archive" => {
            if !known(value) { return None }
            if archived_at(st, value) > 0 { return Some(Act { sets: vec![(format!("sidebar.archived.{value}"), String::new())], ..none() }) }
            Some(Act { sets: vec![(format!("sidebar.archived.{value}"), now.to_string()), (format!("sidebar.pin.{value}"), String::new())], ..none() })
        }
        _ => None,
    }
}

/// The open folds of the session (`open.sec:*` and `open.page-done`).
pub fn opened(session: &Value) -> HashSet<String> {
    session["open"].as_object().into_iter().flatten().filter(|(k, v)| (k.starts_with("sec:") || *k == "page-done") && v.as_bool() == Some(true)).map(|(k, _)| k.clone()).collect()
}
/// A count of each section's rows by id, for tests and the e2e (what the person would see at a glance).
pub fn census(sections: &Value) -> HashMap<String, usize> {
    sections.as_array().into_iter().flatten().map(|s| (s["id"].as_str().unwrap_or("").to_string(), s["count"].as_u64().unwrap_or(s["rows"].as_array().map_or(0, |r| r.len() as u64)) as usize)).collect()
}

#[cfg(test)]
#[path = "sections_tests.rs"]
mod tests;

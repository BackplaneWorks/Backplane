//! One project list over machines (docs/ALWAYS-ON.md section 5): every project of this hub, of each linked machine's mirror and of the
//! registry (machines that are off) folded into one row per key. The key is the project's normalized git origin (`mirror::normalize_origin`,
//! D5) and its folder inside the repository (`github.com/o/r` at the top, `github.com/o/r/boards/a` for a board's folder: one repository holds
//! several projects, #1168); a project with no origin is `<machine>/<id>`, unless a hand link (`project.link.<id>` = another project's key)
//! joins it to another. Copies on one machine never merge by origin: of two clones of one folder there, the older keeps the key and the
//! other is `<machine>/<id>`. So the same folder of a repository on two machines is one row, and different folders, or one machine's clones,
//! are not.
//! Pure: the sidebar sections (present/sections.rs), data/1 `projects.list` and the composer's machine picker all read this and decide nothing
//! of their own about which projects are one.
use crate::mirror::{mark_of, short_name};
use crate::state::State;
use serde_json::{json, Value};
use std::collections::HashMap;

/// A project as the registry on the uberbot's home holds it for a machine that may be off (net/registry.rs fills it).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RegProject { pub key: String, pub name: String, pub origin: String, pub machines: Vec<RegMachine>, pub threads: Vec<RegThread> }
/// One machine's copy of a registry project: its id there (as that machine keys it), folder and when that machine last sent its rows (s).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RegMachine { pub machine: String, pub id: String, pub folder: String, pub seen: u64 }
/// A thread the registry knows: its machine, its id there, title, status (`present::sections::status`'s word) and last activity (s).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RegThread { pub machine: String, pub id: String, pub title: String, pub status: String, pub activity: u64, #[serde(default)] pub pinned: bool }

/// One row of the merged list.
#[derive(Debug)]
pub struct MergedProject<'a> {
    /// the normalized origin and folder, or `<machine>/<id>`, or the key a hand link names
    pub key: String,
    /// the key 0.16.0 gave it (the origin alone): its pins, groups and quiet marks kept under that are carried to `key` (`key_moves`)
    pub was: String,
    /// this machine's name for it, else the most recently active machine's
    pub name: String,
    pub origin: String,
    /// this machine's folder, else the most recently active machine's
    pub folder: String,
    /// this machine's project id when it has the project
    pub here: Option<String>,
    /// every copy: (machine name, "" for this machine; the project id as that state keys it: `<link>~<id>` for a mirror's), this machine first, then by activity
    pub ids: Vec<(String, String)>,
    /// a chip per machine that has it, in `ids` order: `mirror::mark_of` plus `here` (this machine), `state` "here" | "on" | "away" (linked, not
    /// answering) | "off" (known only from the registry), `dim` (away or off) and `note` ("desktop is off") for a dimmed one
    pub machines: Vec<Value>,
    /// every machine's threads, newest first, archived ones too (`MergedThread::archived`)
    pub threads: Vec<MergedThread<'a>>,
    /// the newest activity of any copy (s)
    pub last_activity: u64,
    /// carried from this machine's copy, else the newest (migration: nothing is rewritten)
    pub icon: &'static str, pub color: String, pub css: &'static str,
    /// the state and id the settings (pins, icons, order) of the row are read from: this machine's copy, else the newest live one
    pub settings_of: Option<(&'a State, String)>,
}
#[derive(Debug)]
pub struct MergedThread<'a> {
    /// "" for this machine
    pub machine: String,
    /// as its state keys it (`<link>~<id>` for a mirror's)
    pub id: String,
    pub title: String,
    pub activity: u64,
    pub pinned: bool,
    /// archived on its machine (kept: the sections show an archived thread that needs the person; other readers skip it)
    pub archived: bool,
    /// the live state holding it; None: a registry-only row (its machine is off)
    pub src: Option<&'a State>,
    /// the registry's status word when `src` is None, else ""
    pub status: String,
}

/// What a machine is to the list: this one, a live linked one (its name), or one only the registry knows.
fn chip(machine: &str, here_name: &str, state: &str) -> Value {
    let name = if machine.is_empty() { here_name } else { machine };
    let mut m = mark_of(if name.is_empty() { "this machine" } else { name }, state == "away" || state == "off");
    let dim = state == "away" || state == "off";
    m["here"] = json!(machine.is_empty()); m["state"] = json!(state); m["dim"] = json!(dim);
    let short = m["short"].as_str().unwrap_or("").to_string();
    m["note"] = json!(match state { "off" => format!("{short} is off"), "away" => format!("{short} is off"), /* a linked machine that does not answer: to the person it is off */ _ => String::new() });
    m
}

/// A project's key: its origin; else a hand link's key; else `<machine>/<id>` (`machine` "" is this one, named `here`). `origin` is the
/// project's (`ident`: the origin and its folder in the repository).
pub fn key_of(origin: &str, link: &str, machine: &str, here: &str, id: &str) -> String {
    if !origin.is_empty() { return origin.to_string() }
    if !link.is_empty() { return link.to_string() }
    let local = id.rsplit('~').next().unwrap_or(id);
    format!("{}/{local}", short_name(if machine.is_empty() { if here.is_empty() { "here" } else { here } } else { machine }))
}
/// What names a project across machines: its repository's origin and its folder in it (`rel`, `/`-separated, "" at the top), "" for no origin.
pub fn ident(origin: &str, rel: &str) -> String {
    let rel = rel.trim_matches('/');
    if origin.is_empty() { String::new() } else if rel.is_empty() { origin.to_string() } else { format!("{origin}/{rel}") }
}
/// The folder a key names inside its repository (`ident` read back): `boards/a` of `github.com/o/r/boards/a` with origin `github.com/o/r`.
pub fn rel_in_key(origin: &str, key: &str) -> String {
    if origin.is_empty() { return String::new() }
    key.strip_prefix(origin).and_then(|r| r.strip_prefix('/')).unwrap_or("").to_string()
}
/// One machine's keys for its projects (`(id, created, origin, rel, link)`): `key_of` over `ident`, except that a key an older project of the
/// machine has already taken (two clones of one folder) is that copy's `<machine>/<id>` (or its hand link): one machine's copies never merge.
pub fn keys_of(ps: &[(String, u64, String, String, String)], machine: &str, here: &str) -> HashMap<String, String> {
    let mut v: Vec<&(String, u64, String, String, String)> = ps.iter().collect();
    v.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    let mut taken = std::collections::HashSet::new(); let mut out = HashMap::new();
    for (id, _, o, rel, link) in v {
        let mut k = key_of(&ident(o, rel), link, machine, here, id);
        if taken.contains(&k) { k = key_of("", link, machine, here, id); if taken.contains(&k) { k = key_of("", "", machine, here, id); } }
        taken.insert(k.clone()); out.insert(id.clone(), k);
    }
    out
}
/// This machine's keys (`keys_of` over its projects, `origin` and `rel` read from each folder): what the sidebar, the registry rows and
/// machine_list all say, so every machine computes the same key for a copy.
pub fn local_keys(st: &State, here: &str, origin: &dyn Fn(&str) -> String, rel: &dyn Fn(&str) -> String) -> HashMap<String, String> {
    let ps: Vec<(String, u64, String, String, String)> = st.projects.iter().filter(|(id, p)| !p.removed && id.as_str() != crate::sidebar::SCRATCH)
        .map(|(id, p)| (id.clone(), p.created, origin(&p.root), rel(&p.root), st.settings.get(&format!("project.link.{id}")).cloned().unwrap_or_default())).collect();
    keys_of(&ps, "", here)
}
/// `local_keys` read from the folders on disk.
pub fn local_keys_here(st: &State, here: &str) -> HashMap<String, String> { local_keys(st, here, &|r| crate::mirror::origin_of_root(r), &|r| crate::mirror::repo_rel_of_root(r)) }

/// The per-project settings of the sidebar kept by key (pinned, moved to a group, made quiet, folded).
pub const KEYED: [&str; 4] = ["sidebar.pin.", "sidebar.group.of.", "sidebar.archived.", "sidebar.fold."];
/// The settings that carry what the person set on a project under its 0.16.0 key (`was`, the origin alone, which every project of one
/// repository shared) to its key now, once per key (`sidebar.keys.moved.<key>`): every project of the repository keeps the pin, group and
/// quiet mark its shared row had. Nothing is copied over what is already set under the new key, and nothing is removed. Pure.
pub fn key_moves(st: &State, rows: &[MergedProject]) -> Vec<(String, String)> {
    let get = |k: &str| st.settings.get(k).map(String::as_str).unwrap_or("");
    let mut out = vec![];
    for p in rows {
        if p.was.is_empty() || p.was == p.key { continue }
        let mark = format!("sidebar.keys.moved.{}", p.key);
        if !get(&mark).is_empty() { continue }
        for pre in KEYED { let (old, new) = (format!("{pre}{}", p.was), format!("{pre}{}", p.key)); if !get(&old).is_empty() && get(&new).is_empty() { out.push((new, get(&old).to_string())) } }
        out.push((mark, p.was.clone()));
    }
    out
}

/// The merged list. `st` is this hub's state, `far` the shown linked machines' mirrors `(machine, state)`, `away(machine)` whether a linked machine
/// did not answer, `origin(far id)` a mirrored project's origin (`Mirrors::origin`), `here` this machine's name, `reg` the registry's projects (a
/// machine in it that is neither here nor in `far` is off). Sorted by last activity, newest first.
pub fn merged_projects<'a>(st: &'a State, far: &[(String, &'a State)], away: &dyn Fn(&str) -> bool, origin: &dyn Fn(&str) -> String, here: &str, reg: &[RegProject]) -> Vec<MergedProject<'a>> {
    merged_projects_in(st, far, away, origin, &|_| String::new(), here, reg, &|r| crate::mirror::origin_of_root(r), &|r| crate::mirror::repo_rel_of_root(r))
}
/// `merged_projects` with `rel(far id)`, a mirrored project's folder in its repository (`Mirrors::rel`), and this machine's folders read by
/// `local_origin` and `local_rel` (a project's root -> its origin, its folder in the repository).
#[allow(clippy::too_many_arguments)]
pub fn merged_projects_in<'a>(st: &'a State, far: &[(String, &'a State)], away: &dyn Fn(&str) -> bool, origin: &dyn Fn(&str) -> String, rel: &dyn Fn(&str) -> String, here: &str, reg: &[RegProject],
    local_origin: &dyn Fn(&str) -> String, local_rel: &dyn Fn(&str) -> String) -> Vec<MergedProject<'a>> {
    struct Copy<'a> { machine: String, id: String, st: Option<&'a State>, name: String, folder: String, origin: String, activity: u64, was: String }
    let mut by: Vec<(String, Vec<Copy<'a>>)> = vec![];
    let mut slot: HashMap<String, usize> = HashMap::new();
    let mut put = |key: String, c: Copy<'a>| { let i = *slot.entry(key.clone()).or_insert_with(|| { by.push((key, vec![])); by.len() - 1 }); by[i].1.push(c) };
    let live_act = |s: &State, id: &str, created: u64| s.threads.values().filter(|t| t.project == id && !t.deleted).map(|t| t.activity.max(t.created)).max().unwrap_or(created);
    let set = |s: &'a State, k: String| s.settings.get(&k).map(String::as_str).unwrap_or("");
    // this machine's, oldest first (so of two local projects of one repository the older names the row)
    let mut lp: Vec<_> = st.projects.iter().filter(|(id, p)| !p.removed && id.as_str() != crate::sidebar::SCRATCH).collect();
    lp.sort_by(|a, b| a.1.created.cmp(&b.1.created).then(a.0.cmp(b.0)));
    let keys = local_keys(st, here, local_origin, local_rel);
    for (id, p) in lp {
        let o = local_origin(&p.root);
        let was = key_of(&o, set(st, format!("project.link.{id}")), "", here, id);
        put(keys[id].clone(), Copy { machine: String::new(), id: id.clone(), st: Some(st), name: p.title.clone(), folder: p.root.clone(), origin: o, activity: live_act(st, id, p.created), was });
    }
    let live_names: Vec<&str> = far.iter().map(|(n, _)| n.as_str()).collect();
    for (m, f) in far {
        let mut fp: Vec<_> = f.projects.iter().filter(|(id, p)| !p.removed && !id.ends_with(&format!("~{}", crate::sidebar::SCRATCH)) && id.as_str() != crate::sidebar::SCRATCH).collect();
        fp.sort_by(|a, b| a.1.created.cmp(&b.1.created).then(a.0.cmp(b.0)));
        // (one machine's copies never merge with each other: `keys_of`'s rule, applied to the keys as this hub learns them)
        let mut taken = std::collections::HashSet::new();
        for (id, p) in fp {
            let local = id.rsplit('~').next().unwrap_or(id);
            let mut o = origin(id);
            let mut key = None;
            // its machine has not said its origin yet (a link just made): the registry's key for that copy, else it waits (never machine/id while the
            // origin is only unknown: the same repository would be two rows); a machine that does not answer is keyed by machine/id as it has no say
            if o == crate::mirror::UNKNOWN || o.is_empty() {
                // (the registry has the copy's key as that machine computed it: it wins over "none" too, which a machine that will not say gets)
                match reg.iter().find(|r| r.machines.iter().any(|c| c.machine == *m && c.id == local)) {
                    Some(r) => { o = r.origin.clone(); key = Some(r.key.clone()); }
                    None if o.is_empty() || away(m) => { o = String::new(); }
                    None => continue,
                }
            }
            let link = set(f, format!("project.link.{local}"));
            let was = key_of(&o, link, m, here, id);
            let mut key = key.unwrap_or_else(|| key_of(&ident(&o, &rel(id)), link, m, here, id));
            if taken.contains(&key) { key = key_of("", link, m, here, id); if taken.contains(&key) { key = key_of("", "", m, here, id); } }
            taken.insert(key.clone());
            put(key, Copy { machine: m.clone(), id: id.clone(), st: Some(*f), name: p.title.clone(), folder: p.root.clone(), origin: o, activity: live_act(f, id, p.created), was });
        }
    }
    // the registry: only machines that are neither this one nor live (theirs come from their mirrors)
    let mut reg_threads: HashMap<String, Vec<&RegThread>> = HashMap::new();
    for r in reg {
        for c in &r.machines {
            if c.machine == here || live_names.contains(&c.machine.as_str()) { continue }
            let act = r.threads.iter().filter(|t| t.machine == c.machine).map(|t| t.activity).max().unwrap_or(c.seen);
            put(r.key.clone(), Copy { machine: c.machine.clone(), id: c.id.clone(), st: None, name: r.name.clone(), folder: c.folder.clone(), origin: r.origin.clone(), activity: act, was: if r.origin.is_empty() { r.key.clone() } else { r.origin.clone() } });
            reg_threads.entry(r.key.clone()).or_default().extend(r.threads.iter().filter(|t| t.machine == c.machine));
        }
    }
    let mut out: Vec<MergedProject<'a>> = by.into_iter().map(|(key, mut cs)| {
        // this machine first, then the livelier
        cs.sort_by(|a, b| (!a.machine.is_empty()).cmp(&!b.machine.is_empty()).then(b.activity.cmp(&a.activity)));
        let lead = &cs[0];
        let newest = cs.iter().max_by_key(|c| c.activity).unwrap();
        let (name, folder) = if lead.machine.is_empty() { (lead.name.clone(), lead.folder.clone()) } else { (newest.name.clone(), newest.folder.clone()) };
        let origin = cs.iter().map(|c| c.origin.clone()).find(|o| !o.is_empty()).unwrap_or_default();
        let settings_of = cs.iter().find(|c| c.st.is_some()).map(|c| (c.st.unwrap(), c.id.clone()));
        let (icon, color, css) = match &settings_of {
            Some((s, id)) => { let pick = set(s, format!("project.pick.{id}")); let c = crate::present::badge::color_eff(set(s, format!("project.color.{id}")), pick, id).to_string();
                (crate::present::badge::icon_eff(set(s, format!("project.icon.{id}")), pick, &name), c.clone(), crate::present::badge::css_of(&c)) }
            None => { let c = crate::present::badge::color_eff("", "", &key).to_string(); (crate::present::badge::icon_eff("", "", &name), c.clone(), crate::present::badge::css_of(&c)) }
        };
        let mut machines = vec![]; let mut seen = std::collections::HashSet::new();
        let mut threads = vec![];
        for c in &cs {
            if seen.insert(c.machine.clone()) {
                let state = if c.machine.is_empty() { "here" } else if c.st.is_none() { "off" } else if away(&c.machine) { "away" } else { "on" };
                machines.push(chip(&c.machine, here, state));
            }
            if let Some(s) = c.st { for (tid, t) in s.threads.iter().filter(|(_, t)| t.project == c.id && !t.deleted) {
                threads.push(MergedThread { machine: c.machine.clone(), id: tid.clone(), title: t.title.clone(), activity: t.activity.max(t.created), pinned: t.pinned, archived: t.archived, src: Some(s), status: String::new() });
            } }
        }
        for t in reg_threads.remove(&key).unwrap_or_default() {
            threads.push(MergedThread { machine: t.machine.clone(), id: t.id.clone(), title: t.title.clone(), activity: t.activity, pinned: t.pinned, archived: false, src: None, status: t.status.clone() });
        }
        threads.sort_by(|a, b| b.activity.cmp(&a.activity).then(a.id.cmp(&b.id)));
        let last_activity = cs.iter().map(|c| c.activity).max().unwrap_or(0);
        let here_id = cs.iter().find(|c| c.machine.is_empty()).map(|c| c.id.clone());
        // the 0.16.0 key of the copy the settings are read from
        let was = settings_of.as_ref().and_then(|(_, id)| cs.iter().find(|c| &c.id == id)).map(|c| c.was.clone()).unwrap_or_default();
        MergedProject { key, was, name, origin, folder, here: here_id, ids: cs.iter().map(|c| (c.machine.clone(), c.id.clone())).collect(), machines, threads, last_activity, icon, color, css, settings_of }
    }).collect();
    out.sort_by(|a, b| b.last_activity.cmp(&a.last_activity).then(a.key.cmp(&b.key)));
    out
}

/// `merged_projects` over this hub's mirrors and the registry the home sent last (`net::registry::cached`): `f` gets the list (it borrows the
/// mirrors' lock, so it is handed in rather than returned).
pub fn merged_of<R>(st: &State, m: &crate::mirror::Mirrors, f: impl FnOnce(Vec<MergedProject>) -> R) -> R {
    let reg = crate::net::registry::cached();
    let here = m.here();
    m.shown(|far| { let mut list = merged_projects_in(st, far, &|n| m.mark(n)["away"] == true, &|id| m.origin(id), &|id| m.rel(id), &here, &reg, &|r| crate::mirror::origin_of_root(r), &|r| crate::mirror::repo_rel_of_root(r)); for p in list.iter_mut() { crate::mirror::live_chips(m, &mut p.machines) } f(list) })
}

/// The sidebar's join (present/screen.rs, data/1): which project id draws each merged row (its owner: this machine's copy, else the copy of the
/// first shown machine that has it, `far` order, then the lowest id: stable while activity moves) and every other live copy -> that owner.
/// `rows` is per owner: `key`, `name`, `origin`, `machineChips` (every machine with a copy, `machines`' chips) and `off` (the copies only the
/// registry knows: `{machine, id, folder}`, and their threads as `{machine, id, title, status, activity, pinned}`).
pub struct Join { pub far: HashMap<String, String>, pub rows: HashMap<String, Value> }
pub fn join_of(list: &[MergedProject], far_order: &[String]) -> Join {
    let (mut far, mut rows) = (HashMap::new(), HashMap::new());
    let rank = |m: &str| far_order.iter().position(|n| n == m).unwrap_or(usize::MAX);
    for p in list {
        let live: Vec<&(String, String)> = p.ids.iter().filter(|(m, _)| m.is_empty() || far_order.iter().any(|n| n == m)).collect();
        let Some(owner) = live.iter().min_by(|a, b| (!a.0.is_empty(), rank(&a.0), &a.1).cmp(&(!b.0.is_empty(), rank(&b.0), &b.1))).map(|x| x.1.clone()) else { continue };
        for (_, id) in &live { if *id != owner { far.insert(id.clone(), owner.clone()); } }
        let off_ids: Vec<Value> = p.ids.iter().filter(|c| !live.contains(c)).map(|(m, i)| json!({"machine":m,"id":i})).collect();
        let off_threads: Vec<Value> = p.threads.iter().filter(|t| t.src.is_none()).map(|t| json!({"machine":t.machine,"id":t.id,"title":t.title,"status":t.status,"activity":t.activity,"pinned":t.pinned})).collect();
        // where new work on it runs by default (D6): this machine's copy, else the most recently active live machine's (`ids` is in that order)
        let default = live.first().map(|(m, i)| json!({"machine":m,"id":i})).unwrap_or(Value::Null);
        rows.insert(owner, json!({"key":p.key,"name":p.name,"origin":p.origin,"machineChips":p.machines,"default":default,"ids":live.iter().map(|(m, i)| json!({"machine":m,"id":i})).collect::<Vec<_>>(),"off":{"copies":off_ids,"threads":off_threads}}));
    }
    Join { far, rows }
}
/// Only the registry's projects none of whose machines is live (every copy off): drawn as rows of their own.
pub fn all_off<'a, 'b>(list: &'b [MergedProject<'a>]) -> Vec<&'b MergedProject<'a>> { list.iter().filter(|p| p.settings_of.is_none() && !p.ids.is_empty()).collect() }

/// A merged row as JSON (data/1 `projects.list`'s merged fields, and what the sections draw): `key, name, origin, folder, here, ids, machines,
/// lastActivity, icon, color, css, threadCount`. Threads are not in it (they page separately).
pub fn row_json(p: &MergedProject) -> Value {
    json!({"key":p.key,"name":p.name,"origin":p.origin,"folder":p.folder,"here":p.here,"ids":p.ids.iter().map(|(m, i)| json!({"machine":m,"id":i})).collect::<Vec<_>>(),
        "machines":p.machines,"lastActivity":p.last_activity,"icon":p.icon,"color":p.color,"css":p.css,"threadCount":p.threads.iter().filter(|t| !t.archived).count()})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn st(evs: &[Value]) -> State { let mut s = State::new(); s.lean = true; for e in evs { s.apply(e); } s }
    fn pc(id: &str, title: &str, root: &str, at: u64) -> Value { json!({"$":"ProjectCreated","id":id,"title":title,"root":root,"at":at}) }
    fn tc(id: &str, p: &str, at: u64) -> Value { json!({"$":"ThreadCreated","id":id,"project":p,"title":format!("T {id}"),"at":at}) }

    #[test] fn one_row_per_key_across_this_machine_linked_machines_and_the_registry() {
        let here = st(&[pc("p1", "Local", "", 1), tc("t1", "p1", 10)]);
        // two linked machines with the same repository and none here: one row, not "Backplane on framework" and "Backplane on boat-hub"
        let fw = st(&[pc("L~p7", "Backplane", "/fw/bp", 2), tc("L~t7", "L~p7", 30), pc("L~p8", "Notes", "/fw/n", 2)]);
        let bh = st(&[pc("M~p3", "Backplane", "/bh/bp", 3), tc("M~t3", "M~p3", 20)]);
        let far = vec![("framework".to_string(), &fw), ("boat-hub".to_string(), &bh)];
        let origin = |id: &str| if id == "L~p7" || id == "M~p3" { "github.com/o/bp".to_string() } else { String::new() };
        let reg = vec![RegProject { key: "github.com/o/bp".into(), name: "Backplane".into(), origin: "github.com/o/bp".into(),
            machines: vec![RegMachine { machine: "desktop".into(), id: "p9".into(), folder: "/d/bp".into(), seen: 5 }, RegMachine { machine: "framework".into(), id: "p7".into(), folder: "/fw/bp".into(), seen: 5 }],
            threads: vec![RegThread { machine: "desktop".into(), id: "desktop~t9".into(), title: "off one".into(), status: "needs".into(), activity: 40, pinned: false }, RegThread { machine: "framework".into(), id: "framework~t7".into(), title: "dup".into(), status: "idle".into(), activity: 30, pinned: false }] }];
        let v = merged_projects(&here, &far, &|m| m == "boat-hub", &origin, "laptop", &reg);
        let keys: Vec<&str> = v.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, ["github.com/o/bp", "laptop/p1", "framework/p8"], "newest first; no-origin projects stay per machine");
        let bp = &v[0];
        assert_eq!(bp.name, "Backplane"); assert_eq!(bp.here, None); assert_eq!(bp.folder, "/d/bp", "the newest copy's folder (desktop's, from the registry)");
        let states: Vec<(&str, &str, bool)> = bp.machines.iter().map(|m| (m["short"].as_str().unwrap(), m["state"].as_str().unwrap(), m["dim"].as_bool().unwrap())).collect();
        assert_eq!(states, [("desktop", "off", true), ("framework", "on", false), ("boat-hub", "away", true)]);
        assert_eq!(bp.machines[0]["note"], "desktop is off");
        let ts: Vec<(&str, &str, bool)> = bp.threads.iter().map(|t| (t.id.as_str(), t.machine.as_str(), t.src.is_some())).collect();
        assert_eq!(ts, [("desktop~t9", "desktop", false), ("L~t7", "framework", true), ("M~t3", "boat-hub", true)], "a live machine's registry rows are not doubled");
        assert_eq!(bp.threads[0].status, "needs");
        assert_eq!(v[1].here.as_deref(), Some("p1")); assert_eq!(v[1].machines[0]["here"], true);
        // the join: framework's copy draws the row (first shown machine), boat-hub's joins it, desktop's is off
        let j = join_of(&v, &["framework".to_string(), "boat-hub".to_string()]);
        assert_eq!(j.far.get("M~p3").map(String::as_str), Some("L~p7")); assert!(!j.far.contains_key("L~p7"));
        let r = &j.rows["L~p7"]; assert_eq!(r["key"], "github.com/o/bp"); assert_eq!(r["off"]["copies"], json!([{"machine":"desktop","id":"p9"}]));
        assert_eq!(r["off"]["threads"][0]["id"], "desktop~t9"); assert_eq!(r["machineChips"].as_array().unwrap().len(), 3);
        assert!(all_off(&v).is_empty());
        let only_off = merged_projects(&here, &[], &|_| false, &|_| String::new(), "laptop", &reg);
        assert_eq!(all_off(&only_off).len(), 1, "with every machine off, the registry's project is still a row");
    }
    /// A linked machine whose origins are not known yet (the link was just made): its copy takes the registry's key, else it waits; it is never
    /// keyed machine/id (the same repository twice on a hub that has its own copy). One that does not answer is keyed by machine/id.
    #[test] fn a_copy_whose_origin_is_not_known_yet_takes_the_registrys_key_or_waits() {
        let here = st(&[pc("p1", "demo", "", 1)]);
        let a = st(&[pc("L~pA1", "demo", "/a/demo", 2), tc("L~t1", "L~pA1", 9)]);
        let far = vec![("desktop".to_string(), &a)];
        let unknown = |_: &str| crate::mirror::UNKNOWN.to_string();
        let mine = |id: &str| if id == "p1" { "github.com/o/demo".to_string() } else { String::new() };
        let _ = mine;
        let v = merged_projects(&here, &far, &|_| false, &unknown, "boat-hub", &[]);
        assert!(v.iter().all(|p| p.ids.iter().all(|(m, _)| m.is_empty())), "waits: no desktop/pA1 row: {v:?}");
        let reg = vec![RegProject { key: "laptop/p1".into(), name: "demo".into(), origin: "".into(), machines: vec![RegMachine { machine: "desktop".into(), id: "pA1".into(), folder: "/a/demo".into(), seen: 1 }], threads: vec![] }];
        let v = merged_projects(&here, &far, &|_| false, &unknown, "boat-hub", &reg);
        assert!(v.iter().any(|p| p.key == "laptop/p1" && p.ids.iter().any(|(_, i)| i == "L~pA1")), "the registry's key: {v:?}");
        let v = merged_projects(&here, &far, &|_| true, &unknown, "boat-hub", &[]);
        assert!(v.iter().any(|p| p.key == "desktop/pA1"), "away and not in the registry: machine/id");
    }
    #[test] fn this_machine_names_the_row_and_a_hand_link_joins_projects_with_no_remote() {
        let here = st(&[pc("p1", "My name", "", 1), json!({"$":"SettingSet","key":"project.link.p1","value":"framework/p8","at":1}), json!({"$":"SettingSet","key":"project.icon.p1","value":"cpu","at":1})]);
        let fw = st(&[pc("L~p8", "Their name", "/fw/n", 2), tc("L~t8", "L~p8", 50)]);
        let far = vec![("framework".to_string(), &fw)];
        let v = merged_projects(&here, &far, &|_| false, &|_| String::new(), "laptop", &[]);
        assert_eq!(v.len(), 1, "{v:?}"); assert_eq!(v[0].key, "framework/p8"); assert_eq!(v[0].name, "My name"); assert_eq!(v[0].here.as_deref(), Some("p1"));
        assert_eq!(v[0].ids, [("".to_string(), "p1".to_string()), ("framework".to_string(), "L~p8".to_string())]);
        assert_eq!(v[0].settings_of.as_ref().map(|(_, i)| i.as_str()), Some("p1"), "pins and icons carry from this machine's copy");
        assert_eq!(v[0].last_activity, 50);
        let mut fw2 = st(&[pc("L~p8", "Their name", "/fw/n", 2), tc("L~t8", "L~p8", 50), tc("L~t9", "L~p8", 60)]); fw2.apply(&json!({"$":"ThreadArchived","id":"L~t9","archived":true,"at":61}));
        let far2 = vec![("framework".to_string(), &fw2)];
        let v2 = merged_projects(&here, &far2, &|_| false, &|_| String::new(), "laptop", &[]);
        assert_eq!(v2[0].threads.iter().map(|t| (t.id.as_str(), t.archived)).collect::<Vec<_>>(), [("L~t9", false), ("L~t8", false)], "an archive is a settle now: the thread is listed");
        assert_eq!(row_json(&v2[0])["threadCount"], 2);
        assert_eq!(row_json(&v[0])["machines"][1]["state"], "on");
    }

    /// #1168: one repository with three projects (its top and two boards' folders) on one machine is three rows; the same folder on another
    /// machine joins its row, a folder only there is its own; two clones of one folder on one machine stay apart (the older keeps the key).
    #[test] fn projects_in_folders_of_one_repository_are_rows_of_their_own_and_merge_only_with_the_same_folder_elsewhere() {
        let here = st(&[pc("p1", "repo", "/r", 1), pc("p2", "Board A", "/r/boards/a", 2), pc("p3", "Board B", "/r/boards/b", 3), pc("p4", "repo clone", "/r2", 4),
            tc("t1", "p1", 40), tc("t2", "p2", 10), tc("t3", "p3", 20), tc("t4", "p4", 5)]);
        let lo = |r: &str| if r.starts_with("/r") { "example.org/acme/repo".to_string() } else { String::new() };
        let lr = |r: &str| r.strip_prefix("/r2").or_else(|| r.strip_prefix("/r")).unwrap_or("").trim_start_matches('/').to_string();
        let v = merged_projects_in(&here, &[], &|_| false, &|_| String::new(), &|_| String::new(), "desk", &[], &lo, &lr);
        let mut keys: Vec<(&str, &str)> = v.iter().map(|p| (p.name.as_str(), p.key.as_str())).collect(); keys.sort();
        assert_eq!(keys, [("Board A", "example.org/acme/repo/boards/a"), ("Board B", "example.org/acme/repo/boards/b"), ("repo", "example.org/acme/repo"), ("repo clone", "desk/p4")]);
        for p in &v { assert_eq!(p.threads.len(), 1, "each its own threads: {}", p.name); assert_eq!(p.was, "example.org/acme/repo", "the 0.16.0 key: {}", p.name); }
        assert_eq!(local_keys(&here, "desk", &lo, &lr)["p2"], "example.org/acme/repo/boards/a", "what machine_list and the registry rows say");
        // the laptop has the repository too: its top and Board A join those rows, its board C is a row of its own; its second clone of boards/a stays apart
        let lap = st(&[pc("L~q1", "repo", "/home/x/repo", 1), pc("L~q2", "A", "/home/x/repo/boards/a", 2), pc("L~q3", "C", "/home/x/repo/boards/c", 3), pc("L~q4", "A again", "/home/x/other/boards/a", 4),
            tc("L~u2", "L~q2", 50)]);
        let far = vec![("laptop".to_string(), &lap)];
        let fo = |_: &str| "example.org/acme/repo".to_string();
        let fr = |id: &str| match id { "L~q2" | "L~q4" => "boards/a".to_string(), "L~q3" => "boards/c".to_string(), _ => String::new() };
        let v = merged_projects_in(&here, &far, &|_| false, &fo, &fr, "desk", &[], &lo, &lr);
        let row = |k: &str| v.iter().find(|p| p.key == k).unwrap_or_else(|| panic!("{k}: {v:?}"));
        assert_eq!(row("example.org/acme/repo").ids, [("".to_string(), "p1".to_string()), ("laptop".to_string(), "L~q1".to_string())]);
        let a = row("example.org/acme/repo/boards/a");
        assert_eq!(a.ids, [("".to_string(), "p2".to_string()), ("laptop".to_string(), "L~q2".to_string())]);
        assert_eq!((a.name.as_str(), a.threads.len()), ("Board A", 2), "this machine names it; both machines' threads");
        assert_eq!(row("example.org/acme/repo/boards/c").ids, [("laptop".to_string(), "L~q3".to_string())]);
        assert_eq!(row("laptop/q4").ids, [("laptop".to_string(), "L~q4".to_string())], "one machine's clones never merge");
        assert_eq!(v.len(), 6);
        // a laptop of 0.16.0 says no folder: its copies key by the origin alone, so only its top's merge as before
        let v = merged_projects_in(&here, &far, &|_| false, &fo, &|_| String::new(), "desk", &[], &lo, &lr);
        assert!(v.iter().any(|p| p.key == "example.org/acme/repo" && p.ids.len() == 2));
        assert_eq!(ident("o/r", ""), "o/r"); assert_eq!(ident("o/r", "/b/a/"), "o/r/b/a"); assert_eq!(ident("", "b"), "");
        assert_eq!(rel_in_key("o/r", "o/r/b/a"), "b/a"); assert_eq!(rel_in_key("o/r", "o/r"), ""); assert_eq!(rel_in_key("o/r", "o/rx/b"), ""); assert_eq!(rel_in_key("", "desk/p1"), "");
    }
    /// The pins, groups, quiet marks and folds kept under the 0.16.0 key (the origin, which the repository's projects shared) are carried to each
    /// project's key now, once (`sidebar.keys.moved.<key>`), never over one set since; the repository's top keeps its key and needs nothing.
    #[test] fn what_was_set_under_the_shared_key_is_carried_to_each_projects_key_once() {
        let o = "example.org/acme/repo";
        let mut evs = vec![pc("p1", "repo", "/r", 1), pc("p2", "Board A", "/r/boards/a", 2), pc("p3", "Board B", "/r/boards/b", 3)];
        for (k, v) in [("sidebar.pin.", "77"), ("sidebar.group.of.", "custom:acme"), ("sidebar.archived.", "66"), ("sidebar.fold.", "55")] { evs.push(json!({"$":"SettingSet","key":format!("{k}{o}"),"value":v,"at":5})); }
        evs.push(json!({"$":"SettingSet","key":format!("sidebar.group.of.{o}/boards/b"),"value":"custom:mine","at":6}));
        let mut s = st(&evs);
        let lo = |r: &str| if r.starts_with("/r") { o.to_string() } else { String::new() };
        let lr = |r: &str| r.strip_prefix("/r").unwrap_or("").trim_start_matches('/').to_string();
        let moves = |s: &State| { let v = merged_projects_in(s, &[], &|_| false, &|_| String::new(), &|_| String::new(), "desk", &[], &lo, &lr); let mut m = key_moves(s, &v); m.sort(); m };
        let m = moves(&s);
        let a = format!("{o}/boards/a"); let b = format!("{o}/boards/b");
        let want: Vec<(String, String)> = { let mut w = vec![
            (format!("sidebar.pin.{a}"), "77".to_string()), (format!("sidebar.group.of.{a}"), "custom:acme".into()), (format!("sidebar.archived.{a}"), "66".into()), (format!("sidebar.fold.{a}"), "55".into()), (format!("sidebar.keys.moved.{a}"), o.into()),
            (format!("sidebar.pin.{b}"), "77".into()), (format!("sidebar.archived.{b}"), "66".into()), (format!("sidebar.fold.{b}"), "55".into()), (format!("sidebar.keys.moved.{b}"), o.into())]; w.sort(); w };
        assert_eq!(m, want, "Board B keeps the group set on it since");
        for (k, v) in &m { s.apply(&json!({"$":"SettingSet","key":k,"value":v,"at":9})); }
        assert!(moves(&s).is_empty(), "once");
        // the person unpins Board A afterwards: the old pin (the repository's top's own now) is not carried again
        s.apply(&json!({"$":"SettingSet","key":format!("sidebar.pin.{a}"),"value":"","at":10}));
        assert!(moves(&s).is_empty());
        assert_eq!(s.settings.get(&format!("sidebar.pin.{o}")).map(String::as_str), Some("77"), "nothing is removed: the top keeps its pin");
    }
}

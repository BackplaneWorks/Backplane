//! One project list over machines (docs/ALWAYS-ON.md section 5): every project of this hub, of each linked machine's mirror and of the
//! registry (machines that are off) folded into one row per key. The key is the project's normalized git origin (`mirror::normalize_origin`,
//! D5); a project with no origin is `<machine>/<id>`, unless a hand link (`project.link.<id>` = another project's key) joins it to another.
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
    /// the normalized origin, or `<machine>/<id>`, or the key a hand link names
    pub key: String,
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
    m["note"] = json!(match state { "off" => format!("{short} is off"), "away" => format!("{short} is away"), _ => String::new() });
    m
}

/// A project's key: its origin; else a hand link's key; else `<machine>/<id>` (`machine` "" is this one, named `here`).
pub fn key_of(origin: &str, link: &str, machine: &str, here: &str, id: &str) -> String {
    if !origin.is_empty() { return origin.to_string() }
    if !link.is_empty() { return link.to_string() }
    let local = id.rsplit('~').next().unwrap_or(id);
    format!("{}/{local}", short_name(if machine.is_empty() { if here.is_empty() { "here" } else { here } } else { machine }))
}

/// The merged list. `st` is this hub's state, `far` the shown linked machines' mirrors `(machine, state)`, `away(machine)` whether a linked machine
/// did not answer, `origin(far id)` a mirrored project's origin (`Mirrors::origin`), `here` this machine's name, `reg` the registry's projects (a
/// machine in it that is neither here nor in `far` is off). Sorted by last activity, newest first.
pub fn merged_projects<'a>(st: &'a State, far: &[(String, &'a State)], away: &dyn Fn(&str) -> bool, origin: &dyn Fn(&str) -> String, here: &str, reg: &[RegProject]) -> Vec<MergedProject<'a>> {
    struct Copy<'a> { machine: String, id: String, st: Option<&'a State>, name: String, folder: String, origin: String, activity: u64 }
    let mut by: Vec<(String, Vec<Copy<'a>>)> = vec![];
    let mut slot: HashMap<String, usize> = HashMap::new();
    let mut put = |key: String, c: Copy<'a>| { let i = *slot.entry(key.clone()).or_insert_with(|| { by.push((key, vec![])); by.len() - 1 }); by[i].1.push(c) };
    let live_act = |s: &State, id: &str, created: u64| s.threads.values().filter(|t| t.project == id && !t.deleted).map(|t| t.activity.max(t.created)).max().unwrap_or(created);
    let set = |s: &'a State, k: String| s.settings.get(&k).map(String::as_str).unwrap_or("");
    // this machine's, oldest first (so of two local projects of one repository the older names the row)
    let mut lp: Vec<_> = st.projects.iter().filter(|(id, p)| !p.removed && id.as_str() != crate::sidebar::SCRATCH).collect();
    lp.sort_by(|a, b| a.1.created.cmp(&b.1.created).then(a.0.cmp(b.0)));
    for (id, p) in lp {
        let o = crate::mirror::origin_of_root(&p.root);
        put(key_of(&o, set(st, format!("project.link.{id}")), "", here, id), Copy { machine: String::new(), id: id.clone(), st: Some(st), name: p.title.clone(), folder: p.root.clone(), origin: o, activity: live_act(st, id, p.created) });
    }
    let live_names: Vec<&str> = far.iter().map(|(n, _)| n.as_str()).collect();
    for (m, f) in far {
        let mut fp: Vec<_> = f.projects.iter().filter(|(id, p)| !p.removed && !id.ends_with(&format!("~{}", crate::sidebar::SCRATCH)) && id.as_str() != crate::sidebar::SCRATCH).collect();
        fp.sort_by(|a, b| a.1.created.cmp(&b.1.created).then(a.0.cmp(b.0)));
        for (id, p) in fp {
            let o = origin(id);
            put(key_of(&o, set(f, format!("project.link.{}", id.rsplit('~').next().unwrap_or(id))), m, here, id), Copy { machine: m.clone(), id: id.clone(), st: Some(*f), name: p.title.clone(), folder: p.root.clone(), origin: o, activity: live_act(f, id, p.created) });
        }
    }
    // the registry: only machines that are neither this one nor live (theirs come from their mirrors)
    let mut reg_threads: HashMap<String, Vec<&RegThread>> = HashMap::new();
    for r in reg {
        for c in &r.machines {
            if c.machine == here || live_names.contains(&c.machine.as_str()) { continue }
            let act = r.threads.iter().filter(|t| t.machine == c.machine).map(|t| t.activity).max().unwrap_or(c.seen);
            put(r.key.clone(), Copy { machine: c.machine.clone(), id: c.id.clone(), st: None, name: r.name.clone(), folder: c.folder.clone(), origin: r.origin.clone(), activity: act });
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
        MergedProject { key, name, origin, folder, here: here_id, ids: cs.iter().map(|c| (c.machine.clone(), c.id.clone())).collect(), machines, threads, last_activity, icon, color, css, settings_of }
    }).collect();
    out.sort_by(|a, b| b.last_activity.cmp(&a.last_activity).then(a.key.cmp(&b.key)));
    out
}

/// `merged_projects` over this hub's mirrors and the registry the home sent last (`net::registry::cached`): `f` gets the list (it borrows the
/// mirrors' lock, so it is handed in rather than returned).
pub fn merged_of<R>(st: &State, m: &crate::mirror::Mirrors, f: impl FnOnce(Vec<MergedProject>) -> R) -> R {
    let reg = crate::net::registry::cached();
    let here = m.here();
    m.shown(|far| f(merged_projects(st, far, &|n| m.mark(n)["away"] == true, &|id| m.origin(id), &here, &reg)))
}

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
        assert_eq!(v2[0].threads.iter().map(|t| (t.id.as_str(), t.archived)).collect::<Vec<_>>(), [("L~t9", true), ("L~t8", false)], "archived threads are kept and marked");
        assert_eq!(row_json(&v2[0])["threadCount"], 1);
        assert_eq!(row_json(&v[0])["machines"][1]["state"], "on");
    }
}

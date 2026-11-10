//! Your machines on a phone (docs/PHONE-DATA.md, "Your machines"): the linked machines' projects and threads in the same lists as this hub's own, from the
//! display-only mirrors (`Hub::mirrors().shown`, never the policy state). The decisions are here and pure (the apps hold none): which machine is picked,
//! which of a linked machine's projects is the same repository as one of this hub's (`present::screen::unified_of`, as the desktop sidebar), the rows with
//! their machine marks, the switcher's chips and where a new thread can start. One list over machines is ordered by recency, newest first.
use crate::present::screen::{machine_switch, new_on, Unified};
use crate::state::{State, Turn};
use serde_json::{json, Value};
use std::collections::HashMap;

pub type Far<'a> = [(String, &'a State)];

/// What the switcher has chosen: "" every machine, "@here" this one, or a shown machine's name (a name that is not shown any more is every machine again).
pub fn pick_of(want: &str, far: &Far) -> String {
    if want == "@here" || far.iter().any(|(n, _)| n == want) { want.to_string() } else { String::new() }
}
/// The machines a pick shows linked threads of.
pub fn shown_far<'a, 'b>(far: &'b Far<'a>, pick: &str) -> Vec<&'b (String, &'a State)> { far.iter().filter(|(n, _)| pick.is_empty() || n == pick).collect() }
/// Whether this machine's own rows show.
pub fn here_on(pick: &str) -> bool { pick.is_empty() || pick == "@here" }

/// The switcher chips (`{value,label,on,away,hint}`, the desktop's `machine_switch`): empty with no linked machine, so a phone draws none.
pub fn chips(far: &Far, un: &Unified, pick: &str) -> Value { if far.is_empty() { json!([]) } else { machine_switch(far, un, pick) } }

/// A linked project's id -> this hub's project of the same repository; none when a machine is picked (its projects keep their own rows then).
pub fn join_map<'a>(joined: &'a HashMap<String, String>, pick: &str) -> Option<&'a HashMap<String, String>> { if pick.is_empty() { Some(joined) } else { None } }

fn setting<'a>(st: &'a State, k: &str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or("") }
fn ms(at: u64) -> u64 { super::ms(at) }

/// A linked machine's thread as the list rows have it. `project` is the id the phone files it under (this hub's project when the repository is one).
pub fn thread_json(id: &str, st: &State, project: &str, mark: &Value, preview: &str) -> Option<Value> {
    let t = st.threads.get(id).filter(|t| !t.deleted)?;
    let waiting = st.asks.values().filter(|(th, open)| *open && th == id).count() as u64;
    let state = if waiting > 0 { "asking" } else { match t.turn { Turn::Running => "working", Turn::Failed => "failed", _ => "idle" } };
    let (pinned, settled, settled_at) = super::shelf_flags(t);
    let status = if waiting > 0 { "waiting" } else if t.turn == Turn::Running { "active" } else if settled { "settled" } else { "" };
    let runtime = st.runtime.get(id).cloned().unwrap_or_default();
    Some(json!({"id":id,"project":project,"title":t.title,"state":state,"updatedAt":ms(t.activity.max(t.created)),"pinned":pinned,"settled":settled,"settledAt":settled_at,"archived":t.archived,
        "provider":t.provider,"model":t.model,"effort":"","runtime":runtime,"chip":super::ext::chip(&t.provider, &t.model, ""),"preview":preview,"waiting":waiting,"status":status,
        "parent":setting(st, &format!("thread.parent.{id}")),"history":false,"mark":mark}))
}

/// One of a linked machine's projects as a row of its own (not one with a project here): counts of what this machine holds, its mark, and where a new thread starts.
pub fn project_json(id: &str, st: &State, mark: &Value, un: &Unified, machine: &str) -> Option<Value> {
    let p = st.projects.get(id).filter(|p| !p.removed)?;
    let ts: Vec<_> = st.threads.values().filter(|t| t.project == id && !t.deleted).collect();
    let live: Vec<_> = ts.iter().filter(|t| !t.archived).collect();
    let settled = live.iter().filter(|t| super::shelf_flags(t).1).count();
    let updated = ts.iter().map(|t| t.activity.max(t.created)).max().unwrap_or(p.created);
    let waiting = st.asks.values().filter(|(th, open)| *open && st.threads.get(th).map_or(false, |t| t.project == id)).count();
    let pick = setting(st, &format!("project.pick.{id}"));
    let color = crate::present::badge::color_eff(setting(st, &format!("project.color.{id}")), pick, id).to_string();
    Some(json!({"id":id,"title":p.title,"root":p.root,"origin":"","threadCount":live.len(),"settledCount":settled,"updatedAt":ms(updated),"waiting":waiting,
        "icon":crate::present::badge::icon_eff(setting(st, &format!("project.icon.{id}")), pick),"css":crate::present::badge::css_of(&color),"color":color,
        "machine":machine,"mark":mark,"newOn":new_on_far(id, machine, mark, un)}))
}
/// A project that is only on a linked machine starts its new threads there.
fn new_on_far(id: &str, _machine: &str, mark: &Value, _un: &Unified) -> Value {
    let short = mark["short"].as_str().unwrap_or("");
    json!([{"value":id,"label":format!("New thread on {short}"),"away":mark["away"]}])
}

/// What a linked machine holds of one of this hub's projects.
pub struct Partner { pub machine: String, pub id: String, pub count: u64, pub settled: u64, pub updated: u64, pub waiting: u64 }
/// The partners of this hub's projects: `join` maps a linked project to this hub's.
pub fn partners(far: &Far, pick: &str, join: &HashMap<String, String>) -> HashMap<String, Vec<Partner>> {
    let mut out: HashMap<String, Vec<Partner>> = HashMap::new();
    for (m, st) in shown_far(far, pick) {
        for (fid, lid) in join {
            if !st.projects.get(fid).map_or(false, |p| !p.removed) { continue }
            let ts: Vec<_> = st.threads.values().filter(|t| t.project == *fid && !t.deleted).collect();
            let live: Vec<_> = ts.iter().filter(|t| !t.archived).collect();
            let waiting = st.asks.values().filter(|(th, open)| *open && st.threads.get(th).map_or(false, |t| t.project == *fid)).count() as u64;
            out.entry(lid.clone()).or_default().push(Partner { machine: m.clone(), id: fid.clone(), count: live.len() as u64, settled: live.iter().filter(|t| super::shelf_flags(t).1).count() as u64,
                updated: ms(ts.iter().map(|t| t.activity.max(t.created)).max().unwrap_or(0)), waiting });
        }
    }
    out
}
/// This hub's project row with the linked machines' share of the same repository: their threads count in it, it is as recent as the livelier of them, it says
/// "also on desktop" and offers a new thread on each machine.
pub fn join_project(row: &mut Value, ps: &[Partner], un: &Unified) {
    if ps.is_empty() { return }
    let add = |row: &mut Value, k: &str, n: u64| { row[k] = json!(row[k].as_u64().unwrap_or(0) + n) };
    let (mut marks, mut seen) = (vec![], std::collections::HashSet::new());
    for p in ps {
        add(row, "threadCount", p.count); add(row, "settledCount", p.settled); add(row, "waiting", p.waiting);
        row["updatedAt"] = json!(row["updatedAt"].as_u64().unwrap_or(0).max(p.updated));
        if seen.insert(p.machine.clone()) { marks.push(un.marks.get(&p.machine).cloned().unwrap_or_else(|| crate::mirror::mark_of(&p.machine, false))) }
    }
    let local = row["id"].as_str().unwrap_or("").to_string();
    row["newOn"] = new_on(&local, ps.iter().map(|p| (p.machine.as_str(), p.id.as_str())), un);
    row["also"] = json!(crate::sidebar::also_label(&marks)); row["marks"] = json!(marks);
}

/// The linked threads to list: `(updatedAt, id)` for the machines `pick` shows, in a project (`project` here, or a linked one's own id; "" any), archived or not, settled
/// shelf or not (`want`: Some(true) only settled, Some(false) only the rest). A project of this hub also holds its partners' threads.
pub fn thread_keys(far: &Far, pick: &str, join: &HashMap<String, String>, project: &str, archived: bool, want: Option<bool>) -> Vec<(u64, String)> {
    let mut out = vec![];
    for (_, st) in shown_far(far, pick) {
        for (id, t) in &st.threads {
            if t.deleted || t.archived != archived || t.project == crate::sidebar::SCRATCH { continue }
            let shown = project_shown(&t.project, join);
            if !project.is_empty() && shown != project && t.project != project { continue }
            let (_, settled, at) = super::shelf_flags(t);
            if want.map_or(false, |w| w != settled) { continue }
            out.push((if want == Some(true) { at } else { ms(t.activity.max(t.created)) }, id.clone()));
        }
    }
    out
}
/// The project a linked thread is filed under: this hub's when it is the same repository, else its own.
pub fn project_shown(far_project: &str, join: &HashMap<String, String>) -> String { join.get(far_project).cloned().unwrap_or_else(|| far_project.to_string()) }

/// The linked project rows to list (those with no project here to join), with the machines' share folded into this hub's rows by the caller.
pub fn project_rows(far: &Far, pick: &str, join: &HashMap<String, String>, un: &Unified) -> Vec<(u64, String, Value)> {
    let mut out = vec![];
    for (m, st) in shown_far(far, pick) {
        let mark = un.marks.get(m).cloned().unwrap_or_else(|| crate::mirror::mark_of(m, false));
        for id in st.projects.keys() {
            if id == crate::sidebar::SCRATCH || join.contains_key(id) { continue }
            if let Some(v) = project_json(id, st, &mark, un, m) { out.push((v["updatedAt"].as_u64().unwrap_or(0), id.clone(), v)) }
        }
    }
    out
}
/// The mark of the machine that holds a linked thread (its id is `<link>~<id>`): that machine's, None when no shown machine holds it.
pub fn holder<'a>(far: &'a Far, id: &str) -> Option<&'a (String, &'a State)> { far.iter().find(|(_, st)| st.threads.get(id).map_or(false, |t| !t.deleted)) }

/// (the machine's link, the thread's folder over there: its worktree, else its project's root) of a thread of a linked machine, from its mirror.
pub fn far_root(hub: &crate::hub::Hub, id: &str) -> Option<(String, String)> {
    if !is_far(id) { return None }
    hub.mirrors().shown(|far| { let (_, st) = holder(far, id)?; let t = st.threads.get(id)?;
        let root = if !t.worktree.is_empty() { t.worktree.clone() } else { st.projects.get(&t.project)?.root.clone() };
        if root.is_empty() { None } else { Some((crate::net::far::link(id), root)) } })
}
/// A viewer request's params for a thread of a linked machine: `on` names the machine so the request goes there (net/rpc.rs, `far::params_link`).
pub fn on_link(mut q: Value, thread: &str) -> Value { if is_far(thread) { q["on"] = json!(crate::net::far::link(thread)) } q }
/// The listing text of an owner's `files.dir` reply: this hub's own words (`fmEntries`), or the far Files page's (`listing`).
pub fn listing_of(res: &Value) -> &str { res["fmEntries"].as_str().filter(|t| !t.is_empty()).or_else(|| res["listing"].as_str()).unwrap_or("") }
/// The board and schematic of a project from the pcb a linked machine's `boards.list` names (`board_default`, from the project's root).
pub fn board_files(root: &str, board_default: &str) -> Value {
    let pcb = if board_default.is_empty() { String::new() } else { format!("{root}/{board_default}") };
    let stem = pcb.strip_suffix(".kicad_pcb").unwrap_or("");
    json!({"root":root,"pro":if stem.is_empty() { String::new() } else { format!("{stem}.kicad_pro") },"pcb":pcb,"sch":if stem.is_empty() { String::new() } else { format!("{stem}.kicad_sch") }})
}

/// Where a request about a thread or project of a linked machine goes: its `<link>~<id>` names it.
pub fn is_far(id: &str) -> bool { crate::net::far::is(id) }

#[cfg(test)]
mod tests {
    use super::*;
    fn st(events: &[Value]) -> State { let mut s = State::new(); s.lean = true; for e in events { s.apply(e); } s }
    fn desk() -> State { st(&[json!({"$":"ProjectCreated","id":"L~p1","title":"Board","root":"/r","at":1}), json!({"$":"ProjectCreated","id":"L~p2","title":"Solo","root":"/s","at":1}),
        json!({"$":"ThreadCreated","id":"L~t1","project":"L~p1","title":"one","at":10}), json!({"$":"ThreadCreated","id":"L~t2","project":"L~p2","title":"two","at":30}),
        json!({"$":"ThreadCreated","id":"L~t3","project":"L~p1","title":"three","at":20}), json!({"$":"ThreadArchived","id":"L~t3","archived":true,"at":21}),
        json!({"$":"TurnChanged","thread":"L~t1","state":"running","at":40})]) }
    fn marks() -> HashMap<String, Value> { let mut m = HashMap::new(); m.insert("desktop".to_string(), crate::mirror::mark_of("desktop", false)); m }
    #[test] fn the_pick_is_every_machine_this_one_or_a_shown_machine() {
        let d = desk(); let far = vec![("desktop".to_string(), &d)];
        assert_eq!(pick_of("", &far), ""); assert_eq!(pick_of("@here", &far), "@here"); assert_eq!(pick_of("desktop", &far), "desktop"); assert_eq!(pick_of("gone", &far), "");
        assert!(here_on("") && here_on("@here") && !here_on("desktop"));
        assert_eq!(shown_far(&far, "@here").len(), 0);
        assert_eq!(shown_far(&far, "desktop").len(), 1); assert_eq!(shown_far(&far, "").len(), 1);
    }
    #[test] fn the_chips_are_empty_with_no_machine_and_all_here_and_each_machine_otherwise() {
        let m = marks(); let un = Unified { far: &HashMap::new(), marks: &m, here: "laptop" };
        assert_eq!(chips(&[], &un, ""), json!([]));
        let d = desk(); let far = vec![("desktop".to_string(), &d)];
        let c = chips(&far, &un, "desktop"); let v: Vec<(&str, bool)> = c.as_array().unwrap().iter().map(|x| (x["value"].as_str().unwrap(), x["on"].as_bool().unwrap())).collect();
        assert_eq!(v, vec![("", false), ("@here", false), ("desktop", true)]); assert_eq!(c[1]["label"], "laptop");
    }
    #[test] fn a_linked_thread_carries_its_mark_state_and_shelf() {
        let d = desk(); let mk = crate::mirror::mark_of("desktop", true);
        let t = thread_json("L~t1", &d, "p9", &mk, "hi").unwrap();
        assert_eq!(t["project"], "p9"); assert_eq!(t["state"], "working"); assert_eq!(t["status"], "active"); assert_eq!(t["mark"]["away"], true); assert_eq!(t["mark"]["short"], "desktop");
        assert_eq!(t["preview"], "hi"); assert_eq!(t["updatedAt"], 40_000); assert_eq!(t["archived"], false);
        assert_eq!(thread_json("L~t3", &d, "x", &mk, "").unwrap()["archived"], true); assert!(thread_json("L~none", &d, "x", &mk, "").is_none());
    }
    #[test] fn linked_threads_list_by_recency_in_their_project_and_join_a_project_here() {
        let d = desk(); let far = vec![("desktop".to_string(), &d)];
        let mut join = HashMap::new(); join.insert("L~p1".to_string(), "p1".to_string());
        let ids = |v: Vec<(u64, String)>| v.into_iter().map(|x| x.1).collect::<Vec<_>>();
        let mut all = thread_keys(&far, "", &join, "", false, None); all.sort(); assert_eq!(ids(all), vec!["L~t2", "L~t1"]);
        assert_eq!(ids(thread_keys(&far, "", &join, "p1", false, None)), vec!["L~t1"], "this hub's project holds its partner's threads");
        assert_eq!(ids(thread_keys(&far, "", &join, "L~p1", false, None)), vec!["L~t1"], "and the linked project by its own id");
        assert_eq!(ids(thread_keys(&far, "", &join, "L~p2", false, None)), vec!["L~t2"]);
        assert_eq!(ids(thread_keys(&far, "", &join, "", true, None)), vec!["L~t3"], "archived");
        assert_eq!(thread_keys(&far, "@here", &join, "", false, None).len(), 0, "this machine only: none of theirs");
        assert_eq!(project_shown("L~p1", &join), "p1"); assert_eq!(project_shown("L~p2", &join), "L~p2");
    }
    #[test] fn a_project_on_both_machines_is_one_row_and_a_linked_only_one_has_its_own() {
        let d = desk(); let far = vec![("desktop".to_string(), &d)]; let m = marks();
        let mut join = HashMap::new(); join.insert("L~p1".to_string(), "p1".to_string());
        let un = Unified { far: &join, marks: &m, here: "laptop" };
        let rows = project_rows(&far, "", &join, &un); assert_eq!(rows.len(), 1); assert_eq!(rows[0].1, "L~p2");
        assert_eq!(rows[0].2["mark"]["name"], "desktop"); assert_eq!(rows[0].2["newOn"][0]["value"], "L~p2"); assert_eq!(rows[0].2["newOn"][0]["label"], "New thread on desktop");
        assert_eq!(rows[0].2["threadCount"], 1);
        assert_eq!(project_rows(&far, "desktop", &HashMap::new(), &un).len(), 2, "a machine picked: its projects keep their own rows");
        let mut local = json!({"id":"p1","title":"Board","threadCount":2,"settledCount":0,"waiting":0,"updatedAt":5});
        let ps = partners(&far, "", &join); join_project(&mut local, &ps["p1"], &un);
        assert_eq!(local["threadCount"], 3); assert_eq!(local["updatedAt"], 40_000); assert_eq!(local["also"], "also on desktop"); assert_eq!(local["marks"][0]["name"], "desktop");
        assert_eq!(local["newOn"][0]["value"], "p1"); assert_eq!(local["newOn"][1]["value"], "L~p1"); assert_eq!(local["newOn"][1]["label"], "New thread on desktop");
        assert!(holder(&far, "L~t1").is_some() && holder(&far, "t1").is_none());
        assert_eq!(on_link(json!({"kind":"board"}), "L~t1")["on"], "L"); assert_eq!(on_link(json!({"kind":"board"}), "t1"), json!({"kind":"board"}));
        assert_eq!(listing_of(&json!({"fmEntries":"d\t0\tsrc"})), "d\t0\tsrc"); assert_eq!(listing_of(&json!({"listing":"f\t3\ta"})), "f\t3\ta"); assert_eq!(listing_of(&json!({})), "");
        assert_eq!(board_files("/r", "b/x.kicad_pcb")["sch"], "/r/b/x.kicad_sch"); assert_eq!(board_files("/r", "")["pcb"], "");
    }
}

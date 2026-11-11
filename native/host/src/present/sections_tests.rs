//! D11 and D12 (docs/ALWAYS-ON.md section 8): every state the hub has a thread in, and where the sidebar puts it.
use super::*;
use crate::present::merged::{merged_projects, RegMachine, RegProject, RegThread};
use serde_json::json;

const NOW: u64 = 10_000_000;
fn st(evs: &[Value]) -> State { let mut s = State::new(); s.lean = true; for e in evs { assert!(s.apply(e), "{e}"); } s }
fn pc(id: &str, title: &str, root: &str, at: u64) -> Value { json!({"$":"ProjectCreated","id":id,"title":title,"root":root,"at":at}) }
fn tc(id: &str, p: &str, at: u64) -> Value { json!({"$":"ThreadCreated","id":id,"project":p,"title":format!("T {id}"),"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at}) }
fn turn(id: &str, state: &str, at: u64) -> Value { json!({"$":"TurnChanged","thread":id,"state":state,"at":at}) }
fn set(k: &str, v: &str, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
fn ask(id: &str, th: &str, kind: &str) -> Value { json!({"$":"AskOpened","id":id,"thread":th,"kind":kind,"tool":"Bash","detail":"rm x","options":""}) }
/// one project with one thread, then `evs`
fn one(evs: &[Value]) -> State { let mut v = vec![pc("p1", "Proj", "/nowhere/proj", NOW - 100), tc("t1", "p1", NOW - 90)]; v.extend(evs.iter().cloned()); st(&v) }

// ---- the status words ------------------------------------------------------------------------------------------------------
#[test] fn every_turn_state_has_its_word() {
    assert_eq!(status(&one(&[]), "t1"), "idle");
    assert_eq!(status(&one(&[turn("t1", "running", NOW - 50)]), "t1"), "running");
    assert_eq!(status(&one(&[turn("t1", "completed", NOW - 50)]), "t1"), "done");
    assert_eq!(status(&one(&[turn("t1", "failed", NOW - 50)]), "t1"), "failed");
    assert_eq!(status(&one(&[turn("t1", "interrupted", NOW - 50)]), "t1"), "interrupted");
    assert_eq!(status(&one(&[]), "nope"), "idle", "an unknown thread");
}
#[test] fn every_ask_is_needs_and_comes_before_the_turn() {
    for k in ["approval", "input", "plan", "browser", "question"] { assert_eq!(status(&one(&[turn("t1", "running", NOW - 50), ask("q1", "t1", k)]), "t1"), "needs", "{k}"); }
    assert_eq!(status(&one(&[json!({"$":"PendingChanged","thread":"t1","count":1,"at":NOW-40})]), "t1"), "needs", "a permission pending");
    assert_eq!(status(&one(&[turn("t1", "failed", NOW - 50), set("auth.wait.t1", "claude", NOW - 40)]), "t1"), "needs", "a sign-in it waits on");
    assert_eq!(status(&one(&[set("secret.ask.boat.api_key", &json!({"thread":"t1","purpose":"x"}).to_string(), NOW - 40)]), "t1"), "needs", "a secret it asked for");
    assert_eq!(status(&one(&[ask("q1", "t1", "approval"), json!({"$":"AskClosed","id":"q1","answer":"yes"})]), "t1"), "idle", "an answered ask is gone");
}
#[test] fn the_phases_after_a_turn_are_monitoring() {
    let bg = "b1\twatching shell\t\t\tshell";
    assert_eq!(status(&one(&[turn("t1", "completed", NOW - 50), set("bg.t1", bg, NOW - 40)]), "t1"), "monitoring", "a background shell");
    assert_eq!(status(&one(&[turn("t1", "completed", NOW - 50), set("bg.t1", "b1\t\u{1}ended:0\t\t\tshell", NOW - 40)]), "t1"), "done", "an ended one is not");
    assert_eq!(status(&one(&[turn("t1", "completed", NOW - 50), set("watch.t1", "w1\twatching pr", NOW - 40)]), "t1"), "monitoring", "a watch");
    assert_eq!(status(&one(&[tc("t2", "p1", NOW - 80), turn("t1", "completed", NOW - 50), json!({"$":"TaskDelegated","task":"k1","parent":"t1","child":"t2","depth":1,"at":NOW-45})]), "t1"), "monitoring", "a delegated task");
    assert_eq!(status(&one(&[turn("t1", "completed", NOW - 50), set("order.wait.t1", "o1", NOW - 40)]), "t1"), "monitoring", "an order it waits on");
}
#[test] fn a_usage_limit_and_a_restart_cut_have_their_own_words() {
    assert_eq!(status(&one(&[turn("t1", "interrupted", NOW - 50), json!({"$":"TurnQueued","thread":"t1","msg":"fallback:held:1","text":"go on"})]), "t1"), "limit", "held in line for the reset");
    assert_eq!(status(&one(&[turn("t1", "failed", NOW - 50), set("provider.resume.t1", &json!({"reset":NOW+60}).to_string(), NOW - 40)]), "t1"), "limit", "a resume record");
    assert_eq!(status(&one(&[turn("t1", "running", NOW - 60), turn("t1", "interrupted", NOW - 50), set("sidebar.cut.t1", &(NOW - 50).to_string(), NOW - 50)]), "t1"), "cut");
    assert_eq!(status(&one(&[turn("t1", "completed", NOW - 60), json!({"$":"TurnQueued","thread":"t1","msg":"m9","text":"next"})]), "t1"), "done", "a queued message nothing took is not running work");
    assert_eq!(status(&one(&[turn("t1", "interrupted", NOW - 60), json!({"$":"TurnQueued","thread":"t1","msg":"m9","text":"next"})]), "t1"), "interrupted", "nor does it hide a stop");
}
#[test] fn a_turn_that_starts_clears_the_cut_mark() {
    let s = one(&[turn("t1", "interrupted", NOW - 50), set("sidebar.cut.t1", "1", NOW - 50)]);
    let ev = meta(&s, &turn("t1", "running", NOW - 10));
    assert_eq!(ev, vec![set("sidebar.cut.t1", "", NOW - 10)]);
    assert!(meta(&s, &turn("t1", "completed", NOW - 10)).is_empty() && meta(&one(&[]), &turn("t1", "running", NOW)).is_empty());
}

// ---- D11: visible or hidden ------------------------------------------------------------------------------------------------
#[test] fn running_needs_and_monitoring_always_show_and_finished_or_idle_never_do() {
    assert!(visible(&one(&[turn("t1", "running", NOW - 50)]), "t1"));
    assert!(visible(&one(&[ask("q1", "t1", "approval")]), "t1"));
    assert!(visible(&one(&[turn("t1", "completed", NOW - 50), set("bg.t1", "b1\twatching\t\t\tshell", NOW - 40)]), "t1"));
    assert!(visible(&one(&[turn("t1", "interrupted", NOW - 50), json!({"$":"TurnQueued","thread":"t1","msg":"fallback:held:1","text":"x"})]), "t1"), "limit-held");
    assert!(!visible(&one(&[turn("t1", "completed", NOW - 50)]), "t1"), "done");
    assert!(!visible(&one(&[]), "t1"), "idle");
}
#[test] fn a_thread_that_needs_the_person_is_never_hidden() {
    // settled, archived, snoozed, a mail run's thread: an open ask still shows
    let base = [ask("q1", "t1", "question")];
    let mut cases = vec![
        one(&[json!({"$":"ThreadSettled","id":"t1","manual":true,"at":NOW-30}), base[0].clone()]),
        one(&[json!({"$":"ThreadArchived","id":"t1","archived":true,"at":NOW-30}), base[0].clone()]),
        one(&[json!({"$":"ThreadSnoozed","id":"t1","until":NOW+9999,"at":NOW-30}), base[0].clone()]),
        one(&[set("sidebar.looked.t1", &NOW.to_string(), NOW), base[0].clone()]),
    ];
    cases.push(one(&[json!({"$":"PendingChanged","thread":"t1","count":2,"at":NOW-40}), json!({"$":"ThreadArchived","id":"t1","archived":true,"at":NOW-30})]));
    for (i, s) in cases.iter().enumerate() { assert!(visible(s, "t1"), "case {i}"); assert_eq!(status(s, "t1"), "needs", "case {i}"); }
    // and it is in Needs you, archived or not
    for s in &cases { let rows = merged_projects(s, &[], &|_| false, &|_| String::new(), "desk", &[]); let v = build(s, &rows, &Value::Null, &ctx(&HashSet::new())); assert_eq!(census(&v).get("needs"), Some(&1), "{v}"); }
}
#[test] fn an_abnormal_stop_shows_until_the_person_looks_at_it_or_settles_it() {
    for (w, evs) in [("failed", vec![turn("t1", "failed", NOW - 50)]), ("interrupted", vec![turn("t1", "interrupted", NOW - 50)]),
                     ("cut", vec![turn("t1", "interrupted", NOW - 50), set("sidebar.cut.t1", "1", NOW - 50)])] {
        let s = one(&evs); assert_eq!(status(&s, "t1"), w); assert!(visible(&s, "t1"), "{w} shows");
        // the person opens it: the look mark (written once), and it is hidden
        let ev = look(&s, "t1", NOW); assert_eq!(ev.len(), 1, "{w}");
        let mut e2 = evs.clone(); e2.extend(ev.clone()); let looked = one(&e2);
        assert!(!visible(&looked, "t1"), "{w}: looked at"); assert!(look(&looked, "t1", NOW + 5).is_empty(), "{w}: marked once");
        // something new after the look (a second failure, a restart's cut) shows it again
        let mut e3 = e2.clone(); e3.push(turn("t1", "failed", NOW + 50)); assert!(visible(&one(&e3), "t1"), "{w}: moved again");
        // settled by hand
        let mut e4 = evs.clone(); e4.push(json!({"$":"ThreadSettled","id":"t1","manual":true,"at":NOW-20})); assert!(!visible(&one(&e4), "t1"), "{w}: settled");
    }
    assert!(look(&one(&[turn("t1", "completed", NOW - 50)]), "t1", NOW).is_empty(), "a finished thread needs no look mark");
    assert!(look(&one(&[turn("t1", "running", NOW - 50)]), "t1", NOW).is_empty());
}
#[test] fn a_pinned_thread_always_shows_and_a_deleted_or_mail_run_never() {
    let s = one(&[turn("t1", "completed", NOW - 50), json!({"$":"ThreadPinned","id":"t1","pinned":true,"at":NOW-40})]);
    assert!(visible(&s, "t1"));
    assert!(!visible(&one(&[json!({"$":"ThreadDeleted","id":"t1","at":NOW-40})]), "t1"));
}
#[test] fn a_registry_row_shows_by_its_word() {
    for w in ["needs", "running", "limit", "monitoring", "failed", "cut", "interrupted"] { assert!(visible_word(w, false), "{w}"); }
    for w in ["done", "idle"] { assert!(!visible_word(w, false), "{w}"); assert!(visible_word(w, true), "{w} pinned"); }
}

// ---- D12: the sections ------------------------------------------------------------------------------------------------------
fn ctx(open: &HashSet<String>) -> Ctx<'_> { Ctx { hub: "h", now: NOW, query: "", open, selected: "" } }
fn ids(v: &Value) -> Vec<String> { v.as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap().to_string()).collect() }
fn titles(s: &Value) -> Vec<String> { s["rows"].as_array().unwrap().iter().map(|r| r["title"].as_str().unwrap_or("").to_string()).collect() }
fn sec<'a>(v: &'a Value, id: &str) -> &'a Value { v.as_array().unwrap().iter().find(|s| s["id"] == id).unwrap_or(&Value::Null) }
const DAY: u64 = 86400;
/// live (a running thread, old project), recent (activity yesterday), grouped (8 days), quiet (30 days), a failure elsewhere
fn world() -> State {
    st(&[pc("pa", "Alive", "/w/acme/alive", NOW - 90 * DAY), tc("ta", "pa", NOW - 90 * DAY), turn("ta", "running", NOW - 60),
         pc("pr", "Recent", "/w/acme/recent", NOW - 2 * DAY), tc("tr", "pr", NOW - DAY), turn("tr", "completed", NOW - DAY),
         pc("pg", "Grouped", "/w/acme/grouped", NOW - 9 * DAY), tc("tg", "pg", NOW - 8 * DAY), turn("tg", "completed", NOW - 8 * DAY),
         pc("pq", "Quiet one", "/w/other/quiet", NOW - 40 * DAY), tc("tq", "pq", NOW - 30 * DAY), turn("tq", "completed", NOW - 30 * DAY),
         pc("pf", "Failing", "/w/other/failing", NOW - 50 * DAY), tc("tf", "pf", NOW - 20 * DAY), turn("tf", "failed", NOW - 20 * DAY),
         ask("q1", "ta", "approval")])
}
#[test] fn the_sections_come_in_their_order_and_each_project_lands_once() {
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let bub = json!({"id":"h|bub","name":"Bub","uber":true});
    let open = HashSet::new(); let v = build(&s, &rows, &bub, &ctx(&open));
    assert_eq!(ids(&v), ["bub", "needs", "active", "group:folder:/w/acme", "quiet"]);
    // Needs you: the approval, then the failure (twenty days old, never looked at)
    assert_eq!(titles(sec(&v, "needs")), ["T ta", "T tf"]);
    assert_eq!(sec(&v, "needs")["rows"][0]["want"], "approval"); assert_eq!(sec(&v, "needs")["rows"][1]["want"], "failed");
    // Active: a project with a visible thread (Alive runs, Failing's failure shows) or activity within three days, newest first
    assert_eq!(titles(sec(&v, "active")), ["Alive", "Recent", "Failing"], "{v}");
    let alive = &sec(&v, "active")["rows"][0]; assert_eq!(alive["running"], 1); assert_eq!(alive["needsCount"], 1);
    assert!(alive["threads"].as_array().unwrap().is_empty(), "its thread is in Needs you, not repeated under it");
    // a group folds by default with its count, and Quiet too
    let g = sec(&v, "group:folder:/w/acme"); assert_eq!((g["open"].as_bool(), g["count"].as_u64(), g["rows"].as_array().unwrap().len()), (Some(false), Some(1), 0));
    assert_eq!(g["title"], "acme"); assert_eq!(sec(&v, "quiet")["count"], 1);
    // opened, they list their projects
    let open: HashSet<String> = ["sec:group:folder:/w/acme".to_string(), "sec:quiet".to_string()].into(); let v = build(&s, &rows, &bub, &ctx(&open));
    assert_eq!(titles(sec(&v, "group:folder:/w/acme")), ["Grouped"]); assert_eq!(titles(sec(&v, "quiet")), ["Quiet one"]);
}
#[test] fn needs_you_is_gone_when_empty_and_bub_only_when_there_is_one() {
    let s = one(&[turn("t1", "completed", NOW - 50)]); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert_eq!(ids(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new()))), ["active"]);
}
#[test] fn a_pinned_project_leads_and_the_pin_toggles() {
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let key = rows.iter().find(|p| p.name == "Quiet one").unwrap().key.clone();
    let a = act(&s, &rows, "project-pin", &key, &json!({}), NOW).unwrap(); assert_eq!(a.sets, vec![(format!("sidebar.pin.{key}"), NOW.to_string())]);
    let mut s2 = world(); s2.apply(&set(&format!("sidebar.pin.{key}"), &NOW.to_string(), NOW)); let rows2 = merged_projects(&s2, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v = build(&s2, &rows2, &Value::Null, &ctx(&HashSet::new()));
    assert_eq!(ids(&v)[1], "pinned"); assert_eq!(titles(sec(&v, "pinned")), ["Quiet one"]); assert!(sec(&v, "quiet").is_null(), "it left Quiet");
    assert_eq!(act(&s2, &rows2, "project-pin", &key, &json!({}), NOW).unwrap().sets[0].1, "", "pressed again: unpinned");
    assert!(act(&s, &rows, "project-pin", "no-such-key", &json!({}), NOW).is_none());
}
#[test] fn groups_follow_the_git_owner_then_the_folder_and_take_the_persons_name_and_moves() {
    assert_eq!(auto_group("github.com/AmericanEmbedded/fazepro", "/x/y"), ("owner:github.com/americanembedded".into(), "AmericanEmbedded".into()));
    assert_eq!(auto_group("", "/home/h/Documents/Nordic/thingy"), ("folder:/home/h/Documents/Nordic".into(), "Nordic".into()));
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let gid = "folder:/w/acme";
    assert_eq!(group_name(&s, gid, "acme"), "acme");
    let mut s2 = world(); s2.apply(&set(&format!("sidebar.group.pick.{gid}"), "Acme Corp", 1)); assert_eq!(group_name(&s2, gid, "acme"), "Acme Corp", "Haiku's name");
    s2.apply(&set(&format!("sidebar.group.name.{gid}"), "ACME", 2)); assert_eq!(group_name(&s2, gid, "acme"), "ACME", "the person's wins");
    assert!(groups_due(&s2, &rows).iter().all(|g| g.0 != gid), "a named group is not asked again");
    assert!(groups_due(&s, &rows).iter().any(|g| g.0 == gid && g.2.iter().any(|m| m.starts_with("Grouped"))));
    // rename from its field, a move to another group and to a new one
    let a = act(&s, &rows, "group-rename", gid, &json!({format!("@f.group.{gid}"): "  Acme  "}), NOW).unwrap(); assert_eq!(a.sets, vec![(format!("sidebar.group.name.{gid}"), "Acme".into())]);
    let key = rows.iter().find(|p| p.name == "Grouped").unwrap().key.clone();
    let a = act(&s, &rows, "group-move", &format!("{key}\u{1f}+Client work"), &json!({}), NOW).unwrap();
    assert_eq!(a.sets, vec![(format!("sidebar.group.of.{key}"), "custom:client-work".into()), ("sidebar.group.name.custom:client-work".into(), "Client work".into())]);
    let mut s3 = world(); for (k, v) in &a.sets { s3.apply(&set(k, v, 3)); } let rows3 = merged_projects(&s3, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v = build(&s3, &rows3, &Value::Null, &ctx(&HashSet::new())); assert_eq!(sec(&v, "group:custom:client-work")["title"], "Client work");
}
#[test] fn the_sidebar_search_opens_folds_and_finds_hidden_threads() {
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let open = HashSet::new(); let c = Ctx { query: "tq", ..ctx(&open) }; let v = build(&s, &rows, &Value::Null, &c);
    let q = sec(&v, "quiet"); assert_eq!(q["open"], true); assert_eq!(titles(q), ["Quiet one"]); assert_eq!(q["rows"][0]["threads"][0]["title"], "T tq", "a finished thread found by its title");
}
#[test] fn a_machine_that_is_off_keeps_its_needs_and_failures_visible() {
    let s = st(&[]);
    let reg = vec![RegProject { key: "github.com/o/r".into(), name: "Far".into(), origin: "github.com/o/r".into(), machines: vec![RegMachine { machine: "desktop".into(), id: "p9".into(), folder: "/d/r".into(), seen: NOW - 60 }],
        threads: vec![RegThread { machine: "desktop".into(), id: "t9".into(), title: "asks".into(), status: "needs".into(), activity: NOW - 70, pinned: false },
                      RegThread { machine: "desktop".into(), id: "t8".into(), title: "broke".into(), status: "failed".into(), activity: NOW - 80, pinned: false },
                      RegThread { machine: "desktop".into(), id: "t7".into(), title: "fine".into(), status: "done".into(), activity: NOW - 90, pinned: false }] }];
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "laptop", &reg);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    assert_eq!(titles(sec(&v, "needs")), ["asks"], "a stop over there is not in Needs you: its look mark and launcher are on that machine");
    assert_eq!(sec(&v, "needs")["rows"][0]["off"], true); assert_eq!(sec(&v, "needs")["rows"][0]["nav"], "", "nothing to open while it is off");
}
#[test] fn a_linked_machines_threads_show_in_the_same_sections_with_their_marker() {
    let s = st(&[pc("p1", "Here", "/h/r", NOW - 100)]);
    let f = st(&[pc("L~p1", "There", "/d/r", NOW - 100), tc("L~t1", "L~p1", NOW - 50), turn("L~t1", "failed", NOW - 40)]);
    let far = vec![("desktop".to_string(), &f)];
    let rows = merged_projects(&s, &far, &|_| false, &|_| String::new(), "laptop", &[]);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    let r = &sec(&v, "needs")["rows"][0]; assert_eq!(r["id"], "h|L~t1"); assert_eq!(r["machine"], "desktop"); assert_eq!(r["mark"]["short"], "desktop");
}
#[test] fn a_far_thread_quiet_past_active_or_running_on_a_machine_away_leaves_the_sidebar_and_needs_you() {
    assert_eq!(far_word("needs", false, ACTIVE + 1), "idle", "an ask over there, quiet for longer than ACTIVE, drops out like a settled one");
    assert_eq!(far_word("running", false, ACTIVE + 1), "idle"); assert_eq!(far_word("failed", true, ACTIVE + 1), "idle");
    assert_eq!(far_word("running", true, 60), "idle", "a machine that is away runs nothing the person can see");
    assert_eq!(far_word("needs", true, 60), "needs", "its recent ask is still the person's"); assert_eq!(far_word("running", false, 60), "running");
    let s = st(&[pc("p1", "Here", "/h/r", NOW - 100)]);
    let f = st(&[pc("L~p1", "There", "/d/r", NOW - 4 * 86400), tc("L~old", "L~p1", NOW - 4 * 86400), ask("q", "L~old", "question"),
        tc("L~run", "L~p1", NOW - 2 * 86400), turn("L~run", "running", NOW - 2 * 86400), tc("L~chat", "", NOW - 4 * 86400), ask("q2", "L~chat", "question")]);
    let far = vec![("framework".to_string(), &f)];
    let rows = merged_projects(&s, &far, &|n| n == "framework", &|_| String::new(), "desk", &[]);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    assert!(sec(&v, "needs").is_null(), "no stale ask from the far machine, in a project or outside one: {v}");
    let there = sec(&v, "active")["rows"].as_array().unwrap().iter().find(|r| r["name"] == "There").cloned().unwrap_or(Value::Null);
    assert_eq!(there["running"], 0, "its old running turn is not counted while the machine is away: {there}");
    // the same machine answering, and the threads recent: as they were
    let f2 = st(&[pc("L~p1", "There", "/d/r", NOW - 100), tc("L~run", "L~p1", NOW - 60), turn("L~run", "running", NOW - 60)]);
    let far2 = vec![("framework".to_string(), &f2)];
    let rows2 = merged_projects(&s, &far2, &|_| false, &|_| String::new(), "desk", &[]);
    let v2 = build(&s, &rows2, &Value::Null, &ctx(&HashSet::new()));
    assert_eq!(sec(&v2, "active")["rows"].as_array().unwrap().iter().find(|r| r["name"] == "There").unwrap()["running"], 1);
}
#[test] fn another_machines_thread_can_be_hidden_here_until_something_new_happens_there() {
    let f = st(&[pc("L~p1", "There", "/d/r", NOW - 100), tc("L~t1", "L~p1", NOW - 50), ask("q", "L~t1", "question")]);
    let far = vec![("framework".to_string(), &f)];
    let mine = st(&[pc("p1", "Here", "/h/r", NOW - 100), tc("t1", "p1", NOW - 50), ask("q1", "t1", "question")]);
    let rows = merged_projects(&mine, &far, &|_| false, &|_| String::new(), "desk", &[]);
    let v = build(&mine, &rows, &Value::Null, &ctx(&HashSet::new()));
    let far_row = sec(&v, "needs")["rows"].as_array().unwrap().iter().find(|r| r["id"] == "h|L~t1").cloned().unwrap();
    assert_eq!((far_row["hide"].as_str(), far_row["lead"][0]["action"].as_str()), (Some("h|L~t1"), Some("thread-hide")), "{far_row}");
    let local_row = sec(&v, "needs")["rows"].as_array().unwrap().iter().find(|r| r["id"] == "h|t1").cloned().unwrap();
    assert_eq!(local_row["hide"], "", "a local thread has Settle and Archive");
    assert!(act(&mine, &rows, "thread-hide", "h|t1", &json!({}), NOW).is_none(), "not a local one");
    assert!(act(&mine, &rows, "thread-hide", "h|L~nope", &json!({}), NOW).is_none(), "not one it does not list");
    let a = act(&mine, &rows, "thread-hide", "h|L~t1", &json!({}), NOW).unwrap();
    assert_eq!(a.sets, vec![("sidebar.hide.L~t1".to_string(), NOW.to_string())]);
    let mine2 = st(&[pc("p1", "Here", "/h/r", NOW - 100), tc("t1", "p1", NOW - 50), ask("q1", "t1", "question"), set("sidebar.hide.L~t1", &NOW.to_string(), NOW)]);
    let rows2 = merged_projects(&mine2, &far, &|_| false, &|_| String::new(), "desk", &[]);
    assert_eq!(titles(sec(&build(&mine2, &rows2, &Value::Null, &ctx(&HashSet::new())), "needs")), ["T t1"], "hidden here");
    let f2 = st(&[pc("L~p1", "There", "/d/r", NOW - 100), tc("L~t1", "L~p1", NOW - 50), ask("q", "L~t1", "question"), turn("L~t1", "running", NOW + 5)]);
    let far2 = vec![("framework".to_string(), &f2)];
    let rows3 = merged_projects(&mine2, &far2, &|_| false, &|_| String::new(), "desk", &[]);
    let c = Ctx { hub: "h", now: NOW + 10, query: "", open: &HashSet::new(), selected: "" };
    assert_eq!(titles(sec(&build(&mine2, &rows3, &Value::Null, &c), "needs")).len(), 2, "back once it moves there");
}

// ---- the project page and Ctrl+K --------------------------------------------------------------------------------------------
#[test] fn the_project_page_lists_running_needs_recent_and_done_behind_its_count() {
    let s = st(&[pc("p1", "Proj", "/n/p", NOW - 100), tc("a", "p1", NOW - 90), turn("a", "running", NOW - 10), tc("b", "p1", NOW - 80), ask("q", "b", "input"),
                 tc("c", "p1", NOW - 70), tc("d", "p1", NOW - 60), turn("d", "completed", NOW - 50), json!({"$":"ThreadSettled","id":"d","manual":false,"at":NOW-40}),
                 tc("e", "p1", NOW - 30), json!({"$":"ThreadArchived","id":"e","archived":true,"at":NOW-20})]);
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let open = HashSet::new(); let pg = page(&s, &rows, &rows[0].key, &ctx(&open), false);
    let secs = pg["sections"].as_array().unwrap();
    assert_eq!(secs.iter().map(|x| x["id"].as_str().unwrap()).collect::<Vec<_>>(), ["running", "needs", "recent", "done"]);
    assert_eq!(titles(&secs[0]), ["T a"]); assert_eq!(titles(&secs[1]), ["T b"]); assert_eq!(titles(&secs[2]), ["T c"]);
    assert_eq!(secs[3]["title"], "Done (1)"); assert!(secs[3]["rows"].as_array().unwrap().is_empty(), "folded");
    assert_eq!(titles(&page(&s, &rows, &rows[0].key, &ctx(&open), true)["sections"][3]), ["T d"]);
    assert!(page(&s, &rows, "nope", &ctx(&open), false).is_null());
}
#[test] fn ctrl_k_lists_every_project_and_thread_waiting_ones_first() {
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let open = HashSet::new(); let j = jump(&rows, &ctx(&open)); let a = j.as_array().unwrap();
    assert_eq!(a.iter().filter(|x| x["kind"] == "project").count(), 5); assert_eq!(a.iter().filter(|x| x["kind"] == "thread").count(), 5, "the hidden threads too");
    assert_eq!(a[0]["label"], "T ta"); assert_eq!(a[0]["action"], "select"); assert_eq!(a[0]["value"], "h|ta");
    let p = a.iter().find(|x| x["kind"] == "project").unwrap(); assert_eq!(p["action"], "project-page");
}
#[test] fn the_section_actions_refuse_what_is_not_theirs() {
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert!(act(&s, &rows, "side-section", "sec:quiet", &json!({}), NOW).unwrap().open_toggle.is_some());
    assert!(act(&s, &rows, "side-section", "settings", &json!({}), NOW).is_none());
    assert!(act(&s, &rows, "project-page", "nope", &json!({}), NOW).is_none());
    assert_eq!(act(&s, &rows, "project-page", "", &json!({}), NOW).unwrap().page.as_deref(), Some(""), "the page shuts");
    assert!(act(&s, &rows, "group-move", "no-separator", &json!({}), NOW).is_none());
    assert!(act(&s, &rows, "other", "", &json!({}), NOW).is_none());
}

#[test] fn every_stop_from_before_this_build_counts_as_seen_and_one_after_it_shows() {
    let since = NOW - 100;
    let s = st(&[pc("p1", "P", "/n/p", 1), tc("a", "p1", 2), turn("a", "failed", NOW - 10 * DAY), tc("b", "p1", 3), turn("b", "interrupted", NOW - 200),
                 tc("c", "p1", 4), turn("c", "failed", NOW - 10 * DAY), ask("q", "c", "approval"), tc("d", "p1", 5), turn("d", "failed", NOW - 50),
                 set("sidebar.looked.since", &since.to_string(), since)]);
    assert!(!visible(&s, "a") && !visible(&s, "b"), "stops before the baseline: seen, however recent"); assert!(visible(&s, "c"), "an ask always shows"); assert!(visible(&s, "d"), "a stop after it shows");
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert_eq!(titles(sec(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new())), "needs")), ["T c", "T d"]);
    let pg = page(&s, &rows, &rows[0].key, &ctx(&HashSet::new()), true); let all: Vec<String> = pg["sections"].as_array().unwrap().iter().flat_map(titles).collect();
    assert!(all.contains(&"T a".to_string()) && all.contains(&"T b".to_string()), "still on the project page: {all:?}");
}
#[test] fn only_the_persons_own_stops_need_them_and_backend_chatter_never_shows() {
    let bot = json!({"$":"BotSet","id":"b1","name":"Mail","thread":"tb","look":1,"at":5});
    let evs = vec![pc("p1", "P", "/n/p", 1),
        tc("tb", "p1", 2), turn("tb", "failed", NOW - 50),                                                       // a bot's chat
        tc("tk", "p1", 3), json!({"$":"TaskDelegated","task":"k","parent":"tl","child":"tk","depth":1,"at":NOW-60}), turn("tk", "interrupted", NOW - 40), // a delegated child
        tc("tl", "p1", 4), turn("tl", "failed", NOW - 30),                                                      // the person's own (the child's parent)
        tc("tn", "p1", 5), set("launch.tn", "Ltl", NOW - 70), turn("tn", "failed", NOW - 20),                 // a lead launched it for the person
        tc("tm", "p1", 6), set("launch.tm", "Ltb", NOW - 70), turn("tm", "failed", NOW - 20),                 // a bot launched it
        tc("tr", "p1", 7), json!({"$":"RunSet","thread":"tr","bot":"b1","conv":"","mail":"","at":NOW-70}), turn("tr", "failed", NOW - 20)]; // a routine's or mail run
    let mut s = State::new(); s.lean = true; for e in &evs { s.apply(e); } s.apply(&bot);
    for id in ["tb", "tk", "tm", "tr"] { assert!(!own(&s, id), "{id}"); assert!(!visible(&s, id), "{id} is backend chatter"); }
    // the uberbot is a bot, but what it launches is the person's (ThreadLaunched sets launch.<t>)
    let mut u = State::new(); u.lean = true; for e in &evs { u.apply(e); } u.apply(&bot);
    u.apply(&json!({"$":"UberSet","bot":"b1","uid":"u","at":6})); u.apply(&json!({"$":"ThreadLaunched","id":"tm","by":"tb","report":true,"at":7}));
    assert!(own(&u, "tm") && visible(&u, "tm"), "launched by the uberbot: the person's");
    for id in ["tl", "tn"] { assert!(own(&s, id), "{id}"); assert!(visible(&s, id), "{id}"); }
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert_eq!(titles(sec(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new())), "needs")), ["T tn", "T tl"]);
    assert!(look(&s, "tb", NOW).is_empty(), "no look mark for a thread that never shows");
    // the bot's chat asking the person is in Needs you all the same
    s.apply(&ask("q", "tb", "question")); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert!(titles(sec(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new())), "needs")).contains(&"T tb".to_string()));
}
#[test] fn a_stop_that_carries_on_by_itself_is_not_abnormal_and_one_written_to_leaves() {
    let s = one(&[turn("t1", "running", NOW - 60), turn("t1", "interrupted", NOW - 50), set("carry.wait.t1", "1", NOW - 50)]);
    assert_eq!(status(&s, "t1"), "running", "the restart's line will carry it on");
    // the person writes to a failed thread: it runs, then is done; neither is a stop
    let s = one(&[turn("t1", "failed", NOW - 50), json!({"$":"MessagePosted","thread":"t1","msg":"m","role":"user","text":"again","at":NOW-40}), turn("t1", "running", NOW - 40)]);
    assert_eq!(status(&s, "t1"), "running");
    let s = one(&[turn("t1", "failed", NOW - 50), turn("t1", "running", NOW - 40), turn("t1", "completed", NOW - 30)]);
    assert!(!visible(&s, "t1"), "resumed (failover, a retry) and done: not abnormal");
}
#[test] fn an_off_machines_stop_is_listed_dimmed_but_only_its_ask_is_in_needs_you() {
    let reg = vec![RegProject { key: "k".into(), name: "Far".into(), origin: "".into(), machines: vec![RegMachine { machine: "desktop".into(), id: "p9".into(), folder: "/d".into(), seen: NOW }],
        threads: vec![RegThread { machine: "desktop".into(), id: "x".into(), title: "broke".into(), status: "failed".into(), activity: NOW - 9, pinned: false }] }];
    let s = st(&[]); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "laptop", &reg);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    assert!(sec(&v, "needs").is_null()); assert_eq!(sec(&v, "active")["rows"][0]["threads"][0]["off"], true);
}
#[test] fn an_ask_outside_every_project_is_in_needs_you() {
    let s = st(&[pc("p1", "P", "/n/p", 1), tc("x", "", NOW - 50), ask("q", "x", "question")]);
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    assert_eq!(titles(sec(&v, "needs")), ["T x"]); assert_eq!(sec(&v, "needs")["rows"][0]["project"], "Conversation history");
}

/// The sections a real log gives, from a copy (`BP_SIDEBAR_LOG=<copy of events.jsonl> cargo test -- --ignored census_of_a_log --nocapture`): the
/// counts the person would see after upgrading (the baseline written at the log's end, as a first start does). Never run on the live file.
#[test] #[ignore] fn census_of_a_log() {
    use std::io::BufRead;
    let Ok(path) = std::env::var("BP_SIDEBAR_LOG") else { return };
    let mut s = State::new(); s.lean = true; let mut last = 0u64;
    for l in std::io::BufReader::new(std::fs::File::open(&path).unwrap()).lines().map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<Value>(&l) { last = last.max(v["at"].as_u64().unwrap_or(0)); } s.apply_line(&l);
    }
    let now = std::env::var("BP_SIDEBAR_NOW").ok().and_then(|v| v.parse().ok()).unwrap_or(last);
    if std::env::var("BP_SIDEBAR_NO_BASELINE").is_err() { s.apply(&set("sidebar.looked.since", &now.to_string(), now)); }
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let open = HashSet::new(); let c = Ctx { hub: "", now, query: "", open: &open, selected: "" };
    let v = build(&s, &rows, &Value::Null, &c);
    let top: usize = v.as_array().unwrap().iter().filter(|x| x["kind"] == "projects").flat_map(|x| x["rows"].as_array().cloned().unwrap_or_default()).map(|p| p["threads"].as_array().map_or(0, |t| t.len())).sum();
    println!("census: sections {:?}", ids(&v));
    for x in v.as_array().unwrap() { println!("census: {} count={} rows={}", x["id"], x["count"], x["rows"].as_array().map_or(0, |r| r.len())); }
    println!("census: thread rows under shown projects {top}");
    for r in sec(&v, "needs")["rows"].as_array().into_iter().flatten() { println!("census: needs  {} | {} | {} | {}", r["want"], r["word"], r["project"], r["title"]); }
    for r in sec(&v, "active")["rows"].as_array().into_iter().flatten() { println!("census: active {} running={} threads={}", r["title"], r["running"], r["threads"].as_array().map_or(0, |t| t.len())); }
}

#[test] fn a_live_swarm_is_one_row_under_its_project_and_in_needs_you_when_it_waits_on_the_person() {
    let room = |id: &str, state: &str, reason: &str| json!({"$":"RoomSet","id":id,"name":format!("S {id}"),"members":"thread:a\nthread:b","kind":"swarm","project":"p1","state":state,"reason":reason,"at":NOW-30});
    // an old project whose only life is a running swarm: the swarm's row, the project Active, the members never rows of their own
    let s = st(&[pc("p1", "P", "/n/p", NOW - 90 * DAY), tc("a", "p1", NOW - 90 * DAY), tc("b", "p1", NOW - 90 * DAY), turn("a", "running", NOW - 40), turn("b", "failed", NOW - 90 * DAY), room("r1", "", "")]);
    assert!(!visible(&s, "a") && !visible(&s, "b") && !own(&s, "b"), "members: the swarm's row stands for them");
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    // b's failed turn makes the swarm "failed" (sidebar::live_swarms): Needs you, opening the swarm's page
    let n = sec(&v, "needs"); assert_eq!(n["rows"][0]["kind"], "swarm"); assert_eq!(n["rows"][0]["nav"], "@room:r1"); assert_eq!(n["rows"][0]["title"], "S r1");
    let p = &sec(&v, "active")["rows"][0]; assert_eq!(p["title"], "P"); assert!(p["threads"].as_array().unwrap().is_empty(), "no member row: {p}");
    // a running swarm: a row under its project, counted running, not in Needs you
    let s2 = st(&[pc("p1", "P", "/n/p", NOW - 90 * DAY), tc("a", "p1", NOW - 90 * DAY), tc("b", "p1", NOW - 90 * DAY), turn("a", "running", NOW - 40), room("r1", "", "")]);
    let rows2 = merged_projects(&s2, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v2 = build(&s2, &rows2, &Value::Null, &ctx(&HashSet::new()));
    assert!(sec(&v2, "needs").is_null());
    let p2 = &sec(&v2, "active")["rows"][0]; assert_eq!(p2["swarms"][0]["nav"], "@room:r1"); assert_eq!(p2["swarms"][0]["members"], 2); assert!(p2["running"].as_u64().unwrap() >= 1);
    // a guard's pause asks the person; a member's ask does too
    for evs in [vec![room("r1", "paused", "circular")], vec![room("r1", "", ""), ask("q", "a", "question")]] {
        let mut e = vec![pc("p1", "P", "/n/p", 1), tc("a", "p1", 2), tc("b", "p1", 3)]; e.extend(evs);
        let s3 = st(&e); let rows3 = merged_projects(&s3, &[], &|_| false, &|_| String::new(), "desk", &[]);
        let n3 = sec(&build(&s3, &rows3, &Value::Null, &ctx(&HashSet::new())), "needs").clone();
        assert_eq!(n3["count"], 1, "{n3}"); assert_eq!(n3["rows"][0]["kind"], "swarm"); assert_eq!(n3["rows"][0]["word"], "needs");
    }
    // one that is over is no row; Ctrl+K reaches a live one and its project's page lists it
    let s4 = st(&[pc("p1", "P", "/n/p", 1), tc("a", "p1", 2), tc("b", "p1", 3), room("r1", "done", "")]);
    let rows4 = merged_projects(&s4, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert!(sec(&build(&s4, &rows4, &Value::Null, &ctx(&HashSet::new())), "needs").is_null());
    let j = jump(&rows2, &ctx(&HashSet::new())); assert!(j.as_array().unwrap().iter().any(|x| x["kind"] == "swarm" && x["action"] == "room" && x["value"] == "h|r1"));
    let pg = page(&s2, &rows2, &rows2[0].key, &ctx(&HashSet::new()), false); assert_eq!(pg["sections"][0]["rows"][0]["kind"], "swarm");
}

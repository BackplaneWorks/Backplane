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
/// A pinned thread is a fixed row of its project: first under it, in the order pinned, whatever runs below, and never cut by the row cap.
#[test] fn a_pinned_thread_is_first_under_its_project_and_every_row_of_this_machine_has_its_menu() {
    let pin = |id: &str, on: bool, at: u64| json!({"$":"ThreadPinned","id":id,"pinned":on,"at":at});
    let mut evs = vec![tc("t2", "p1", NOW - 80), tc("t3", "p1", NOW - 70), turn("t1", "completed", NOW - 60), turn("t3", "completed", NOW - 55), turn("t2", "running", NOW - 10), pin("t3", true, NOW - 40), pin("t1", true, NOW - 30)];
    let under = |s: &State| { let rows = merged_projects(s, &[], &|_| false, &|_| String::new(), "desk", &[]); let v = build(s, &rows, &Value::Null, &ctx(&HashSet::new())); sec(&v, "active")["rows"][0]["threads"].clone() };
    let names = |u: &Value| u.as_array().unwrap().iter().map(|r| r["title"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let u = under(&one(&evs));
    assert_eq!(names(&u), ["T t3", "T t1"], "the pins first, as pinned; the running one is a row of Running");
    let running = { let s = one(&evs); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]); sec(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new())), "running")["rows"].clone() };
    assert_eq!(names(&running), ["T t2"]); assert_eq!(running[0]["project"], "Proj"); assert_eq!(running[0]["settle"], "", "a thread that runs is not settled from under its turn");
    assert_eq!(u[0]["settle"], "h|t3", "a finished one settles in one press");
    let menu = |r: &Value| r["menu"].as_array().unwrap().iter().map(|m| (m["id"].as_str().unwrap().to_string(), m["action"].as_str().unwrap().to_string(), m["value"].as_str().unwrap().to_string())).collect::<Vec<_>>();
    assert_eq!(menu(&u[0]), [("unpin".into(), "row-unpin".into(), "h|t3".into()), ("settle".into(), "row-settle".into(), "h|t3".into())], "no Archive: it is Settle");
    assert_eq!(menu(&running[0])[0].0, "pin"); assert_eq!(u[0]["menu"][0]["label"], "Unpin"); assert_eq!(running[0]["menu"][0]["label"], "Pin");
    // no fold: a project's rows are few and fixed (two pins, one other), so no chevron; an old fold setting changes nothing
    { let s = one(&evs); let key = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[])[0].key.clone();
      let mut f = evs.clone(); f.push(json!({"$":"SettingSet","key":format!("sidebar.fold.{key}"),"value":"1","at":NOW-1}));
      let s = one(&f); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]); let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
      let p = &sec(&v, "active")["rows"][0]; assert_eq!((p["fold"].as_str(), p["folded"].as_bool(), p["under"].as_u64()), (Some(""), Some(false), Some(0))); assert_eq!(names(&p["threads"]), ["T t3", "T t1"]); }
    // ten more running threads, all newer: still the two pins alone (what runs is in Running)
    for i in 0..10 { let id = format!("r{i}"); evs.push(tc(&id, "p1", NOW - 9)); evs.push(turn(&id, "running", NOW - 5)); }
    let u = under(&one(&evs)); assert_eq!(names(&u), ["T t3", "T t1"]);
    // settled by hand, a thread gives up its pin (the state's law) and leaves; unpinned and done, the other leaves too. The page's Done row offers Pin and Unsettle
    evs.truncate(7); evs.push(json!({"$":"ThreadSettled","id":"t3","manual":true,"at":NOW-20})); evs.push(pin("t1", false, NOW - 20));
    let s = one(&evs); assert!(names(&under(&s)).is_empty());
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]); let open = HashSet::new();
    let pg = page(&s, &rows, &rows[0].key, &ctx(&open), true); let done = pg["sections"].as_array().unwrap().iter().find(|x| x["id"] == "done").unwrap();
    let t3 = done["rows"].as_array().unwrap().iter().find(|r| r["title"] == "T t3").unwrap(); assert_eq!((t3["menu"][0]["id"].as_str(), t3["menu"][1]["id"].as_str()), (Some("pin"), Some("unsettle")));
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
         ask("q1", "ta", "approval"), set(GROUPS, "on", NOW - 90 * DAY)])
}
/// Groups are a setting, off by default: the projects that are neither active nor quiet stay in the one list, newest first, no group is named,
/// and a group the person made is still a section (with a way back out of it).
#[test] fn automatic_groups_are_off_unless_the_setting_is_on() {
    let mut s = world(); s.apply(&set(GROUPS, "", NOW)); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert!(!groups_on(&s) && groups_on(&world()));
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    assert_eq!(ids(&v), ["needs", "active", "quiet"], "no group section");
    assert_eq!(titles(sec(&v, "active")), ["Alive", "Failing", "Grouped", "Recent"], "the grouped one is in the list, by name like the rest");
    assert!(groups_due(&s, &rows).is_empty(), "nothing asks Haiku for a name");
    let g = sec(&v, "active")["rows"].as_array().unwrap().iter().find(|r| r["title"] == "Grouped").unwrap().clone(); assert_eq!(g["group"], "");
    let sub = |r: &Value| r["menu"].as_array().unwrap().iter().find(|i| i["id"] == "move").unwrap()["sub"].as_array().unwrap().iter().map(|i| i["label"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(sub(&g), ["New group\u{2026}"]);
    // the person makes a group: it is a section, and the project's menu has the way back out
    let key = rows.iter().find(|p| p.name == "Grouped").unwrap().key.clone();
    for (k, val) in new_group_sets(&key, "Clients").unwrap() { s.apply(&set(&k, &val, NOW)); }
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let open: HashSet<String> = ["sec:group:custom:clients".to_string()].into(); let v = build(&s, &rows, &Value::Null, &ctx(&open));
    assert_eq!(ids(&v), ["needs", "active", "group:custom:clients", "quiet"]); assert_eq!(titles(sec(&v, "group:custom:clients")), ["Grouped"]);
    let g = sec(&v, "group:custom:clients")["rows"][0].clone(); assert_eq!(sub(&g), ["No group", "New group\u{2026}"]);
    let back = g["menu"].as_array().unwrap().iter().find(|i| i["id"] == "move").unwrap()["sub"][0]["value"].as_str().unwrap().to_string();
    assert_eq!(act(&s, &rows, "group-move", &back, &json!({}), NOW).unwrap().sets, vec![(format!("sidebar.group.of.{key}"), String::new())]);
}
#[test] fn the_sections_come_in_their_order_and_each_project_lands_once() {
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let bub = json!({"id":"h|bub","name":"Bub","uber":true});
    let open = HashSet::new(); let v = build(&s, &rows, &bub, &ctx(&open));
    assert_eq!(ids(&v), ["bub", "needs", "active", "group:folder:/w/acme", "quiet"]);
    // Needs you: the approval, then the failure (twenty days old, never looked at)
    assert_eq!(titles(sec(&v, "needs")), ["T ta", "T tf"]);
    assert_eq!(sec(&v, "needs")["rows"][0]["want"], "approval"); assert_eq!(sec(&v, "needs")["rows"][1]["want"], "failed");
    // Recent: a project with a visible thread (Alive runs, Failing's failure shows) or activity within three days; on the bench by name
    assert_eq!(titles(sec(&v, "active")), ["Alive", "Failing", "Recent"], "{v}");
    let alive = &sec(&v, "active")["rows"][0]; assert_eq!(alive["running"], 1); assert_eq!(alive["needsCount"], 1);
    assert!(alive["threads"].as_array().unwrap().is_empty(), "its thread is in Needs you, not repeated under it");
    // a group folds by default with its count, and Quiet too
    let g = sec(&v, "group:folder:/w/acme"); assert_eq!((g["open"].as_bool(), g["count"].as_u64(), g["rows"].as_array().unwrap().len()), (Some(false), Some(1), 0));
    assert_eq!(g["title"], "acme"); assert_eq!(sec(&v, "quiet")["count"], 1);
    // opened, they list their projects
    let open: HashSet<String> = ["sec:group:folder:/w/acme".to_string(), "sec:quiet".to_string()].into(); let v = build(&s, &rows, &bub, &ctx(&open));
    assert_eq!(titles(sec(&v, "group:folder:/w/acme")), ["Grouped"]); assert_eq!(titles(sec(&v, "quiet")), ["Quiet one"]);
}
#[test] fn a_long_done_list_keeps_the_rest_behind_older() {
    let mut evs = vec![]; for i in 2..=70 { let id = format!("t{i}"); evs.push(tc(&id, "p1", NOW - 5000 + i)); evs.push(turn(&id, "completed", NOW - 4000 + i)); }
    let s = one(&evs); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]); let key = rows[0].key.clone();
    let lens = |v: &Value| -> Vec<(String, u64, usize)> { v["sections"].as_array().unwrap().iter().filter(|x| x["fold"] == true).map(|x| (x["title"].as_str().unwrap().to_string(), x["count"].as_u64().unwrap(), x["rows"].as_array().unwrap().len())).collect() };
    let none = HashSet::new(); let older: HashSet<String> = ["page-older".to_string()].into();
    assert_eq!(lens(&page(&s, &rows, &key, &ctx(&none), false)), [("Done (69)".to_string(), 69, 0), ("Older (9)".to_string(), 0, 0)], "Done folded: no Older head");
    assert_eq!(lens(&page(&s, &rows, &key, &ctx(&none), true)), [("Done (69)".to_string(), 69, 60), ("Older (9)".to_string(), 9, 0)], "the newest 60, the rest counted");
    assert_eq!(lens(&page(&s, &rows, &key, &ctx(&older), true)), [("Done (69)".to_string(), 69, 60), ("Older (9)".to_string(), 9, 9)], "Older opened: every thread is on the page");
    assert!(act(&s, &rows, "side-section", "page-older", &json!({}), NOW).unwrap().open_toggle.is_some());
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
#[test] fn a_project_never_folds_and_lists_one_thread_besides_its_pins() {
    // Alive: one thread asking (in Needs you, not repeated under it) and two running: the newer is its one row
    let mut s = world(); s.apply(&tc("tb", "pa", NOW - 80)); s.apply(&turn("tb", "running", NOW - 70)); s.apply(&tc("tc", "pa", NOW - 60)); s.apply(&turn("tc", "running", NOW - 50));
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    let alive = &sec(&v, "active")["rows"][0];
    assert_eq!((alive["fold"].as_str(), alive["folded"].as_bool(), alive["under"].as_u64(), alive["rest"].as_u64()), (Some(""), Some(false), Some(0), Some(0)), "{alive}");
    assert!(titles_of(&alive["threads"]).is_empty(), "what runs is in Running"); assert_eq!(titles(sec(&v, "running")), ["T tc", "T tb"], "the newest first");
    assert_eq!((alive["running"].as_u64(), alive["needsCount"].as_u64()), (Some(3), Some(1)), "its counts stay on the row");
    assert_eq!(titles(sec(&v, "needs"))[0], "T ta", "a thread of it that asks the person is in Needs you");
    // a search still lists every hit under it
    let open = HashSet::new(); let c = Ctx { query: "T t", ..ctx(&open) };
    assert!(sec(&build(&s, &rows, &Value::Null, &c), "active")["rows"][0]["threads"].as_array().unwrap().len() >= 2);
}
fn titles_of(v: &Value) -> Vec<String> { v.as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap().to_string()).collect() }
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
/// Groups of one name are one group: Haiku called three folders "Personal" (one a linked machine's home) and two "American Embedded"
/// (11 Nov: the menu listed Personal four times); a move kept on any of the merged ids lands in the one group.
fn named_world() -> State {
    let old = NOW - 8 * DAY;
    let mut v = vec![set(GROUPS, "on", 1)];
    for (i, (id, root)) in [("p1", "/w/acme/one"), ("p2", "/w/stuff/two"), ("p3", "/home/someone/three"), ("p4", "/w/ae/four"), ("p5", "/w/ae-inc/five"), ("p6", "/w/nordic/six")].iter().enumerate() {
        v.push(pc(id, &id.to_uppercase(), root, old - 10)); v.push(tc(&format!("t{i}"), id, old)); v.push(turn(&format!("t{i}"), "completed", old));
    }
    for (g, n) in [("folder:/w/acme", "Personal"), ("folder:/w/stuff", "personal"), ("folder:/w/ae", "American Embedded"), ("folder:/w/ae-inc", "American  Embedded"), ("folder:/w/nordic", "Nordic")] { v.push(set(&format!("sidebar.group.pick.{g}"), n, 1)); }
    st(&v)
}
#[test] fn groups_of_one_name_are_one_group_and_keep_their_projects() {
    assert_eq!(auto_group("", "/Users/alex/x").0, "folder:~", "a Mac's home folder is the personal group too");
    assert_eq!(auto_group("", "C:\\Users\\alex\\x").0, "folder:~");
    assert_eq!(auto_group("", "/home/someone/x").0, "folder:~");
    assert_eq!(auto_group("", "/home/someone/src/x").0, "folder:/home/someone/src");
    let mut s = named_world();
    let key6 = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]).iter().find(|p| p.name == "P6").unwrap().key.clone();
    s.apply(&set(&format!("sidebar.group.of.{key6}"), "folder:/w/ae-inc", 2)); // the person moved it there before the merge
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let to = move_targets(&s, &rows);
    let labels: Vec<&str> = to.as_array().unwrap().iter().map(|g| g["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["American Embedded", "Personal"], "each group once ({to})");
    let open: HashSet<String> = ["sec:group:folder:/w/acme".to_string(), "sec:group:folder:/w/ae".to_string()].into();
    let v = build(&s, &rows, &Value::Null, &ctx(&open));
    let groups: Vec<String> = ids(&v).into_iter().filter(|i| i.starts_with("group:")).collect();
    assert_eq!(groups, ["group:folder:/w/ae", "group:folder:/w/acme"], "{v}");
    let mut t = titles(sec(&v, "group:folder:/w/acme")); t.sort(); assert_eq!(t, ["P1", "P2", "P3"]);
    let mut t = titles(sec(&v, "group:folder:/w/ae")); t.sort(); assert_eq!(t, ["P4", "P5", "P6"], "the move kept on a merged id stays");
    // renaming the merged group names each of its ids, so it stays one group
    let a = act(&s, &rows, "group-rename", "folder:/w/ae\u{1f}AE", &json!({}), NOW).unwrap();
    let mut keys: Vec<&str> = a.sets.iter().map(|x| x.0.as_str()).collect(); keys.sort();
    assert_eq!(keys, ["sidebar.group.name.folder:/w/ae", "sidebar.group.name.folder:/w/ae-inc"]);
    // two groups the person names alike are one
    let mut s2 = named_world(); s2.apply(&set("sidebar.group.name.folder:/w/nordic", "personal", 3));
    let rows2 = merged_projects(&s2, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert_eq!(move_targets(&s2, &rows2).as_array().unwrap().len(), 2, "Nordic renamed Personal joined Personal");
    // a move to a merged id writes the group's own id; a new group of an existing name joins it
    let a = act(&s, &rows, "group-move", &format!("{key6}\u{1f}folder:/w/stuff"), &json!({}), NOW).unwrap();
    assert_eq!(a.sets, vec![(format!("sidebar.group.of.{key6}"), "folder:/w/acme".to_string())]);
    let a = act(&s, &rows, "group-move", &format!("{key6}\u{1f}+Personal"), &json!({}), NOW).unwrap();
    let mut s3 = named_world(); for (k, v) in &a.sets { s3.apply(&set(k, v, 4)); }
    let rows3 = merged_projects(&s3, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert_eq!(move_targets(&s3, &rows3).as_array().unwrap().iter().filter(|g| g["label"] == "Personal").count(), 1);
}
fn menu_ids(m: &Value) -> Vec<String> { m.as_array().unwrap().iter().map(|i| i["id"].as_str().unwrap().to_string()).collect() }
#[test] fn a_project_row_has_the_whole_menu() {
    let s = named_world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let open: HashSet<String> = ["sec:group:folder:/w/acme".to_string()].into();
    let v = build(&s, &rows, &Value::Null, &ctx(&open));
    let p1 = sec(&v, "group:folder:/w/acme")["rows"].as_array().unwrap().iter().find(|r| r["title"] == "P1").unwrap().clone();
    let m = &p1["menu"];
    assert_eq!(menu_ids(m), ["pin", "swarm", "rename", "badge", "open", "move", "quiet", "remove"]);
    let labels: Vec<&str> = m.as_array().unwrap().iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["Pin project", "New swarm", "Rename", "Badge", "Open folder", "Move to group", "Make quiet", "Remove project"]);
    let rm = &m[7]; assert_eq!((rm["action"].as_str(), rm["value"].as_str(), rm["confirm"].as_bool()), (Some("proj-remove"), Some("p1"), Some(true)), "Remove asks first and names the project here");
    assert_eq!(m[4]["value"], "/w/acme/one");
    // Move to group: every other group once, never the one it is in, then New group…
    let sub: Vec<&str> = m[5]["sub"].as_array().unwrap().iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert_eq!(sub, ["American Embedded", "Nordic", "New group\u{2026}"]);
    assert_eq!(m[5]["sub"][0]["value"], format!("{}\u{1f}folder:/w/ae", p1["nav"].as_str().unwrap()));
    assert_eq!(m[5]["sub"][2]["action"], "group-new-start");
    // New group… asks a name in the rename bar; its save makes the group with the project in it
    let key = p1["nav"].as_str().unwrap();
    let a = act(&s, &rows, "group-new-start", key, &json!({}), NOW).unwrap();
    assert!(a.drafts.contains(&("@ren.kind".to_string(), "group-new".to_string())) && a.drafts.contains(&("@ren".to_string(), key.to_string())));
    assert_eq!(new_group_sets(key, "  Clients "), Some(vec![(format!("sidebar.group.of.{key}"), "custom:clients".into()), ("sidebar.group.name.custom:clients".into(), "Clients".into())]));
    assert_eq!(new_group_sets(key, "  "), None);
}
#[test] fn a_project_made_quiet_is_in_quiet_until_something_new_happens_or_it_is_brought_back() {
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let key = rows.iter().find(|p| p.name == "Recent").unwrap().key.clone();
    let a = act(&s, &rows, "project-archive", &key, &json!({}), NOW).unwrap();
    assert_eq!(a.sets, vec![(format!("sidebar.archived.{key}"), NOW.to_string()), (format!("sidebar.pin.{key}"), String::new())]);
    let mut s2 = world(); for (k, v) in &a.sets { s2.apply(&set(k, v, NOW)); }
    let rows2 = merged_projects(&s2, &[], &|_| false, &|_| String::new(), "desk", &[]);
    // in Quiet with the projects nothing happened in; there is no Archived section
    let open: HashSet<String> = ["sec:quiet".to_string()].into(); let v = build(&s2, &rows2, &Value::Null, &ctx(&open));
    assert!(!ids(&v).contains(&"archived".to_string()));
    assert!(!titles(sec(&v, "active")).contains(&"Recent".to_string())); assert!(titles(sec(&v, "quiet")).contains(&"Recent".to_string()));
    // the project the open thread is in goes too: Make quiet acts at once
    let mut c = ctx(&open); c.selected = "tr"; assert!(titles(sec(&build(&s2, &rows2, &Value::Null, &c), "quiet")).contains(&"Recent".to_string()));
    let r = sec(&v, "quiet")["rows"].as_array().unwrap().iter().find(|r| r["title"] == "Recent").unwrap().clone();
    assert_eq!(r["quiet"], true); assert_eq!(r["menu"].as_array().unwrap().iter().find(|i| i["id"] == "quiet").unwrap()["label"], "Back to Active");
    assert_eq!(act(&s2, &rows2, "project-archive", &key, &json!({}), NOW).unwrap().sets, vec![(format!("sidebar.archived.{key}"), String::new())], "again: back to Active");
    assert_eq!(act(&s2, &rows2, "project-pin", &key, &json!({}), NOW).unwrap().sets, vec![(format!("sidebar.pin.{key}"), NOW.to_string()), (format!("sidebar.archived.{key}"), String::new())], "pinned: back from Quiet");
    // something new in it: back in Active by itself, and its menu offers Make quiet again
    let mut s3 = world(); s3.apply(&set(&format!("sidebar.archived.{key}"), &(NOW - 5).to_string(), NOW - 5)); s3.apply(&turn("tr", "running", NOW - 1));
    let rows3 = merged_projects(&s3, &[], &|_| false, &|_| String::new(), "desk", &[]); let v3 = build(&s3, &rows3, &Value::Null, &ctx(&open));
    let r3 = sec(&v3, "active")["rows"].as_array().unwrap().iter().find(|r| r["title"] == "Recent").expect("back in Active").clone();
    assert_eq!(r3["menu"].as_array().unwrap().iter().find(|i| i["id"] == "quiet").unwrap()["label"], "Make quiet");
    assert!(act(&s, &rows, "project-archive", "nope", &json!({}), NOW).is_none());
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
    assert_eq!(secs[3]["title"], "Done (2)"); assert!(secs[3]["rows"].as_array().unwrap().is_empty(), "folded");
    let mut done = titles(&page(&s, &rows, &rows[0].key, &ctx(&open), true)["sections"][3]); done.sort();
    assert_eq!(done, ["T d", "T e"], "an archived thread is a settled one now: in Done, never out of sight");
    assert!(page(&s, &rows, "nope", &ctx(&open), false).is_null());
}
#[test] fn ctrl_k_lists_every_project_and_thread_waiting_ones_first() {
    let s = world(); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let open = HashSet::new(); let j = jump(&s, &rows, &ctx(&open)); let a = j.as_array().unwrap();
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
    assert_eq!(titles(sec(&v, "needs")), ["T x"]); assert_eq!(sec(&v, "needs")["rows"][0]["project"], "Chats with no project");
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
    let p2 = &sec(&v2, "active")["rows"][0]; assert!(p2["swarms"].as_array().unwrap().is_empty(), "not repeated under its project"); assert!(p2["running"].as_u64().unwrap() >= 1);
    let r2 = &sec(&v2, "running")["rows"][0]; assert_eq!(r2["nav"], "@room:r1"); assert_eq!(r2["members"], 2);
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
    let j = jump(&s2, &rows2, &ctx(&HashSet::new())); assert!(j.as_array().unwrap().iter().any(|x| x["kind"] == "swarm" && x["action"] == "room" && x["value"] == "h|r1"));
    let pg = page(&s2, &rows2, &rows2[0].key, &ctx(&HashSet::new()), false); assert_eq!(pg["sections"][0]["rows"][0]["kind"], "swarm");
}

// ---- finished threads: counted on the project's row, and the one on show listed under it -----------------------------------
#[test] fn the_one_row_besides_the_pins_is_the_open_thread_and_what_runs_is_in_running() {
    let s = one(&[tc("t2", "p1", NOW - 80), tc("t3", "p1", NOW - 70), turn("t1", "completed", NOW - 50), turn("t2", "completed", NOW - 40),
        json!({"$":"ThreadSettled","id":"t2","manual":true,"at":NOW-30}), turn("t3", "running", NOW - 20)]);
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]); let open = HashSet::new();
    let row = |s: &State, sel: &'static str| { let rows = merged_projects(s, &[], &|_| false, &|_| String::new(), "desk", &[]); let c = Ctx { selected: sel, ..ctx(&open) }; sec(&build(s, &rows, &Value::Null, &c), "active")["rows"][0].clone() };
    let _ = rows;
    let r = row(&s, ""); assert_eq!((titles_of(&r["threads"]), r["rest"].as_u64(), r["running"].as_u64()), (vec![], Some(0), Some(1)), "the running one is in Running, counted here; no count of the rest");
    assert_eq!(titles_of(&row(&s, "t2")["threads"]), ["T t2"], "the one on show has the row");
    assert_eq!(titles_of(&row(&s, "t3")["threads"]), ["T t3"], "the running one too, while it is the one on show");
    // nothing runs: the thread on show has the row, and leaves when another is opened
    let s = one(&[tc("t2", "p1", NOW - 80), turn("t1", "completed", NOW - 50), turn("t2", "completed", NOW - 40), json!({"$":"ThreadSettled","id":"t2","manual":true,"at":NOW-30})]);
    assert_eq!(titles_of(&row(&s, "t2")["threads"]), ["T t2"]);
    assert_eq!(titles_of(&row(&s, "t1")["threads"]), ["T t1"]);
}

// ---- #1186: "Only threads that need attention" (on by default) or every thread --------------------------------------------------
/// On (unset, a new install's and an existing one's alike): under the project its pin and the one thread that runs; the ask is in Needs you.
/// Off: every thread the person has not settled or archived is listed under it, the ask too, pins first, then the newest; one the clock
/// settled stays; the row folds by its chevron, unless the thread on show is under it.
#[test] fn the_thread_list_shows_what_needs_attention_by_default_and_every_thread_when_that_is_off() {
    let settle = |id: &str, manual: bool, at: u64| json!({"$":"ThreadSettled","id":id,"manual":manual,"at":at});
    let evs = vec![tc("t2", "p1", NOW - 85), tc("t3", "p1", NOW - 80), tc("t4", "p1", NOW - 75), tc("t5", "p1", NOW - 70), tc("t6", "p1", NOW - 65), tc("t7", "p1", NOW - 60), tc("t8", "p1", NOW - 55),
        turn("t2", "completed", NOW - 50), turn("t3", "running", NOW - 45), turn("t4", "completed", NOW - 44), settle("t4", false, NOW - 43),
        turn("t5", "completed", NOW - 42), settle("t5", true, NOW - 41), json!({"$":"ThreadArchived","id":"t6","archived":true,"at":NOW-40}),
        ask("q7", "t7", "question"), turn("t8", "completed", NOW - 38), json!({"$":"ThreadPinned","id":"t8","pinned":true,"at":NOW-37})];
    let open = HashSet::new();
    let view = |s: &State, sel: &'static str| { let rows = merged_projects(s, &[], &|_| false, &|_| String::new(), "desk", &[]); let c = Ctx { selected: sel, ..ctx(&open) }; build(s, &rows, &Value::Null, &c) };
    let s = one(&evs);
    assert!(attention_only(&s), "on unless the person turns it off");
    let v = view(&s, ""); let p = &sec(&v, "active")["rows"][0];
    assert_eq!(titles_of(&p["threads"]), ["T t8"], "the pin"); assert_eq!(titles_of(&sec(&v, "running")["rows"]), ["T t3"], "what runs is in Running");
    assert_eq!(titles_of(&sec(&v, "needs")["rows"]), ["T t7"]);
    assert_eq!(p["fold"], "", "no chevron");
    let mut off = evs.clone(); off.push(set(ATTENTION, "off", NOW - 1));
    let s = one(&off);
    assert!(!attention_only(&s));
    let v = view(&s, ""); let p = &sec(&v, "active")["rows"][0];
    let t = titles_of(&p["threads"]);
    assert_eq!(t[0], "T t8", "the pin first");
    let mut rest = t[1..].to_vec(); rest.sort();
    assert_eq!(rest, ["T t1", "T t3", "T t7"], "active/unsettled threads stay directly under the project: {t:?}");
    let acts: Vec<u64> = p["threads"].as_array().unwrap()[1..].iter().map(|r| r["activity"].as_u64().unwrap()).collect();
    assert!(acts.windows(2).all(|w| w[0] >= w[1]), "newest first: {acts:?}");
    assert_eq!((p["doneCount"].as_u64(), p["doneOpen"].as_bool(), p["done"].as_array().unwrap().len()), (Some(4), Some(false), 0), "completed and settled threads collapse into Done (4) by default: {p}");
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]); let key = rows[0].key.clone();
    assert_eq!(p["doneToggle"].as_str(), Some(format!("proj-done:{key}").as_str()));
    assert_eq!(act(&s, &rows, "side-section", &format!("proj-done:{key}"), &json!({}), NOW).unwrap().open_toggle.as_deref(), Some(format!("proj-done:{key}").as_str()));
    assert!(act(&s, &rows, "side-section", "proj-done:nope", &json!({}), NOW).is_none());
    let done_open_set: HashSet<String> = [format!("proj-done:{key}")].into();
    let v_done_open = build(&s, &rows, &Value::Null, &ctx(&done_open_set));
    let p_open = &sec(&v_done_open, "active")["rows"][0];
    assert_eq!(p_open["doneOpen"], true);
    assert_eq!(titles_of(&p_open["done"]), ["T t5", "T t4", "T t2", "T t6"], "expanded Done fold lists completed and settled threads newest first");
    assert_eq!(p_open["done"][2]["settle"], "h|t2", "unsettled completed thread still exposes settle when expanded");
    assert_eq!(p_open["done"][3]["settle"], "", "archived (settled) thread does not");
    assert_eq!(p_open["done"][0]["settle"], "", "manually settled thread does not");
    assert_eq!(p_open["done"][1]["settle"], "", "clock-settled thread does not");
    assert_eq!(titles_of(&sec(&v, "needs")["rows"]), ["T t7"], "Needs you is the same");
    assert_eq!((p["fold"].as_str(), p["folded"].as_bool(), p["under"].as_u64()), (Some("project-fold"), Some(false), Some(8)));
    // the chevron folds them (the hub's project-fold), and the thread on show opens the row again
    let a = act(&s, &rows, "project-fold", &key, &json!({}), NOW).unwrap();
    let mut folded = off.clone(); for (k, v) in &a.sets { folded.push(set(k, v, NOW)); }
    let s = one(&folded);
    let p = sec(&view(&s, ""), "active")["rows"][0].clone();
    assert_eq!((p["folded"].as_bool(), p["threads"].as_array().unwrap().len(), p["doneCount"].as_u64().unwrap_or(0), p["done"].as_array().map_or(0, Vec::len)), (Some(true), 0, 0, 0));
    let v_sel = view(&s, "t2");
    let p_sel = &sec(&v_sel, "active")["rows"][0];
    assert_eq!(p_sel["folded"], false, "the open thread's project is never folded away");
    assert_eq!(p_sel["threads"].as_array().unwrap().len(), 5, "the single selected Done thread stays visible without expanding the rest of Done");
    assert_eq!(p_sel["doneCount"], 3, "the remaining three Done threads stay collapsed");
    // the search lists what matches either way
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]); let c = Ctx { query: "t5", ..ctx(&open) };
    assert_eq!(titles_of(&sec(&build(&s, &rows, &Value::Null, &c), "active")["rows"][0]["threads"]), ["T t5"]);
    // the Settings row, under Appearance
    let r = crate::present::settings::view(&s, &json!({"drafts":{"@settings.page":"appearance"}}), &json!({}));
    let row = r["rows"].as_array().unwrap().iter().find(|r| r["label"] == "Only threads that need attention").expect("the row").clone();
    assert_eq!(row["buttons"].as_array().unwrap().iter().map(|b| (b["value"].as_str().unwrap(), b["on"].as_bool().unwrap())).collect::<Vec<_>>(), [("sidebar.attention=on", false), ("sidebar.attention=off", true)]);
}

#[test] fn settled_and_done_threads_collapse_into_project_fold_and_unsettled_threads_expose_settle() {
    let settle_ev = |id: &str, manual: bool, at: u64| json!({"$":"ThreadSettled","id":id,"manual":manual,"at":at});
    let unsettle_ev = |id: &str, at: u64| json!({"$":"ThreadUnsettled","id":id,"at":at});
    let mut evs = vec![
        tc("t_run", "p1", NOW - 90), turn("t_run", "running", NOW - 10),
        tc("t_need", "p1", NOW - 85), ask("q_need", "t_need", "approval"),
        tc("t_fail", "p1", NOW - 80), turn("t_fail", "failed", NOW - 20),
        tc("t_pin", "p1", NOW - 75), turn("t_pin", "completed", NOW - 30), json!({"$":"ThreadPinned","id":"t_pin","pinned":true,"at":NOW-29}),
        tc("t_done_3h", "p1", NOW - 70), turn("t_done_3h", "completed", NOW - 3 * 3600),
        tc("t_done_1d", "p1", NOW - 65), turn("t_done_1d", "completed", NOW - 86400),
        tc("t_settled", "p1", NOW - 60), turn("t_settled", "completed", NOW - 2 * 86400), settle_ev("t_settled", true, NOW - 86400),
        set(ATTENTION, "off", NOW - 1),
    ];
    let s = one(&evs);
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let key = rows[0].key.clone();
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    let p = &sec(&v, "active")["rows"][0];
    let active_titles = titles_of(&p["threads"]);
    assert_eq!(active_titles, ["T t_pin", "T t_run", "T t_fail", "T t_need", "T t1"], "active/pinned/failed/idle stay visible under project: {active_titles:?}");
    assert_eq!(p["doneCount"], 3);
    assert_eq!(p["doneOpen"], false);
    assert!(p["done"].as_array().unwrap().is_empty());

    // Unsettled non-running threads expose `settle`, and header tools expose Settle / Settled
    let fail_row = p["threads"].as_array().unwrap().iter().find(|r| r["id"] == "h|t_fail").unwrap();
    let pin_row = p["threads"].as_array().unwrap().iter().find(|r| r["id"] == "h|t_pin").unwrap();
    let run_row = p["threads"].as_array().unwrap().iter().find(|r| r["id"] == "h|t_run").unwrap();
    assert_eq!(fail_row["settle"], "h|t_fail");
    assert_eq!(pin_row["settle"], "h|t_pin");
    assert_eq!(run_row["settle"], "");
    let tools_done = crate::present::keys::tools(Turn::Completed, false, crate::present::keys::settled(&s, "t_done_3h"), false, false);
    assert!(tools_done.iter().any(|t| t["action"] == "settle" && t["label"] == "Settle"));
    let tools_settled = crate::present::keys::tools(Turn::Completed, false, crate::present::keys::settled(&s, "t_settled"), false, false);
    assert!(tools_settled.iter().any(|t| t["action"] == "unsettle" && t["label"] == "Settled"));

    // Settling t_fail moves it into the collapsed Done fold; unsettling t_settled brings it back to active threads
    evs.push(settle_ev("t_fail", true, NOW));
    evs.push(unsettle_ev("t_settled", NOW));
    let s2 = one(&evs);
    let rows2 = merged_projects(&s2, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v2 = build(&s2, &rows2, &Value::Null, &ctx(&HashSet::new()));
    let p2 = &sec(&v2, "active")["rows"][0];
    let active2 = titles_of(&p2["threads"]);
    assert!(!active2.contains(&"T t_fail".to_string()), "settled thread moved into collapsed fold: {active2:?}");
    assert!(active2.contains(&"T t_settled".to_string()), "unsettled thread returned to active list: {active2:?}");
    let unsettled_row = p2["threads"].as_array().unwrap().iter().find(|r| r["id"] == "h|t_settled").unwrap();
    assert_eq!(unsettled_row["settle"], "h|t_settled");
    assert_eq!(p2["doneCount"], 3);

    // Expanding the Done fold reveals all collapsed Done/settled threads
    let open_done: HashSet<String> = [format!("proj-done:{key}")].into();
    let v3 = build(&s2, &rows2, &Value::Null, &ctx(&open_done));
    let p3 = &sec(&v3, "active")["rows"][0];
    assert_eq!(p3["doneOpen"], true);
    let mut done_titles = titles_of(&p3["done"]); done_titles.sort();
    assert_eq!(done_titles, ["T t_done_1d", "T t_done_3h", "T t_fail"]);
}

#[test] fn a_swarms_needs_you_row_says_what_happened_in_one_line_and_carries_the_actions_that_resolve_it() {
    let room = |state: &str, reason: &str| json!({"$":"RoomSet","id":"r1","name":"Web UI","members":"thread:a\nthread:b","kind":"swarm","project":"p1","state":state,"reason":reason,"at":NOW-30});
    let member = |t: &str, n: &str| set(&format!("room.member.r1.thread:{t}"), &json!({"name":n}).to_string(), NOW - 40);
    let row = |extra: Vec<Value>| { let mut e = vec![pc("p1", "P", "/n/p", 1), tc("a", "p1", 2), tc("b", "p1", 3), member("a", "opus"), member("b", "gemini")]; e.extend(extra);
        let s = st(&e); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]); sec(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new())), "needs")["rows"][0].clone() };
    let r = row(vec![room("paused", "idle")]);
    assert_eq!((r["say"].as_str(), r["need"]["line"].as_str()), (Some("Paused: all members idle 10 min"), Some("Paused: all members idle 10 min")), "{r}");
    assert_eq!(r["need"]["actions"], json!([{"label":"Resume","act":"resume","room":"h|r1","thread":""}]));
    let r = row(vec![room("paused", "budget"), set("room.need.r1", r#"{"for":"person","part":"tokens"}"#, NOW - 20)]);
    assert_eq!(r["say"], "Paused: budget used (tokens)"); assert_eq!(r["need"]["actions"][0]["act"], "raise");
    assert_eq!(row(vec![room("paused", "circular")])["need"]["actions"][0]["label"], "Read board");
    // a bot's swarm paused for its starter: not the person's
    let mut e = vec![pc("p1", "P", "/n/p", 1), tc("a", "p1", 2), tc("b", "p1", 3), room("paused", "idle"), set("room.need.r1", r#"{"for":"starter"}"#, NOW - 20)];
    let s = st(&e); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert!(sec(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new())), "needs").is_null(), "a routine pause of a bot's swarm reaches its starter only");
    // a member that failed on its sign-in: named, with Sign in and Move to Claude, even in a bot's swarm
    e[2] = json!({"$":"ThreadCreated","id":"b","project":"p1","title":"T b","env":"local","branch":"","worktree":"","provider":"antigravity","model":"","at":3});
    e.extend([member("b", "gemini"), turn("b", "failed", NOW - 10), set("room.need.r1", r#"{"for":"starter","failed":[{"thread":"b","cause":"sign-in","say":"You are not logged into Antigravity"}]}"#, NOW - 5)]);
    let s = st(&e); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let r = sec(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new())), "needs")["rows"][0].clone();
    assert_eq!((r["say"].as_str(), r["want"].as_str()), (Some("Member gemini failed: Gemini sign-in expired"), Some("sign-in")), "{r}");
    let labels: Vec<&str> = r["need"]["actions"].as_array().unwrap().iter().map(|a| a["label"].as_str().unwrap()).collect(); assert_eq!(labels, ["Sign in to Gemini", "Move to Claude"]);
    assert_eq!(r["need"]["actions"][0]["thread"], "h|b");
}

#[test] fn a_member_of_a_swarm_that_is_over_is_a_thread_again_and_its_open_ask_is_in_needs_you() {
    let room = |state: &str| json!({"$":"RoomSet","id":"r1","name":"Web UI","members":"thread:a\nthread:b","kind":"swarm","project":"p1","state":state,"reason":"","at":NOW-30});
    let s = st(&[pc("p1", "P", "/n/p", 1), tc("a", "p1", 2), tc("b", "p1", 3), room("done"), ask("q", "a", "approval")]);
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let n = sec(&build(&s, &rows, &Value::Null, &ctx(&HashSet::new())), "needs").clone();
    assert_eq!(n["count"], 1, "{n}"); assert_eq!(n["rows"][0]["kind"], Value::Null, "the thread itself, no swarm row: {n}");
    assert!(!crate::state::visible::swarm_member(&s, "a"));
    let live = st(&[pc("p1", "P", "/n/p", 1), tc("a", "p1", 2), tc("b", "p1", 3), room(""), ask("q", "a", "approval")]);
    assert!(crate::state::visible::swarm_member(&live, "a"), "a live swarm's member is its row's");
}

#[test] fn mailroom_runs_and_historical_bub_launch_threads_are_hidden_everywhere() {
    // Historical run* ("Bub: Launch: ...") and mailcoord-* threads in events.jsonl, with or without RunSet, plus mirrored and registry rows
    let mk_tc = |id: &str, proj: &str, title: &str, at: u64| json!({"$":"ThreadCreated","id":id,"project":proj,"title":title,"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":at});
    let s = st(&[
        pc("p1", "Backplane", "/n/bp", 1),
        mk_tc("t1", "p1", "User thread", NOW - 500),
        mk_tc("runner", "p1", "Test runner", NOW - 400), // normal id starting with "run" + letter: must NOT be hidden
        // Historical mailroom run with project="p1" and RunSet, currently running + has an open ask
        mk_tc("run1794092771-1", "p1", "Bub: Launch: Fix bug", NOW - 10),
        json!({"$":"RunSet","thread":"run1794092771-1","bot":"b1","conv":"c1","mail":"m1","at":NOW - 10}),
        turn("run1794092771-1", "running", NOW - 10),
        ask("a1", "run1794092771-1", "question"),
        // Historical mailroom run with project="p1" and NO RunSet (e.g. older log)
        mk_tc("run1794092771-2", "p1", "Bub: Launch: Old report", NOW - 5),
        // Private coordination lane (with project="" and with project="p1")
        mk_tc("mailcoord-b1-turn1", "p1", "Bub: private coordination", NOW - 4),
        mk_tc("mailcoord-b1-turn2", "", "Bub: private coordination", NOW - 3),
        ask("a2", "mailcoord-b1-turn2", "question"),
        // Show-all-threads mode ("Only threads that need attention" = off)
        set(ATTENTION, "off", NOW - 1),
    ]);
    let far_st = st(&[
        pc("L~p1", "Backplane", "/f/bp", 1),
        mk_tc("L~t2", "L~p1", "Remote user thread", NOW - 300),
        mk_tc("L~run1794092771-9", "L~p1", "Bub: Launch: Remote run", NOW - 2),
        mk_tc("L~mailcoord-b1-turn9", "L~p1", "Bub: private coordination", NOW - 1),
    ]);
    let reg = vec![RegProject {
        key: "desk/p1".into(), name: "Backplane".into(), origin: String::new(),
        machines: vec![RegMachine { machine: "offbox".into(), id: "p9".into(), folder: "/o/bp".into(), seen: NOW - 600 }],
        threads: vec![
            RegThread { machine: "offbox".into(), id: "offbox~t3".into(), title: "Offline user thread".into(), status: "idle".into(), activity: NOW - 600, pinned: false },
            RegThread { machine: "offbox".into(), id: "offbox~run1794092771-7".into(), title: "Bub: Launch: Offline run".into(), status: "running".into(), activity: NOW - 1, pinned: false },
        ],
    }];
    let origin = |id: &str| if id == "L~p1" { "desk/p1".to_string() } else { String::new() };
    let far = vec![("laptop".to_string(), &far_st)];
    let rows = merged_projects(&s, &far, &|_| false, &origin, "desk", &reg);
    let bp = rows.iter().find(|p| p.key == "desk/p1").unwrap();
    let thread_ids: Vec<&str> = bp.threads.iter().map(|t| t.id.as_str()).collect();
    assert!(thread_ids.contains(&"t1") && thread_ids.contains(&"runner"), "normal threads kept: {thread_ids:?}");
    assert!(!thread_ids.iter().any(|id| id.contains("run1794") || id.contains("mailcoord-")), "no mailroom runs in merged project: {thread_ids:?}");
    assert_eq!(bp.last_activity, NOW - 300, "mailroom runs must not bump project last_activity");

    // Sidebar (with attention_only=off AND Needs you)
    let side = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    let side_str = side.to_string();
    assert!(!side_str.contains("Bub: Launch:") && !side_str.contains("mailcoord-"), "no mailroom runs in sidebar or Needs you: {side_str}");
    assert!(side_str.contains("User thread") && side_str.contains("Test runner"));

    // Project page (Running, Needs you, Recent, Done)
    let pg = page(&s, &rows, &bp.key, &ctx(&HashSet::new()), true);
    let pg_str = pg.to_string();
    assert!(!pg_str.contains("Bub: Launch:") && !pg_str.contains("mailcoord-"), "no mailroom runs on project page: {pg_str}");
    assert!(pg_str.contains("User thread") && pg_str.contains("Test runner"));

    // Ctrl+K command palette
    let jmp = jump(&s, &rows, &ctx(&HashSet::new()));
    let jmp_str = jmp.to_string();
    assert!(!jmp_str.contains("Bub: Launch:") && !jmp_str.contains("mailcoord-"), "no mailroom runs in Ctrl+K: {jmp_str}");
    assert!(jmp_str.contains("User thread") && jmp_str.contains("Test runner"));
}

// ---- the bench: Recent holds the projects the person was in last, the rest fold into All projects ----------------------------------
#[test] fn recent_holds_the_last_visited_projects_and_the_rest_fold_into_all_projects() {
    // twelve projects, each with one finished thread, the higher the number the newer its activity
    let mut evs = vec![]; for i in 0..12u64 { let (p, t) = (format!("q{i:02}"), format!("u{i:02}")); evs.push(pc(&p, &format!("N{i:02}"), &format!("/n/q{i:02}"), NOW - 5 * DAY)); evs.push(tc(&t, &p, NOW - 4 * DAY + i)); evs.push(turn(&t, "completed", NOW - 4 * DAY + i)); }
    let s = st(&evs); let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    assert_eq!(ids(&v), ["active", "all"]); assert_eq!(sec(&v, "active")["title"], "Recent");
    assert_eq!(titles(sec(&v, "active")), ["N04", "N05", "N06", "N07", "N08", "N09", "N10", "N11"], "never visited: the BENCH newest by activity; on the bench by name, so no row moves as the person goes between them");
    assert_eq!((sec(&v, "all")["count"].as_u64(), sec(&v, "all")["open"].as_bool(), sec(&v, "all")["rows"].as_array().unwrap().len()), (Some(4), Some(false), 0), "the rest: one folded line");
    // opened: by name
    let open: HashSet<String> = ["sec:all".to_string()].into();
    assert_eq!(titles(sec(&build(&s, &rows, &Value::Null, &ctx(&open)), "all")), ["N00", "N01", "N02", "N03"]);
    assert_eq!(act(&s, &rows, "side-section", "sec:all", &json!({}), NOW).unwrap().open_toggle.as_deref(), Some("sec:all"));
    // a visit (its page opened, or a thread of it) brings an old one to the top, whatever the agents did since
    let key = rows.iter().find(|p| p.name == "N01").unwrap().key.clone();
    let a = act(&s, &rows, "project-page", &key, &json!({}), NOW).unwrap(); assert_eq!(a.sets, vec![("sidebar.visit.q01".to_string(), NOW.to_string())]);
    let mut s2 = st(&evs); s2.apply(&set("sidebar.visit.q01", &NOW.to_string(), NOW));
    let rows2 = merged_projects(&s2, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert_eq!(titles(sec(&build(&s2, &rows2, &Value::Null, &ctx(&HashSet::new())), "active")), ["N01", "N05", "N06", "N07", "N08", "N09", "N10", "N11"], "it took the oldest one's place");
    assert!(act(&s2, &rows2, "project-page", &key, &json!({}), NOW + 60).unwrap().sets.is_empty(), "at most one visit line in ten minutes");
    assert_eq!(crate::state::visible::visit(&s2, "u02", NOW)[0]["key"], "sidebar.visit.q02"); assert!(crate::state::visible::visit(&s2, "u01", NOW + 60).is_empty());
    // the open thread's project is on the bench, wherever it was
    let none = HashSet::new(); let c = Ctx { selected: "u00", ..ctx(&none) };
    assert_eq!(titles(sec(&build(&s, &rows, &Value::Null, &c), "active")), ["N00", "N05", "N06", "N07", "N08", "N09", "N10", "N11"]);
    // a search lists every hit: All projects opens by itself
    let c = Ctx { query: "N02", ..ctx(&none) }; let v = build(&s, &rows, &Value::Null, &c);
    assert_eq!(titles(sec(&v, "active")), ["N02"]);
}

// ---- status labels and snoozing (the person, 2026-11-23) ----------------------------------------------------------------------------------
#[test] fn a_row_says_its_status_in_a_word_and_a_snoozed_thread_waits_in_its_own_fold() {
    assert_eq!([badge("needs", "approval"), badge("needs", "question"), badge("needs", "sign-in"), badge("failed", "failed"), badge("cut", "stopped"), badge("running", ""), badge("monitoring", ""), badge("done", ""), badge("idle", "")],
        [("Approve", "bad"), ("Input", "bad"), ("Sign in", "bad"), ("Failed", "bad"), ("Stopped", "bad"), ("Working", "run"), ("Waiting", "wait"), ("Done", "ok"), ("", "")]);
    let snz = |id: &str, until: u64| json!({"$":"ThreadSnoozed","id":id,"until":until,"at":NOW-5});
    let evs = vec![tc("t2", "p1", NOW - 80), tc("t3", "p1", NOW - 70), turn("t1", "failed", NOW - 60), turn("t2", "running", NOW - 50), ask("q3", "t3", "question")];
    let view = |s: &State, open: &HashSet<String>| { let rows = merged_projects(s, &[], &|_| false, &|_| String::new(), "desk", &[]); build(s, &rows, &Value::Null, &ctx(open)) };
    let none = HashSet::new(); let v = view(&one(&evs), &none);
    assert_eq!(ids(&v), ["needs", "running", "active"]);
    let n = &sec(&v, "needs")["rows"]; assert_eq!((n[0]["label"].as_str(), n[0]["tone"].as_str()), (Some("Input"), Some("bad"))); assert_eq!((n[1]["label"].as_str(), n[1]["settle"].as_str()), (Some("Failed"), Some("h|t1")));
    assert_eq!(n[0]["snooze"], json!([]), "an open ask is answered, not snoozed"); assert_eq!(n[1]["snooze"][0], json!({"label":"1 hour","value":"h|t1|1h"}));
    let r = &sec(&v, "running")["rows"][0]; assert_eq!((r["label"].as_str(), r["tone"].as_str(), r["snooze"].as_array().map(Vec::len)), (Some("Working"), Some("run"), Some(3)));
    // snoozed: the stop and the running one leave their lists for one folded line; the ask stays whatever its snooze
    let mut e2 = evs.clone(); e2.extend([snz("t1", NOW + 3600), snz("t2", NOW + 7200), snz("t3", NOW + 3600)]);
    let s2 = one(&e2); let v = view(&s2, &none);
    assert_eq!(ids(&v), ["needs", "snoozed", "active"]); assert_eq!(titles(sec(&v, "needs")), ["T t3"]);
    let z = sec(&v, "snoozed"); assert_eq!((z["count"].as_u64(), z["fold"].as_bool(), z["open"].as_bool(), z["rows"].as_array().map(Vec::len)), (Some(2), Some(true), Some(false), Some(0)));
    let p = &sec(&v, "active")["rows"][0]; assert_eq!((p["running"].as_u64(), p["needsCount"].as_u64()), (Some(0), Some(1)), "a snoozed thread is not counted on its project");
    let open: HashSet<String> = ["sec:snoozed".to_string()].into(); let v = view(&s2, &open); let z = sec(&v, "snoozed");
    assert_eq!(titles(z), ["T t1", "T t2"], "soonest to wake first"); assert_eq!((z["rows"][0]["wake"].as_str(), z["rows"][0]["ago"].as_str()), (Some("h|t1|off"), Some("1h")));
    // its time passed: back where it was
    let mut e3 = evs.clone(); e3.push(snz("t1", NOW - 1)); assert_eq!(ids(&view(&one(&e3), &none)), ["needs", "running", "active"]);
}

#[test] fn collapsed_stale_and_settled_swarms_respect_sidebar_visibility_and_stay_in_history() {
    let room = |id: &str, state: &str, reason: &str, at: u64| json!({"$":"RoomSet","id":id,"name":format!("Swarm {id}"),"members":"thread:a\nthread:b","kind":"swarm","project":"p1","state":state,"reason":reason,"at":at});
    // 1. Collapsed project + 0-working idle swarm (even when attention_only is off): hidden from sidebar, running==0, needsCount==0
    let mut evs = vec![
        pc("p1", "Backplane", "/n/bp", NOW - 100),
        tc("a", "p1", NOW - 40), tc("b", "p1", NOW - 35),
        set(ATTENTION, "off", NOW - 10),
        room("r_idle", "", "", NOW - 30),
    ];
    let s_tmp = st(&evs);
    let key = merged_projects(&s_tmp, &[], &|_| false, &|_| String::new(), "desk", &[])[0].key.clone();
    evs.push(set(&format!("sidebar.fold.{key}"), &(NOW - 5).to_string(), NOW - 5));
    let s = st(&evs);
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v = build(&s, &rows, &Value::Null, &ctx(&HashSet::new()));
    let bp = &sec(&v, "active")["rows"][0];
    assert_eq!(bp["folded"], true);
    assert!(bp["swarms"].as_array().unwrap().is_empty(), "collapsed project hides 0-working idle swarm: {bp}");
    assert_eq!((bp["running"].as_u64(), bp["needsCount"].as_u64()), (Some(0), Some(0)));
    assert!(sec(&v, "running").is_null() && sec(&v, "needs").is_null());

    // 2. Collapsed project + working > 0 swarm: shown under collapsed project and counted as running
    let mut evs_work = evs.clone();
    evs_work.push(turn("a", "running", NOW - 1));
    let s_work = st(&evs_work);
    let rows_work = merged_projects(&s_work, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v_work = build(&s_work, &rows_work, &Value::Null, &ctx(&HashSet::new()));
    let bp_work = &sec(&v_work, "active")["rows"][0];
    assert_eq!(bp_work["swarms"].as_array().unwrap().len(), 1);
    assert!(bp_work["running"].as_u64().unwrap() >= 1);

    // 3. Collapsed project + needs swarm (open approval on a member): shown in Needs you and needsCount==1
    let mut evs_ask = evs.clone();
    evs_ask.push(ask("q1", "a", "approval"));
    let s_ask = st(&evs_ask);
    let rows_ask = merged_projects(&s_ask, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v_ask = build(&s_ask, &rows_ask, &Value::Null, &ctx(&HashSet::new()));
    assert_eq!(sec(&v_ask, "needs")["rows"][0]["nav"], "@room:r_idle");
    assert_eq!(sec(&v_ask, "active")["rows"][0]["needsCount"], 1);

    // 4. Stale (> 24h) 0-working swarm: hidden from sidebar even when expanded, NOT in Running/Needs you, still in Project Page Recent and Ctrl+K
    let stale_at = NOW - crate::sidebar::SWARM_STALE - 60;
    let s_stale = st(&[
        pc("p1", "Backplane", "/n/bp", NOW - 100),
        tc("a", "p1", NOW - 40), tc("b", "p1", NOW - 35),
        set(ATTENTION, "off", NOW - 10),
        room("r_old", "", "", stale_at),
    ]);
    let rows_stale = merged_projects(&s_stale, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v_stale = build(&s_stale, &rows_stale, &Value::Null, &ctx(&HashSet::new()));
    let bp_stale = &sec(&v_stale, "active")["rows"][0];
    assert!(bp_stale["swarms"].as_array().unwrap().is_empty(), "stale 0-working swarm hidden even when expanded");
    assert!(sec(&v_stale, "running").is_null() && sec(&v_stale, "needs").is_null());
    let pg_stale = page(&s_stale, &rows_stale, &rows_stale[0].key, &ctx(&HashSet::new()), true);
    assert!(titles(&pg_stale["sections"][2]).contains(&"Swarm r_old".to_string()), "stale live swarm remains on Project Page Recent: {:?}", pg_stale["sections"][2]);
    assert!(jump(&s_stale, &rows_stale, &ctx(&HashSet::new())).as_array().unwrap().iter().any(|x| x["kind"] == "swarm" && x["value"] == "h|r_old"));

    // 5. swarm-settle action dismisses paused/idle swarm to Project Page Done and keeps it in Ctrl+K; newer post resurfaces it
    let mut paused_evs = vec![
        pc("p1", "Backplane", "/n/bp", NOW - 100),
        tc("a", "p1", NOW - 40), tc("b", "p1", NOW - 35),
        set(ATTENTION, "off", NOW - 10),
        room("r_pause", "paused", "circular", NOW - 30),
    ];
    let s_paused = st(&paused_evs);
    let rows_paused = merged_projects(&s_paused, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let a = act(&s_paused, &rows_paused, "swarm-settle", "h|r_pause", &json!({}), NOW).unwrap();
    assert_eq!(a.sets, vec![("sidebar.swarm.settled.r_pause".to_string(), NOW.to_string())]);
    for (k, val) in &a.sets { paused_evs.push(set(k, val, NOW)); }
    let s_settled = st(&paused_evs);
    let rows_settled = merged_projects(&s_settled, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v_settled = build(&s_settled, &rows_settled, &Value::Null, &ctx(&HashSet::new()));
    assert!(sec(&v_settled, "needs").is_null(), "settled swarm leaves Needs you");
    assert!(sec(&v_settled, "active")["rows"][0]["swarms"].as_array().unwrap().is_empty());
    let pg_settled = page(&s_settled, &rows_settled, &rows_settled[0].key, &ctx(&HashSet::new()), true);
    assert_eq!(titles(&pg_settled["sections"][3]), ["Swarm r_pause"], "settled swarm moves to Project Page Done");
    assert!(jump(&s_settled, &rows_settled, &ctx(&HashSet::new())).as_array().unwrap().iter().any(|x| x["kind"] == "swarm" && x["value"] == "h|r_pause"));
    // Newer post after settle brings it back
    paused_evs.push(json!({"$":"RoomPosted","id":"p2","room":"r_pause","from":"opus","text":"new","hop":1,"at":NOW + 10}));
    let s_resurf = st(&paused_evs);
    let rows_resurf = merged_projects(&s_resurf, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v_resurf = build(&s_resurf, &rows_resurf, &Value::Null, &ctx(&HashSet::new()));
    assert_eq!(sec(&v_resurf, "needs")["rows"][0]["nav"], "@room:r_pause");

    // 6. Finished (done / stopped) swarm is hidden from sidebar and appears in Project Page Done and Ctrl+K
    let s_done = st(&[
        pc("p1", "Backplane", "/n/bp", NOW - 100),
        tc("a", "p1", NOW - 40), tc("b", "p1", NOW - 35),
        set(ATTENTION, "off", NOW - 10),
        room("r_done", "done", "", NOW - 20),
        room("r_stop", "stopped", "person", NOW - 25),
    ]);
    let rows_done = merged_projects(&s_done, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let v_done = build(&s_done, &rows_done, &Value::Null, &ctx(&HashSet::new()));
    assert!(sec(&v_done, "active")["rows"][0]["swarms"].as_array().unwrap().is_empty());
    let pg_done = page(&s_done, &rows_done, &rows_done[0].key, &ctx(&HashSet::new()), true);
    let done_titles = titles(&pg_done["sections"][3]);
    assert!(done_titles.contains(&"Swarm r_done".to_string()) && done_titles.contains(&"Swarm r_stop".to_string()), "{done_titles:?}");
    let jmp = jump(&s_done, &rows_done, &ctx(&HashSet::new()));
    assert!(jmp.as_array().unwrap().iter().any(|x| x["kind"] == "swarm" && x["value"] == "h|r_done"));
    assert!(jmp.as_array().unwrap().iter().any(|x| x["kind"] == "swarm" && x["value"] == "h|r_stop"));
}

#[test] fn quick_chats_appear_in_chats_section_and_jump_and_offer_move_to_project() {
    let base = vec![
        pc("p1", "Beta", "/nowhere/beta", NOW - 200),
        pc("p2", "Alpha", "/nowhere/alpha", NOW - 190),
    ];
    let p2_key = merged_projects(&st(&base), &[], &|_| false, &|_| String::new(), "desk", &[])
        .into_iter().find(|r| r.name == "Alpha").unwrap().key;
    let mut evs = base;
    evs.extend([
        set(&format!("sidebar.pin.{p2_key}"), &NOW.to_string(), NOW - 185),
        tc("t1", "p1", NOW - 180),
        turn("t1", "failed", NOW - 160), json!({"$":"ThreadSnoozed","id":"t1","until":NOW + 3600,"at":NOW - 150}),
        tc("c1", "chats", NOW - 100),
        tc("c2", "chats", NOW - 90),
        tc("c3", "chats", NOW - 80), json!({"$":"ThreadSettled","id":"c3","manual":true,"at":NOW - 70}),
    ]);
    let s = st(&evs);
    let rows = merged_projects(&s, &[], &|_| false, &|_| String::new(), "desk", &[]);
    let none = HashSet::new();
    let v = build(&s, &rows, &Value::Null, &ctx(&none));
    // chats appears after snoozed and before pinned (and active)
    assert_eq!(ids(&v), ["snoozed", "chats", "pinned", "active"]);
    let cs = sec(&v, "chats");
    assert_eq!((cs["kind"].as_str(), cs["title"].as_str(), cs["count"].as_u64()), (Some("threads"), Some("Chats"), Some(2)));
    assert_eq!(titles(cs), ["T c2", "T c1"]);
    let r = &cs["rows"][1];
    assert_eq!((r["project"].as_str(), r["projectKey"].as_str(), r["chat"].as_bool(), r["canChild"].as_bool()),
        (Some("Chats"), Some(""), Some(true), Some(false)));
    let mv = r["menu"].as_array().unwrap().iter().find(|m| m["id"] == "move").expect("move menu item");
    assert_eq!(mv["label"], "Move to project\u{2026}");
    assert_eq!(mv["sub"][0]["label"], "Alpha");
    assert_eq!(mv["sub"][0]["action"], "row-move-project");
    assert_eq!(mv["sub"][0]["value"], "h|c1\u{1f}p2");
    assert_eq!(mv["sub"][1]["label"], "Beta");
    // selected settled chat shows up; query filters chats by title
    let v_sel = build(&s, &rows, &Value::Null, &Ctx { selected: "c3", ..ctx(&none) });
    assert_eq!(titles(sec(&v_sel, "chats")), ["T c3", "T c2", "T c1"]);
    let v_q = build(&s, &rows, &Value::Null, &Ctx { query: "c1", ..ctx(&none) });
    assert_eq!(titles(sec(&v_q, "chats")), ["T c1"]);
    // jump includes settled quick chats too with place "Chats"
    let jmp = jump(&s, &rows, &ctx(&none));
    let j_c3 = jmp.as_array().unwrap().iter().find(|x| x["value"] == "h|c3").expect("settled chat in jump");
    assert_eq!((j_c3["place"].as_str(), j_c3["chat"].as_bool()), (Some("Chats"), Some(true)));
    // when no unsettled/selected chat remains, section is omitted
    let s_settled = st(&[pc("p1", "Beta", "/nowhere/beta", NOW - 200), tc("c3", "chats", NOW - 80), json!({"$":"ThreadSettled","id":"c3","manual":true,"at":NOW - 70})]);
    let rows_settled = merged_projects(&s_settled, &[], &|_| false, &|_| String::new(), "desk", &[]);
    assert!(sec(&build(&s_settled, &rows_settled, &Value::Null, &ctx(&none)), "chats").is_null());
}

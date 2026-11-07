//! The thread's top panels as the native client draws them (client `Todos.*`, subs `Subs.*`; build's phones take the same rows through `Mob.todos`
//! and `Mob.subs`): the agent's to-do list ("To-do 3/7") and its background work and subagents ("2 at work"). Pure over the model's settings: `todos.<t>` (the list),
//! `todos.visible.<t>` and `bg.<t>` (rows "id TAB doing TAB call TAB title TAB kind TAB tokens"; `doing` starting with the ended mark is a finished one).
//! Whether a panel is open is the client's own (kept per thread on the device); the rows are always sent.
use crate::agents::{bg, lockrec::{age, Lock}};
use crate::state::State;
use serde_json::{json, Value};

fn set<'a>(st: &'a State, k: String) -> &'a str { st.settings.get(&k).map(String::as_str).unwrap_or("") }

/// `{count: "3/7", head: "To-do 3/7", now: "the step in progress", rows: [{text, status}]}`, or null while the list is empty or hidden (a new turn of the agent hides a finished list: `todos.visible.<t>` is "0").
pub fn todos(st: &State, thread: &str) -> Value {
    let list: Value = serde_json::from_str(set(st, format!("todos.{thread}"))).unwrap_or(Value::Null);
    let items: Vec<&Value> = list.as_array().into_iter().flatten().collect();
    if items.is_empty() || set(st, format!("todos.visible.{thread}")) == "0" { return Value::Null }
    let status = |x: &Value| match x["status"].as_str().unwrap_or("") { "completed" => "completed", "in_progress" => "in_progress", _ => "pending" };
    let done = items.iter().filter(|x| status(x) == "completed").count();
    let now = items.iter().find(|x| status(x) == "in_progress").map(|x| { let f = x["form"].as_str().unwrap_or(""); if f.is_empty() { x["text"].as_str().unwrap_or("") } else { f } }).unwrap_or("");
    let count = format!("{}/{}", done, items.len());
    json!({"count": count, "head": format!("To-do {count}"), "now": now, "rows": items.iter().map(|x| json!({"id": x["id"].as_str().unwrap_or(""), "text": x["text"].as_str().unwrap_or(""), "status": status(x)})).collect::<Vec<_>>()})
}

/// A row of a thread's work the person may act on: `act` is what its button sends, with `value`; `label` the button's word.
fn acting(title: String, kind: &str, state: &str, doing: String, act: &str, value: &str, label: &str) -> Value { json!({"title": title, "kind": kind, "state": state, "doing": doing, "act": act, "value": value, "label": label}) }
/// Subs.watch.row: a job or a watch Backplane owns for the thread. Stop while it is at work and can still be stopped; a queued job's Stop takes it out of the queue.
fn watch_rows(st: &State, thread: &str) -> Vec<Value> {
    bg::lines(set(st, format!("watch.{thread}"))).iter().map(|l| {
        let ended = bg::ended(l); let doing = bg::doing(l);
        let word = if ended { doing.trim_start_matches(bg::ENDED).to_string() } else { doing };
        let stop = !ended && word != "cancelling" && word != "unknown";
        acting(format!("watch: {}", bg::work_title(l)), "watch", if ended { "ended" } else { "working" }, word, if stop { "work-cancel" } else { "" }, &bg::field(l, 0), if stop { "Stop" } else { "" })
    }).collect()
}
/// Subs.lock.row: a thread's own place on each lock, held (Release) or queued (Cancel). A job's place shows on its watch row.
fn lock_rows(st: &State, thread: &str) -> Vec<Value> {
    bg::lines(set(st, "locks.names".into())).iter().filter_map(|name| {
        let l = Lock::read(set(st, format!("lock.{name}"))); if !l.has(thread) { return None }
        let held = l.owner() == thread && l.ready;
        let doing = if held { "Release when done".to_string() } else if l.owner() == thread { "waiting for a flock outside Backplane".to_string() } else { format!("position {} · held by {}", l.position(thread), l.owner()) };
        Some(acting(format!("lock: {name}"), "lock", if held { "held" } else { "waiting" }, doing, if l.owner() == thread { "lock-release" } else { "lock-cancel" }, name, if l.owner() == thread { "Release" } else { "Cancel" }))
    }).collect()
}
/// `{busy: "2 at work" | "", word: "2 at work" | "3 done", rows: [{title, kind, state, doing, act?, value?, label?}]}`, or null with nothing to show. A row is a subagent, a background shell, a monitor or a workflow the agent left running, a thread's lock, or work Backplane watches for it.
pub fn subs(st: &State, thread: &str) -> Value {
    let text = set(st, format!("bg.{thread}"));
    let lines = bg::lines(text);
    let own: Vec<Value> = lock_rows(st, thread).into_iter().chain(watch_rows(st, thread)).collect();
    if lines.is_empty() && own.is_empty() { return Value::Null }
    let live = bg::running(&lines) + own.iter().filter(|r| r["state"] != "ended").count();
    let rows: Vec<Value> = lines.iter().map(|l| {
        let ended = bg::ended(l); let doing = bg::doing(l);
        let title = { let t = bg::field(l, 3); if t.is_empty() { bg::field(l, 0) } else { t } };
        let kind = { let k = bg::field(l, 4); if k.is_empty() { "subagent".to_string() } else { k } };
        let state = if !ended { "working".to_string() } else { doing.trim_start_matches(bg::ENDED).to_string() };
        json!({"title": title, "kind": kind, "state": state, "doing": if ended { String::new() } else { doing }})
    }).chain(own).collect();
    let word = if live > 0 { format!("{live} at work") } else { format!("{} done", rows.len()) };
    json!({"busy": if live > 0 { format!("{live} at work") } else { String::new() }, "word": word, "live": live, "rows": rows})
}
/// Plane.lock.row for the clients: every lock with a holder or a queue, its holder's title, how long held, who waits. Clients draw these as they come.
pub fn locks(st: &State, now: u64, title: impl Fn(&str) -> String) -> Vec<Value> {
    bg::lines(set(st, "locks.names".into())).iter().filter_map(|name| {
        let l = Lock::read(set(st, format!("lock.{name}"))); if l.idle() { return None }
        let h = l.holder.clone().unwrap_or_default();
        let who = if h.thread.is_empty() { h.note.clone() } else { title(&h.thread) };
        let held = if let Some(o) = &l.outside { format!("held outside Backplane by pid {}", o["pid"]) } else if l.ready { format!("{who} · {}", age(now, h.since)) } else { format!("{who} · starting") };
        Some(json!({"name": name, "holder": who, "thread": h.thread, "line": held, "waiting": l.waiters.len()}))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn st(kv: &[(&str, &str)]) -> State { let mut s = State::new(); for (k, v) in kv { s.settings.insert(k.to_string(), v.to_string()); } s }
    #[test] fn a_todo_list_counts_its_done_steps_names_the_one_in_progress_and_hides_while_empty_or_hidden() {
        let list = r#"[{"id":"1","text":"Survey","form":"Surveying","status":"completed"},{"id":"2","text":"Route","form":"Routing","status":"in_progress"},{"id":"3","text":"DRC","form":"","status":"weird"}]"#;
        let v = todos(&st(&[("todos.t1", list), ("todos.visible.t1", "1")]), "t1");
        assert_eq!((v["count"].as_str(), v["head"].as_str(), v["now"].as_str()), (Some("1/3"), Some("To-do 1/3"), Some("Routing")));
        assert_eq!(v["rows"][2]["status"], "pending", "an unknown status is pending");
        assert!(todos(&st(&[("todos.t1", list), ("todos.visible.t1", "0")]), "t1").is_null());
        assert!(todos(&st(&[]), "t1").is_null() && todos(&st(&[("todos.t1", "[]")]), "t1").is_null());
    }
    #[test] fn background_work_counts_who_is_at_work_and_says_how_the_rest_ended() {
        let rows = format!("a1\tReading footprint 3\tc1\tSurvey the footprints\tsubagent\n{}\tc2\tnpm run watch\tshell", bg::line("b1", &format!("{}completed", bg::ENDED)));
        let v = subs(&st(&[("bg.t1", &rows)]), "t1");
        assert_eq!((v["busy"].as_str(), v["live"].as_u64()), (Some("1 at work"), Some(1)));
        assert_eq!((v["rows"][0]["title"].as_str(), v["rows"][0]["state"].as_str(), v["rows"][0]["doing"].as_str()), (Some("Survey the footprints"), Some("working"), Some("Reading footprint 3")));
        assert_eq!((v["rows"][1]["state"].as_str(), v["rows"][1]["kind"].as_str()), (Some("completed"), Some("shell")));
        let done = subs(&st(&[("bg.t1", &bg::line("b1", &format!("{}completed", bg::ENDED)))]), "t1"); assert_eq!((done["busy"].as_str(), done["word"].as_str()), (Some(""), Some("1 done")));
        assert!(subs(&st(&[]), "t1").is_null());
    }
    #[test] fn a_threads_locks_and_jobs_carry_what_their_buttons_send() {
        let held = r#"{"holder":{"id":"t1","kind":"thread","thread":"t1","note":"","since":1,"lease":0},"waiters":[{"id":"t2","kind":"thread","thread":"t2","note":"","since":2,"lease":0}],"ready":true}"#;
        let flock = r#"{"holder":{"id":"t1","kind":"thread","thread":"t1","note":"","since":1,"lease":0},"waiters":[],"ready":false,"outside":{"pid":7,"cmd":"flock x","since":1}}"#;
        let job = "w-1\twaiting for lock b (2nd)\t\tthe build\twatch\trun\t\\emake\t-\t0";
        let s1 = st(&[("locks.names", "a\nb"), ("lock.a", held), ("lock.b", flock), ("watch.t1", job)]);
        let v = subs(&s1, "t1"); let rows = v["rows"].as_array().unwrap();
        assert_eq!((rows[0]["act"].as_str(), rows[0]["value"].as_str(), rows[0]["label"].as_str(), rows[0]["state"].as_str()), (Some("lock-release"), Some("a"), Some("Release"), Some("held")));
        assert_eq!((rows[1]["act"].as_str(), rows[1]["state"].as_str()), (Some("lock-release"), Some("waiting")), "a holder held back by a flock can still let go");
        assert_eq!((rows[2]["act"].as_str(), rows[2]["value"].as_str(), rows[2]["label"].as_str(), rows[2]["doing"].as_str()), (Some("work-cancel"), Some("w-1"), Some("Stop"), Some("waiting for lock b (2nd)")));
        assert_eq!(v["live"], 3);
        let w = subs(&s1, "t2"); assert_eq!((w["rows"][0]["act"].as_str(), w["rows"][0]["label"].as_str(), w["rows"][0]["doing"].as_str()), (Some("lock-cancel"), Some("Cancel"), Some("position 1 · held by t1")));
        assert!(subs(&s1, "t3").is_null(), "another thread's lock is not its row");
        let ls = locks(&s1, 301, |t| format!("title of {t}"));
        assert_eq!((ls[0]["holder"].as_str(), ls[0]["line"].as_str(), ls[0]["waiting"].as_u64()), (Some("title of t1"), Some("title of t1 · 5m"), Some(1)));
        assert_eq!(ls[1]["line"], "held outside Backplane by pid 7");
    }
}

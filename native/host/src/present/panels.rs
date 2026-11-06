//! The thread's top panels as the native client draws them (client.bend `Todos.*`, subs.bend `Subs.*`; the Bend build's phones take the same rows through `Mob.todos`
//! and `Mob.subs`): the agent's to-do list ("To-do 3/7") and its background work and subagents ("2 at work"). Pure over the model's settings: `todos.<t>` (the list),
//! `todos.visible.<t>` and `bg.<t>` (rows "id TAB doing TAB call TAB title TAB kind TAB tokens"; `doing` starting with the ended mark is a finished one).
//! Whether a panel is open is the client's own (kept per thread on the device); the rows are always sent.
use crate::agents::bg;
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
    json!({"count": count, "head": format!("To-do {count}"), "now": now, "rows": items.iter().map(|x| json!({"text": x["text"].as_str().unwrap_or(""), "status": status(x)})).collect::<Vec<_>>()})
}

/// `{busy: "2 at work" | "", word: "2 at work" | "3 done", rows: [{title, kind, state, doing}]}`, or null with nothing to show. A row is a subagent, a background shell, a monitor or a workflow the agent left running.
pub fn subs(st: &State, thread: &str) -> Value {
    let text = set(st, format!("bg.{thread}"));
    let lines = bg::lines(text);
    if lines.is_empty() { return Value::Null }
    let live = bg::running(&lines);
    let rows: Vec<Value> = lines.iter().map(|l| {
        let ended = bg::ended(l); let doing = bg::doing(l);
        let title = { let t = bg::field(l, 3); if t.is_empty() { bg::field(l, 0) } else { t } };
        let kind = { let k = bg::field(l, 4); if k.is_empty() { "subagent".to_string() } else { k } };
        let state = if !ended { "working".to_string() } else { doing.trim_start_matches(bg::ENDED).to_string() };
        json!({"title": title, "kind": kind, "state": state, "doing": if ended { String::new() } else { doing }})
    }).collect();
    let word = if live > 0 { format!("{live} at work") } else { format!("{} done", lines.len()) };
    json!({"busy": if live > 0 { format!("{live} at work") } else { String::new() }, "word": word, "live": live, "rows": rows})
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
}

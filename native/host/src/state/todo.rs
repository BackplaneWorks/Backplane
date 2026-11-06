//! TodosChanged beyond snapshots (src/core/model.bend Todos.changed; src/core/todo.bend): the agent's task tools, the person's
//! edits, and a whole agent list, merged by identity into the settings todos.<t>, todos.base.<t>, todos.visible.<t>.
use super::json::*;
use super::State;
use serde_json::{json, Value};

#[derive(Clone, PartialEq, Debug)]
pub struct Todo { id: String, text: String, form: String, status: String }
fn status_of(s: &str) -> String { if s == "completed" || s == "in_progress" { s.into() } else { "pending".into() } }
fn or(v: &str, d: &str) -> String { if v.is_empty() { d.into() } else { v.into() } }

pub fn read(text: &str) -> Vec<Todo> {
    elems(&parse_or_null(text)).iter().map(|x| Todo { id: gstr(x, "id", "").into(), text: gstr(x, "text", "").into(), form: gstr(x, "form", "").into(), status: status_of(gstr(x, "status", "")) }).collect()
}
pub fn show_list(ts: &[Todo]) -> String {
    show(&Value::Array(ts.iter().map(|t| json!({"id": t.id, "text": t.text, "form": t.form, "status": t.status})).collect()))
}
fn find<'a>(ts: &'a [Todo], id: &str) -> Option<&'a Todo> { ts.iter().find(|t| t.id == id) }

fn write(input: &Value) -> Vec<Todo> {
    elems(gobj(input, "todos")).iter().enumerate().map(|(i, x)| Todo { id: (i + 1).to_string(), text: gstr(x, "content", "").into(), form: gstr(x, "activeForm", "").into(), status: status_of(gstr(x, "status", "")) }).collect()
}
fn create(mut ts: Vec<Todo>, use_: &str, input: &Value) -> Vec<Todo> {
    let text = gstr(input, "subject", gstr(input, "description", ""));
    ts.push(Todo { id: format!("u:{use_}"), text: text.into(), form: gstr(input, "activeForm", "").into(), status: "pending".into() }); ts
}
fn named(ts: Vec<Todo>, use_: &str, n: &str) -> Vec<Todo> {
    ts.into_iter().map(|mut t| { if t.id == format!("u:{use_}") { t.id = n.into(); } t }).collect()
}
/// TaskUpdate: every step with the id changes, or goes when the status is "deleted"
fn update(ts: Vec<Todo>, input: &Value) -> Vec<Todo> {
    let (id, gone) = (gstr(input, "taskId", ""), gstr(input, "status", "") == "deleted");
    ts.into_iter().filter_map(|t| {
        if t.id != id { return Some(t) }
        if gone { return None }
        let st = gstr(input, "status", "");
        Some(Todo { id: t.id.clone(), text: or(gstr(input, "subject", ""), &t.text), form: or(gstr(input, "activeForm", ""), &t.form), status: if st.is_empty() { t.status.clone() } else { status_of(st) } })
    }).collect()
}
fn field(old: &str, human: &str, agent: &str) -> String { if old == agent { human.into() } else { agent.into() } }
/// Todo.merge: an identity-based three-way merge; a repeated snapshot does not undo the person's edits
fn merge(base: &[Todo], current: &[Todo], next: &[Todo]) -> Vec<Todo> {
    let mut out = vec![];
    for h in current {
        match find(next, &h.id) {
            Some(a) => out.push(match find(base, &h.id) { Some(o) => Todo { id: a.id.clone(), text: field(&o.text, &h.text, &a.text), form: field(&o.form, &h.form, &a.form), status: field(&o.status, &h.status, &a.status) }, None => a.clone() }),
            None => if find(base, &h.id).is_none() { out.push(h.clone()) },
        }
    }
    for h in next {
        if find(current, &h.id).is_some() { continue }
        match find(base, &h.id) { Some(b) if b == h => {}, _ => out.push(h.clone()) }
    }
    out
}
/// TaskUpdate names fields explicitly: apply them even after a person's differing edit
fn explicit(ts: Vec<Todo>, base: &[Todo], input: &Value) -> Vec<Todo> {
    let id = gstr(input, "taskId", "");
    if find(&ts, id).is_some() { return update(ts, input) }
    let mut ts = ts; if let Some(b) = find(base, id) { ts.push(b.clone()); } update(ts, input)
}
fn insert(ts: Vec<Todo>, n: u64, h: Todo) -> Vec<Todo> { let mut ts = ts; let at = (n as usize).min(ts.len()); ts.insert(at, h); ts }
/// Todo.edit: the person's add, delete, move and set
fn edit(ts: Vec<Todo>, p: &Value) -> Vec<Todo> {
    let id = gstr(p, "id", "");
    match gstr(p, "op", "") {
        "add" => { let mut ts = ts; ts.push(Todo { id: id.into(), text: gstr(p, "text", "").into(), form: String::new(), status: "pending".into() }); ts }
        "delete" => ts.into_iter().filter(|t| t.id != id).collect(),
        "move" => match find(&ts, id).cloned() { None => ts, Some(h) => insert(ts.into_iter().filter(|t| t.id != id).collect(), gnat(p, "to", 0), h) },
        "set" => update(ts, &json!({"taskId": id, "subject": gstr(p, "text", ""), "status": gstr(p, "status", "")})),
        _ => ts,
    }
}

struct Plan { base: Vec<Todo>, current: Vec<Todo>, fresh: bool }
fn replace(base: &[Todo], current: &[Todo], next: Vec<Todo>) -> Plan { Plan { current: merge(base, current, &next), base: next, fresh: true } }
fn step(name: &str, b: &Value, p: Plan) -> Plan {
    let input = gobj(b, "input");
    match name {
        "TodoWrite" => replace(&p.base, &p.current, write(input)),
        "TaskCreate" => { let use_ = gstr(b, "id", ""); let next = if p.fresh { create(p.base.clone(), use_, input) } else { create(vec![], use_, input) }; replace(&p.base, &p.current, next) }
        "TaskUpdate" => Plan { current: explicit(p.current, &p.base, input), base: update(p.base, input), fresh: true },
        "TaskNamed" => { let (u, n) = (gstr(b, "use", ""), gstr(b, "number", "")); Plan { base: named(p.base, u, n), current: named(p.current, u, n), fresh: p.fresh } }
        _ => p,
    }
}

impl State {
    fn todos_of(&self, th: &str) -> Vec<Todo> { read(&self.get(&format!("todos.{th}"), "")) }
    fn todos_base(&self, th: &str) -> Vec<Todo> { let cur = self.get(&format!("todos.{th}"), ""); read(&self.get(&format!("todos.base.{th}"), &cur)) }
    pub(super) fn todos_changed(&mut self, th: &str, who: &str, data: &str) {
        match who {
            "tools" => {
                let mut p = Plan { base: self.todos_base(th), current: self.todos_of(th), fresh: self.get(&format!("todos.visible.{th}"), "1") != "0" };
                for b in elems(&parse_or_null(data)) { p = step(gstr(b, "name", ""), b, p); }
                self.todos_put(th, &show_list(&p.current), &show_list(&p.base), if p.fresh { "1" } else { "0" });
            }
            "person" => {
                let cur = edit(self.todos_of(th), &parse_or_null(data));
                self.todos_put(th, &show_list(&cur), &show_list(&self.todos_base(th)), "1");
            }
            "snapshot" => {}
            _ => {
                let m = merge(&self.todos_base(th), &self.todos_of(th), &read(data));
                self.todos_put(th, &show_list(&m), data, "1");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fold(lines: &[String]) -> State { let mut s = State::new(); for l in lines { s.apply_line(l); } s }
    fn tc(who: &str, data: &str) -> String { serde_json::to_string(&json!({"$":"TodosChanged","thread":"t","who":who,"data":data,"at":1})).unwrap() }
    #[test] fn todos_tools_create_then_name_then_update() {
        // rust todos_tools_create_then_name_then_update
        let blocks = r#"[{"name":"TaskCreate","id":"c1","input":{"subject":"a"}},{"name":"TaskNamed","use":"c1","number":"3"},{"name":"TaskUpdate","input":{"taskId":"3","status":"in_progress"}}]"#;
        let s = fold(&[tc("tools", blocks)]);
        assert_eq!(s.settings["todos.t"], r#"[{"id":"3","text":"a","form":"","status":"in_progress"}]"#);
        assert_eq!(s.settings["todos.visible.t"], "1");
    }
    #[test] fn todos_person_edits_survive_a_repeated_agent_snapshot() {
        // rust todos_person_edits_survive_a_repeated_agent_snapshot
        let list = r#"[{"id":"1","text":"x","form":"","status":"pending"}]"#;
        let s = fold(&[tc("agent", list), tc("person", r#"{"op":"set","id":"1","text":"mine","status":""}"#), tc("agent", list)]);
        assert!(s.settings["todos.t"].contains("\"mine\""));
    }
}

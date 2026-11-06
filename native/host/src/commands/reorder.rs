//! The sidebar's drag and drop (client.bend `Ui.reorder`, `Ui.reorder_project`): the action `reorder` (a thread dropped where another stands on its shelf) and
//! `reorder-project` (a project dropped on a project, or on a thread standing for its project), value `<moved>\u{1f}<target>` in raw ids. The shelf's ids in
//! their order come from the model (`order_of`); the moved one is taken out and put where the target stood (M.Order.move), and the request goes out as
//! `thread.reorder {section, thread, ids}` or `project.reorder {project, ids}`. A drop that changes nothing (onto itself, or across shelves) sends nothing.
use super::actions::ActOut;
use serde_json::{json, Value};

/// M.Order.move: `id` leaves the list and lands at the index the target had.
pub fn moved(ids: &[String], id: &str, target: &str) -> Option<Vec<String>> {
    let from = ids.iter().position(|x| x == id)?; let to = ids.iter().position(|x| x == target)?;
    if from == to { return None }
    let mut out: Vec<String> = ids.to_vec(); let item = out.remove(from); out.insert(to, item); Some(out)
}

pub fn act(hub: &str, session: &Value, action: &str, value: &str, order_of: &dyn Fn(&str, &str) -> Option<(String, Vec<String>)>) -> Option<ActOut> {
    if action != "reorder" && action != "reorder-project" { return None }
    let none = || Some(ActOut { session_patch: json!({"drafts": {}}), cmds: vec![], next_target: None });
    let Some((id, target)) = value.split_once('\u{1f}') else { return none() };
    let kind = if action == "reorder" { "thread" } else { "project" };
    let Some((section, ids)) = order_of(kind, id) else { return none() };
    let Some(new) = moved(&ids, id, target) else { return none() };
    let rid = session["rid"].as_u64().unwrap_or(1);
    let (m, p) = if kind == "thread" { ("thread.reorder", json!({"section": section, "thread": id, "ids": new})) } else { ("project.reorder", json!({"project": id, "ids": new})) };
    Some(ActOut { session_patch: json!({"rid": rid + 1, "drafts": {}}), cmds: vec![json!({"type": "send", "hub": hub, "text": json!({"id": rid, "m": m, "p": p}).to_string()})], next_target: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(xs: &[&str]) -> Vec<String> { xs.iter().map(|s| s.to_string()).collect() }
    #[test] fn the_moved_one_lands_where_the_target_stood() {
        assert_eq!(moved(&v(&["a", "b", "c"]), "a", "c").unwrap(), v(&["b", "c", "a"]));
        assert_eq!(moved(&v(&["a", "b", "c"]), "c", "a").unwrap(), v(&["c", "a", "b"]));
        assert!(moved(&v(&["a", "b"]), "a", "a").is_none() && moved(&v(&["a", "b"]), "a", "z").is_none() && moved(&v(&["a", "b"]), "z", "a").is_none());
    }
    #[test] fn a_drop_sends_the_new_order_and_a_useless_one_sends_nothing() {
        let of = |k: &str, _: &str| Some((if k == "thread" { "active".to_string() } else { String::new() }, v(&["t1", "t2", "t3"])));
        let o = act("h", &json!({"rid": 4}), "reorder", "t3\u{1f}t1", &of).unwrap();
        assert_eq!(o.session_patch["rid"], 5); let text: Value = serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap();
        assert_eq!((text["m"].as_str(), text["p"]["section"].as_str(), text["p"]["thread"].as_str()), (Some("thread.reorder"), Some("active"), Some("t3"))); assert_eq!(text["p"]["ids"], json!(["t3", "t1", "t2"]));
        let o = act("h", &json!({"rid": 4}), "reorder-project", "t2\u{1f}t3", &of).unwrap(); assert!(o.cmds[0]["text"].as_str().unwrap().contains("project.reorder"));
        assert!(act("h", &json!({}), "reorder", "t1\u{1f}t1", &of).unwrap().cmds.is_empty());
        assert!(act("h", &json!({}), "reorder", "t1", &of).unwrap().cmds.is_empty());
        assert!(act("h", &json!({}), "pin", "x", &of).is_none());
    }
}

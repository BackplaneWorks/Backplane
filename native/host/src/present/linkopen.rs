//! `link-open`: a `backplane://` link (or a bare id) clicked in a message, as the desktop client sends it. The hub decides what it opens and how the
//! sidebar gets there: a thread of another project (or a settled or archived one) is selected with its project unfolded and its shelf open; a project
//! opens at its newest thread; a room or a bot's chat opens as its row would. What does not open comes back as a short note. Pure over the state.
use crate::state::State;
use serde_json::{json, Value};

#[derive(Debug, PartialEq)]
pub enum Plan {
    /// A thread here: select it, with these sidebar flags (`open` keys) set.
    Select { id: String, open: Vec<(String, bool)> },
    /// A thread this hub mirrors from a linked machine.
    Far(String),
    Room(String),
    Bot(String),
    Note(&'static str),
}

/// `here` is this machine's name: a thread link may name a machine by its link id or its name (`crate::mirror::thread_ref`).
pub fn plan(st: &State, far: &[(String, &State)], here: &str, url: &str, now: u64) -> Plan {
    // the app-scheme aliases are the same link
    let alias = ["bpgpui://", "backplane-gpui://"].iter().find_map(|p| url.trim().strip_prefix(p)).map(|r| format!("backplane://{r}"));
    let url = alias.as_deref().unwrap_or(url).trim();
    if url.is_empty() || url.len() > 4096 { return Plan::Note("That link is empty.") }
    let parsed = if url.starts_with("backplane://") { crate::md::refs::parse(url) } else { crate::md::refs::bare(url) };
    let Some(r) = parsed else { return Plan::Note(if url.starts_with("backplane://") { "That link is not one this hub knows." } else { "That link does not open here." }) };
    let id = r.rest.first().cloned().unwrap_or_default();
    match r.kind.as_str() {
        "thread" => thread(st, far, here, &id, now),
        "project" => project(st, &id, now),
        "room" => room(st, &id),
        "bot" => bot(st, &id),
        // a conversation id: `room:<id>`, `bot:<id>`, `pair:bot:<id>|person` (the bot's own chat)
        "conv" => match id.split_once(':') {
            Some(("room", rest)) => room(st, rest),
            Some(("bot", rest)) => bot(st, rest),
            Some(("pair", rest)) => bot(st, rest.strip_prefix("bot:").unwrap_or(rest).split('|').next().unwrap_or("")),
            _ => Plan::Note("That conversation has no screen here."),
        },
        _ => Plan::Note("This window has no screen for that link."),
    }
}

fn thread(st: &State, far: &[(String, &State)], here: &str, id: &str, now: u64) -> Plan {
    let Some(id) = crate::mirror::thread_ref(id, here, &|t| st.threads.get(t).map_or(false, |x| !x.deleted), far) else {
        return Plan::Note(if id.contains('~') { "That thread is on a machine this hub does not mirror (yet)." } else { "That thread is not on this hub." })
    };
    if id.contains('~') { return Plan::Far(id) }
    let id = id.as_str();
    let Some(t) = st.threads.get(id).filter(|t| !t.deleted) else { return Plan::Note("That thread is not on this hub.") };
    let mut open = vec![(format!("project-closed:{}", t.project), false)];
    let until = st.snooze_until.get(id).copied().unwrap_or(0);
    match crate::shelf::shelf_at(t, until, now) {
        crate::shelf::Shelf::Settled => open.push((t.project.clone(), true)),
        crate::shelf::Shelf::Hidden if t.archived => open.push((format!("arch:{}", t.project), true)),
        _ => {}
    }
    Plan::Select { id: id.to_string(), open }
}

fn project(st: &State, id: &str, _now: u64) -> Plan {
    if !st.projects.get(id).map_or(false, |p| !p.removed) { return Plan::Note(if id.contains('~') { "That project is on another machine." } else { "That project is not on this hub." }) }
    let top = st.threads.iter().filter(|(_, t)| t.project == id && !t.archived && !t.deleted).max_by_key(|(i, t)| (t.pinned, t.activity, i.to_string())).map(|(i, _)| i.clone());
    match top { Some(t) => Plan::Select { id: t, open: vec![(format!("project-closed:{id}"), false)] }, None => Plan::Note("That project has no threads yet.") }
}
fn room(st: &State, id: &str) -> Plan { let id = id.strip_prefix("room:").unwrap_or(id); if !id.starts_with("dm:") && st.rooms_nav.contains_key(id) { Plan::Room(id.to_string()) } else { Plan::Note("That room is not on this hub.") } }
fn bot(st: &State, id: &str) -> Plan { let id = id.strip_prefix("bot:").unwrap_or(id); if st.bots_nav.contains_key(id) { Plan::Bot(id.to_string()) } else { Plan::Note("That conversation is not on this hub.") } }

/// The request `link-open` becomes: `link-far` (go to this thread, the sidebar flags already in the session), a room's or a bot's row action, or a note.
pub fn request(st: &State, far: &[(String, &State)], here: &str, p: &Value) -> Value {
        let mut c = p.clone();
    if !c["session"].is_object() { c["session"] = json!({}); }
    if !c["session"]["open"].is_object() { c["session"]["open"] = json!({}); }
    let (action, value): (&str, String) = match plan(st, far, here, p["value"].as_str().unwrap_or(""), p["now"].as_u64().unwrap_or(0) / 1000) {
        Plan::Select { id, open } => { for (k, v) in open { c["session"]["open"][k] = json!(v); } ("link-far", id) }
        Plan::Far(id) => ("link-far", id),
        Plan::Room(id) => ("room", id),
        Plan::Bot(id) => ("bot", id),
        Plan::Note(n) => ("link-note", n.to_string()),
    };
    c["action"] = json!(action); c["value"] = json!(value);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    fn st() -> State {
        let mut st = State::new(); st.lean = true;
        for e in [json!({"$":"ProjectCreated","id":"pa","title":"A","root":"/a","at":1}), json!({"$":"ProjectCreated","id":"pb","title":"B","root":"/b","at":2}), json!({"$":"ProjectCreated","id":"pc","title":"C","root":"/c","at":3}),
                  json!({"$":"ThreadCreated","id":"t1700000001-1","project":"pa","title":"One","at":5}), json!({"$":"ThreadCreated","id":"t1700000002-2","project":"pb","title":"Two","at":6}), json!({"$":"ThreadSettled","id":"t1700000002-2","at":7})] { assert!(st.apply(&e), "{e}"); }
        st
    }
    #[test] fn a_thread_of_another_project_selects_with_its_project_unfolded() {
        let s = st();
        assert_eq!(plan(&s, &[], "", "backplane://thread/t1700000001-1", 100), Plan::Select { id: "t1700000001-1".into(), open: vec![("project-closed:pa".into(), false)] });
        assert_eq!(plan(&s, &[], "", "t1700000001-1", 100), plan(&s, &[], "", "backplane://thread/t1700000001-1", 100), "a bare id is the same link");
    }
    #[test] fn a_settled_thread_opens_its_shelf_too() {
        let s = st();
        let Plan::Select { open, .. } = plan(&s, &[], "", "backplane://thread/t1700000002-2", 100) else { panic!() };
        assert!(open.contains(&("pb".to_string(), true)), "{open:?}");
    }
    #[test] fn projects_open_at_their_newest_thread_and_empty_ones_say_so() {
        let s = st();
        assert_eq!(plan(&s, &[], "", "backplane://project/pa", 100), Plan::Select { id: "t1700000001-1".into(), open: vec![("project-closed:pa".into(), false)] });
        assert_eq!(plan(&s, &[], "", "backplane://project/pc", 100), Plan::Note("That project has no threads yet."));
        assert_eq!(plan(&s, &[], "", "backplane://project/nope", 100), Plan::Note("That project is not on this hub."));
    }
    #[test] fn unknown_things_are_notes() {
        let s = st();
        assert!(matches!(plan(&s, &[], "", "backplane://thread/t1700000099-9", 1), Plan::Note(_)));
        assert!(matches!(plan(&s, &[], "", "backplane://thread/lab~t9", 1), Plan::Note(_)));
        assert!(matches!(plan(&s, &[], "", "backplane://conv/room%3Anone", 1), Plan::Note(_)));
        assert!(matches!(plan(&s, &[], "", "backplane://nonsense/x", 1), Plan::Note(_)));
        assert!(matches!(plan(&s, &[], "", "", 1), Plan::Note(_)));
    }
    #[test] fn the_request_carries_the_flags_and_the_row_action() {
        let s = st();
        let c = request(&s, &[], "", &json!({"hub":"h","value":"backplane://thread/t1700000002-2","now":100000,"session":{}}));
        assert_eq!((c["action"].as_str(), c["value"].as_str()), (Some("link-far"), Some("t1700000002-2")));
        assert_eq!(c["session"]["open"]["pb"], json!(true));
        let n = request(&s, &[], "", &json!({"hub":"h","value":"backplane://thread/t1700000098-8"}));
        assert_eq!(n["action"], json!("link-note"));
    }
}

//! The action dispatcher the Rust presenter calls for a `view.action`: composer (draft, send), then the commands lane's thread/project actions, then `select`, each a pure function
//! that is None for what it does not own or has not ported; the first answer wins, its patch is merged into a copy of the request's session (`patch::merge`), its commands are
//! returned in order, and the shared post-hook of Bend's `Ui.act` that is mine, `Fab.sync` (a `send fab.look` after any action with a thread selected whose place changed), is
//! appended for the answers that did not already include it (select does). None from here means the helper, or "not ported" on a hub without one.
use super::{composer, patch, select};
use crate::commands::actions as cmdact;
use serde_json::{json, Value};

/// What the three action families ask of the model: the presenter's metadata implements it (`StateFacts` below over `state::State`).
pub trait Facts: composer::Facts + select::Facts {
    /// a thread's title as the sidebar shows it (the field a rename starts from)
    fn title_of(&self, id: &str) -> String;
}

pub struct Dispatched { pub session: Value, pub cmds: Vec<Value>, pub next_target: Option<Value> }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

/// Fab.sync: the look the actor asks for when where the selected thread stands changed since the last ask (`@fab.at`), after the action's own commands, with the next `rid`.
fn fab_tail<F: Facts>(f: &F, hub: &str, session: &mut Value, cmds: &mut Vec<Value>) {
    let sel = s(session, "sel").to_string();
    if sel.is_empty() || sel.contains('~') || sel.starts_with("project-home-") || sel.starts_with('@') || !select::Facts::thread_exists(f, &sel) { return }
    let at = select::fab_at(f, &sel);
    let d = &session["drafts"];
    let want = at != s(d, "@fab.at") && !at.ends_with('#') && (!at.ends_with("#running") || s(&session["info"], &format!("fab.line:{sel}")).is_empty());
    if !want { return }
    let rid = session["rid"].as_u64().unwrap_or(0);
    cmds.push(json!({"type": "send", "hub": hub, "text": format!("{{\"id\":{rid},\"m\":\"fab.look\",\"p\":{{\"thread\":{}}}}}", serde_json::to_string(&sel).unwrap_or_default())}));
    patch::merge(session, &json!({"rid": rid + 1, "drafts": {"@fab.at": at}}));
}

/// The thread screen's buttons that are one request (Ui.act): `answer` (Ui.answer: `ask.answer {id, answer}` from "id|answer…", a value with no id sends no params) and `interrupt`
/// (Ui.on_thread "turn.interrupt"; a thread bound to an account is left alone, Accounts.rpc.params would add the binding); `copy` is the client's own (no request, no patch).
fn thread_button<F: Facts>(f: &F, hub: &str, session: &Value, sel: &str, action: &str, value: &str) -> Option<composer::ActOut> {
    let rid = session["rid"].as_u64().unwrap_or(1);
    let (m, p) = match action {
        "answer" => {
            let mut parts = value.split('|');
            let id = parts.next().unwrap_or("");
            let p = if value.is_empty() { json!({}) } else { json!({"id": id, "answer": parts.collect::<Vec<_>>().join("|")}) };
            ("ask.answer", p)
        }
        "interrupt" => {
            if sel.is_empty() || composer::Facts::account_params(f, sel) { return None }
            ("turn.interrupt", json!({"thread": sel}))
        }
        "copy" => return Some(composer::ActOut { session_patch: json!({}), cmds: vec![] }),
        _ => return None,
    };
    let text = json!({"id": rid, "m": m, "p": p}).to_string();
    Some(composer::ActOut { session_patch: json!({"rid": rid + 1}), cmds: vec![json!({"type": "send", "hub": hub, "text": text})] })
}

/// `params` is the object the hub builds for a view request (`session`, `action`, `value`, `cid`, `hub`, `now` in ms, `target`).
pub fn dispatch<F: Facts>(f: &F, params: &Value) -> Option<Dispatched> {
    let (hub, action, value) = (s(params, "hub"), s(params, "action"), s(params, "value"));
    if action.is_empty() { return None }
    let mut session = params["session"].clone();
    // a viewer panel open: the actor's Dv/Mpage/Rp syncs would add commands: not ported
    if session["panel"].as_bool().unwrap_or(false) { return None }
    let sel = if params["target"]["kind"] == "thread" { s(&params["target"], "id") } else { "" };
    let (cmds, next_target, has_fab) = if let Some(o) = composer::act(params, f) {
        patch::merge(&mut session, &o.session_patch); (o.cmds, None, false)
    } else if let Some(o) = cmdact::act(&cmdact::Req { hub, session: &params["session"], sel, action, value, title_of: &|t| f.title_of(t) }) {
        patch::merge(&mut session, &o.session_patch); (o.cmds, o.next_target, false)
    } else if let Some(o) = thread_button(f, hub, &params["session"], sel, action, value) {
        patch::merge(&mut session, &o.session_patch); (o.cmds, None, false)
    } else if action == "select" {
        let o = select::select(f, hub, &params["session"], value)?;
        patch::merge(&mut session, &o.session_patch); (o.cmds, Some(o.next_target), true)
    } else { return None };
    let mut cmds = cmds;
    if !has_fab { fab_tail(f, hub, &mut session, &mut cmds) }
    Some(Dispatched { session, cmds, next_target })
}

/// The facts over the presenter's `state::State`. The lean state keeps only the settings the policies read (state/policy.rs `lean_keeps`): `order.enabled`, `order.estimates` and
/// `account.thread.<t>.<provider>` are not among them, so over a lean state they read as unset (Ordering on, estimates on, no account binding): state is asked to keep those prefixes.
pub struct StateFacts<'a>(pub &'a crate::state::State);
impl composer::Facts for StateFacts<'_> {
    fn bot_thread(&self, thread: &str) -> bool { self.0.bot_threads_has(thread) }
    fn account_params(&self, thread: &str) -> bool {
        let Some(t) = self.0.threads.get(thread) else { return false };
        match self.0.settings.get(&format!("account.thread.{thread}.{}", t.provider)) {
            None => false,
            Some(raw) if raw.is_empty() => false,
            // Accounts.named: the binding's id is not empty (an unparsable binding reads as "!invalid", also not empty)
            Some(raw) => serde_json::from_str::<Value>(raw).ok().map_or(true, |j| !j["id"].as_str().unwrap_or("").is_empty()),
        }
    }
}
impl select::Facts for StateFacts<'_> {
    fn thread_exists(&self, id: &str) -> bool { self.0.threads.get(id).map_or(false, |t| !t.deleted) }
    fn turn_name(&self, id: &str) -> String { self.0.threads.get(id).map_or(String::new(), |t| if t.project.is_empty() { String::new() } else { t.turn.word().to_string() }) }
    fn order_enabled(&self) -> bool { self.0.settings.get("order.enabled").map_or(true, |v| v != "off") }
    fn estimates_on(&self) -> bool { self.0.settings.get("order.estimates").map_or(true, |v| v != "off") }
}
impl Facts for StateFacts<'_> {
    fn title_of(&self, id: &str) -> String { self.0.threads.get(id).map_or(String::new(), |t| t.title.clone()) }
}

#[cfg(test)]
mod answer_tests {
    use super::*;
    #[test] fn an_answer_action_becomes_ask_answer_with_the_next_rid() {
        // the shape Ui.answer makes: askid|answer, the answer may hold '|'
        let rid = 7u64; let (id, ans) = "q1-9|yes|always".split_once('|').unwrap();
        let text = format!("{{\"id\":{rid},\"m\":\"ask.answer\",\"p\":{{\"id\":{},\"answer\":{}}}}}", serde_json::to_string(id).unwrap(), serde_json::to_string(ans).unwrap());
        assert_eq!(text, r#"{"id":7,"m":"ask.answer","p":{"id":"q1-9","answer":"yes|always"}}"#);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct M { exists: &'static str, running: bool }
    impl composer::Facts for M { fn bot_thread(&self, _: &str) -> bool { false } fn account_params(&self, _: &str) -> bool { false } }
    impl select::Facts for M {
        fn thread_exists(&self, id: &str) -> bool { id == self.exists }
        fn turn_name(&self, _: &str) -> String { if self.running { "running".into() } else { "idle".into() } }
        fn order_enabled(&self) -> bool { true }
        fn estimates_on(&self) -> bool { true }
    }
    impl Facts for M { fn title_of(&self, _: &str) -> String { "T".into() } }
    const T: &str = "t1700000002-3";
    fn params(action: &str, value: &str, session: Value) -> Value {
        json!({"hub": "http://127.0.0.1:3820", "cid": "parity-gpui", "now": 1793864523000u64, "target": {"kind": "thread", "id": T}, "action": action, "value": value, "session": session})
    }
    fn session() -> Value { json!({"sel": T, "rid": 2, "panel": false, "drafts": {T: "hello parity", "@boot": "", "@fab.at": "onon#t1700000002-3#idle"}, "info": {}}) }
    #[test] fn a_send_is_the_composers_and_merges_its_patch() {
        let d = dispatch(&M { exists: T, running: false }, &params("send", "", session())).unwrap();
        assert_eq!(d.session["rid"], json!(3)); assert_eq!(d.session["drafts"][T], json!(""));
        assert_eq!(d.cmds.len(), 3, "keep, send, scroll: the place did not change, no look");
        assert!(d.next_target.is_none());
    }
    #[test] fn select_is_mine_and_carries_its_own_look() {
        let mut s = session(); s["sel"] = json!(""); s["rid"] = json!(1); s["drafts"].as_object_mut().unwrap().remove("@fab.at");
        let d = dispatch(&M { exists: T, running: false }, &params("select", &format!("http://127.0.0.1:3820|{T}"), s)).unwrap();
        assert_eq!(d.session["sel"], json!(T)); assert_eq!(d.session["rid"], json!(2));
        assert_eq!(d.cmds.len(), 3); assert_eq!(d.next_target, Some(json!({"kind": "thread", "id": T})));
    }
    #[test] fn a_thread_action_is_the_commands_lanes_and_gets_the_look_tail() {
        // pin acts on the selected thread; the session never asked for a look at this place: the tail appends it after the action's own commands
        let mut s = session(); s["drafts"].as_object_mut().unwrap().remove("@fab.at");
        let d = dispatch(&M { exists: T, running: false }, &params("row-pin", T, s)).unwrap();
        let last = d.cmds.last().unwrap();
        assert_eq!(last["type"], json!("send")); assert!(last["text"].as_str().unwrap().contains("\"m\":\"fab.look\""));
        assert_eq!(d.session["drafts"]["@fab.at"], json!("onon#t1700000002-3#idle"));
        assert_eq!(d.session["rid"].as_u64(), Some(2 + d.cmds.iter().filter(|c| c["type"] == "send").count() as u64), "one rid per send: the pin's and the look's");
    }
    #[test] fn answer_and_interrupt_are_one_numbered_request_each() {
        let f = M { exists: T, running: true };
        let d = dispatch(&f, &params("answer", "ask1|yes|really", session())).unwrap();
        assert_eq!(d.cmds[0]["text"], json!("{\"id\":2,\"m\":\"ask.answer\",\"p\":{\"id\":\"ask1\",\"answer\":\"yes|really\"}}"));
        assert_eq!(d.session["rid"], json!(3));
        let d = dispatch(&f, &params("interrupt", "", session())).unwrap();
        assert_eq!(d.cmds[0]["text"], json!("{\"id\":2,\"m\":\"turn.interrupt\",\"p\":{\"thread\":\"t1700000002-3\"}}"));
        let d = dispatch(&f, &params("copy", "x", session())).unwrap();
        assert!(d.cmds.is_empty() && d.session["rid"] == json!(2));
    }
    #[test] fn what_nobody_owns_is_none() {
        let f = M { exists: T, running: false };
        for a in ["", "skill", "fm-open", "settings"] { assert!(dispatch(&f, &params(a, "x", session())).is_none(), "{a}") }
        let mut open = session(); open["panel"] = json!(true);
        assert!(dispatch(&f, &params("send", "", open)).is_none(), "a viewer panel open: the syncs are not ported");
        assert!(dispatch(&f, &params("select", "h|unknown", session())).is_none());
    }
}

//! The action dispatcher the Rust presenter calls for a `view.action`: composer (draft, send), then the commands lane's thread/project actions, then `select`, each a pure function
//! that is None for what it does not own or has not ported; the first answer wins, its patch is merged into a copy of the request's session (`patch::merge`), its commands are
//! returned in order, and the shared post-hook of Bend's `Ui.act` that is mine, `Fab.sync` (a `send fab.look` after any action with a thread selected whose place changed), is
//! appended for the answers that did not already include it (select does). None from here means the helper, or "not ported" on a hub without one.
use super::{composer, patch, select, thread_actions};
use crate::commands::actions as cmdact;
use serde_json::{json, Value};

/// What the three action families ask of the model: the presenter's metadata implements it (`StateFacts` below over `state::State`).
pub trait Facts: composer::Facts + select::Facts {
    /// a thread's title as the sidebar shows it (the field a rename starts from)
    fn title_of(&self, id: &str) -> String;
    /// the folder a thread works in (Ui.root.thread: its worktree, else its project's root)
    fn root_of(&self, id: &str) -> String;
    /// AT.Att.max: the attachment limit in bytes (setting files.max.mb, default 512 MB, between 1 and 4000)
    fn attach_cap(&self) -> u64 { 512 << 20 }
}

pub struct Dispatched { pub session: Value, pub cmds: Vec<Value>, pub next_target: Option<Value> }

fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

/// Fab.sync: the look the actor asks for when where the selected thread stands changed since the last ask (`@fab.at`), after the action's own commands, with the next `rid`.
pub(crate) fn fab_tail<F: Facts>(f: &F, hub: &str, session: &mut Value, cmds: &mut Vec<Value>) {
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

/// Accounts.rpc.params for every request an action sends: one about a thread bound to a named account (`p.thread`) carries `accountBinding` first (Ui.rpc applies it to all of them).
fn bind_all<F: Facts>(f: &F, cmds: &mut [Value]) {
    for c in cmds.iter_mut() {
        if c["type"] != "send" { continue }
        let Some(mut req) = c["text"].as_str().and_then(|x| serde_json::from_str::<Value>(x).ok()) else { continue };
        let Some(th) = req["p"]["thread"].as_str().map(str::to_string) else { continue };
        let Some(b) = composer::Facts::account_binding(f, &th) else { continue };
        req["p"] = composer::bind(req["p"].take(), Some(&b));
        c["text"] = json!(req.to_string());
    }
}

/// `params` is the object the hub builds for a view request (`session`, `action`, `value`, `cid`, `hub`, `now` in ms, `target`).
pub fn dispatch<F: Facts>(f: &F, params: &Value) -> Option<Dispatched> {
    let (hub, action, value) = (s(params, "hub"), s(params, "action"), s(params, "value"));
    if action.is_empty() { return None }
    let mut session = params["session"].clone();
    let sel = if params["target"]["kind"] == "thread" { s(&params["target"], "id") } else { "" };
    let (cmds, next_target, has_fab) = if let Some(o) = composer::act(params, f) {
        patch::merge(&mut session, &o.session_patch); (o.cmds, None, false)
    } else if let Some(o) = cmdact::act(&cmdact::Req { hub, session: &params["session"], sel, action, value, title_of: &|t| f.title_of(t) }) {
        patch::merge(&mut session, &o.session_patch); (o.cmds, o.next_target, false)
    } else if let Some(o) = thread_actions::act(f, hub, &params["session"], sel, action, value) {
        patch::merge(&mut session, &o.session_patch); (o.cmds, None, !o.look)
    } else if action == "select" {
        let o = select::select(f, hub, &params["session"], value)?;
        patch::merge(&mut session, &o.session_patch); (o.cmds, Some(o.next_target), true)
    } else { return None };
    // the post-hook's Dv.sync / Mpage.sync / Fv.sync would add commands for what is on show now: not ported (the helper's)
    if thread_actions::syncs_pending(&session) { return None }
    let mut cmds = cmds;
    bind_all(f, &mut cmds);
    if !has_fab { fab_tail(f, hub, &mut session, &mut cmds) }
    Some(Dispatched { session, cmds, next_target })
}

/// The facts over the presenter's `state::State`. The lean state keeps only the settings the policies read (state/policy.rs `lean_keeps`): `order.enabled`, `order.estimates` and
/// `account.thread.<t>.<provider>` are not among them, so over a lean state they read as unset (Ordering on, estimates on, no account binding): state is asked to keep those prefixes.
pub struct StateFacts<'a>(pub &'a crate::state::State);
impl composer::Facts for StateFacts<'_> {
    fn bot_thread(&self, thread: &str) -> bool { self.0.bot_threads_has(thread) }
    fn account_params(&self, thread: &str) -> bool { self.account_binding(thread).is_some() }
    fn account_binding(&self, thread: &str) -> Option<String> {
        let t = self.0.threads.get(thread)?;
        let raw = self.0.settings.get(&format!("account.thread.{thread}.{}", t.provider))?;
        if raw.is_empty() { return None }
        // Accounts.named: the binding's id is not empty (an unparsable binding reads as "!invalid", also not empty)
        let named = serde_json::from_str::<Value>(raw).ok().map_or(true, |j| !j["id"].as_str().unwrap_or("").is_empty());
        named.then(|| raw.clone())
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
    fn attach_cap(&self) -> u64 { self.0.settings.get("files.max.mb").and_then(|v| v.trim().parse::<u64>().ok()).map_or(512, |n| n.clamp(1, 4000)) << 20 }
    fn root_of(&self, id: &str) -> String {
        let Some(t) = self.0.threads.get(id) else { return String::new() };
        if !t.worktree.is_empty() { t.worktree.clone() } else { self.0.projects.get(&t.project).map_or(String::new(), |p| p.root.clone()) }
    }
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
    impl Facts for M { fn title_of(&self, _: &str) -> String { "T".into() } fn root_of(&self, _: &str) -> String { "/w/r".into() } }
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
        let f = M { exists: T, running: false };
        let d = dispatch(&f, &params("answer", "ask1|yes|really", session())).unwrap();
        assert_eq!(d.cmds[0]["text"], json!("{\"id\":2,\"m\":\"ask.answer\",\"p\":{\"id\":\"ask1\",\"answer\":\"yes|really\"}}"));
        assert_eq!(d.session["rid"], json!(3));
        let d = dispatch(&f, &params("interrupt", "", session())).unwrap();
        assert_eq!(d.cmds[0]["text"], json!("{\"id\":2,\"m\":\"turn.interrupt\",\"p\":{\"thread\":\"t1700000002-3\"}}"));
        let d = dispatch(&f, &params("copy", "x", session())).unwrap();
        assert!(d.cmds.is_empty() && d.session["rid"] == json!(2));
    }
    struct Acct(M);
    impl composer::Facts for Acct { fn bot_thread(&self, _: &str) -> bool { false } fn account_params(&self, _: &str) -> bool { true } fn account_binding(&self, _: &str) -> Option<String> { Some("{\"id\":\"a1\",\"epoch\":2}".into()) } }
    impl select::Facts for Acct {
        fn thread_exists(&self, id: &str) -> bool { self.0.thread_exists(id) } fn turn_name(&self, id: &str) -> String { self.0.turn_name(id) }
        fn order_enabled(&self) -> bool { true } fn estimates_on(&self) -> bool { true }
    }
    impl Facts for Acct { fn title_of(&self, _: &str) -> String { "T".into() } fn root_of(&self, _: &str) -> String { "/w/r".into() } }
    #[test] fn a_thread_bound_to_an_account_carries_the_binding_first() {
        let f = Acct(M { exists: T, running: false });
        let d = dispatch(&f, &params("interrupt", "", session())).unwrap();
        assert_eq!(d.cmds[0]["text"], json!("{\"id\":2,\"m\":\"turn.interrupt\",\"p\":{\"accountBinding\":\"{\\\"id\\\":\\\"a1\\\",\\\"epoch\\\":2}\",\"thread\":\"t1700000002-3\"}}"));
        let d = dispatch(&f, &params("send", "", session())).unwrap();
        let send = d.cmds.iter().find(|c| c["type"] == "send").unwrap()["text"].as_str().unwrap().to_string();
        assert!(send.contains("\"p\":{\"accountBinding\":"), "{send}");
        assert_eq!(d.session["outbox"]["2"].as_str().unwrap(), send, "the kept copy is the same text");
        let d = dispatch(&f, &params("answer", "a|b", session())).unwrap();
        assert!(!d.cmds[0]["text"].as_str().unwrap().contains("accountBinding"), "no thread in the params, no binding");
    }
    #[test] fn what_nobody_owns_is_none() {
        let f = M { exists: T, running: false };
        for a in ["", "skill", "fm-open", "settings"] { assert!(dispatch(&f, &params(a, "x", session())).is_none(), "{a}") }
        let mut open = session(); open["panel"] = json!(true); open["info"] = json!({format!("view.kind:{T}"): "diff"});
        assert!(dispatch(&f, &params("send", "", open)).is_none(), "a Diff on show: Dv.sync would ask again, not ported");
        assert!(dispatch(&f, &params("select", "h|unknown", session())).is_none());
    }
}

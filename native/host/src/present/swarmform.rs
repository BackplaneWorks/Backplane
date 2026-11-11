//! The swarm forms (docs/SWARM.md): New swarm, opened from a project's menu, and a swarm page's add-member picker.
//! New swarm is open while the session's scratch `@snew` holds its project's id; its typed fields are `@f.stopic`, `@f.sgoal`, `@f.sseed` and each member's role
//! `srole.<i>` (the `bfield` action); its members are `@snew.members`, one "provider\tmodel\trole" a line. Start is the hub's `swarm.create`.
//! The add picker is open while `@sadd` is "1", with its choice in `@sadd.provider` / `@sadd.model`; its Add is the page's `swarm-act add|…`.
//! The model lists are the composer picker's (`keys::picker_agents`), so a member's choices are the ones a thread's are.
use crate::state::State;
use serde_json::{json, Value};

pub use super::botforms::Out;
pub const MIN: usize = 2;
pub const MAX: usize = 8;
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn draft<'a>(session: &'a Value, k: &str) -> &'a str { s(&session["drafts"], k) }
fn known(p: &str) -> bool { super::keys::AGENTS.iter().any(|(a, _)| *a == p) }
fn default_provider(st: &State) -> &str { let p = st.settings.get("agent.provider").map(String::as_str).unwrap_or("claude"); if known(p) { p } else { "claude" } }

#[derive(Clone, Debug, PartialEq)]
pub struct Row { pub provider: String, pub model: String, pub role: String }
pub fn rows(session: &Value) -> Vec<Row> {
    draft(session, "@snew.members").lines().map(|l| { let mut it = l.splitn(3, '\t'); Row { provider: it.next().unwrap_or("").into(), model: it.next().unwrap_or("").into(), role: it.next().unwrap_or("").into() } })
        .filter(|r| !r.provider.is_empty()).collect()
}
fn pack(rows: &[Row]) -> String { rows.iter().map(|r| format!("{}\t{}\t{}", r.provider, r.model, r.role.replace(['\t', '\n'], " "))).collect::<Vec<_>>().join("\n") }
fn index(v: &str) -> Option<usize> { v.parse().ok() }

/// None for what is not a swarm form's action.
pub fn act(st: &State, hub: &str, session: &Value, action: &str, value: &str) -> Option<Out> {
    let open = !draft(session, "@snew").is_empty();
    let mut rid = session["rid"].as_u64().unwrap_or(1);
    let mut cmds = vec![];
    let set = |rs: &[Row]| json!({"drafts": {"@snew.members": pack(rs)}});
    let patch = match action {
        "swarm-new" => {
            let pid = value.rsplit('|').next().unwrap_or(value);
            if !st.projects.get(pid).map_or(false, |p| !p.removed) { return None }
            let p = default_provider(st).to_string();
            let two = vec![Row { provider: p.clone(), model: String::new(), role: String::new() }; MIN];
            json!({"drafts": {"@snew": pid, "@bnew": "", "@rnew": "", "@f.stopic": "", "@f.sgoal": "", "@f.sseed": "", "@snew.members": pack(&two)}})
        }
        "form-close" => match value { "@snew" => json!({"drafts": {"@snew": ""}}), "@sadd" => json!({"drafts": {"@sadd": ""}}), _ => return None },
        "bfield" => {
            let (name, text) = value.split_once('\u{1f}')?;
            match name {
                "stopic" | "sgoal" | "sseed" => json!({"drafts": {format!("@f.{name}"): text}}),
                n if n.starts_with("srole.") && open => { let i = index(&n[6..])?; let mut rs = rows(session); rs.get_mut(i)?.role = text.to_string(); set(&rs) }
                _ => return None,
            }
        }
        "swarm-row-add" if open => { let mut rs = rows(session); if rs.len() >= MAX { return Some(Out { patch: json!({}), cmds }) }
            rs.push(Row { provider: rs.last().map_or_else(|| default_provider(st).to_string(), |r| r.provider.clone()), model: String::new(), role: String::new() }); set(&rs) }
        "swarm-row-del" if open => { let mut rs = rows(session); let i = index(value)?; if rs.len() <= MIN || i >= rs.len() { return Some(Out { patch: json!({}), cmds }) } rs.remove(i); set(&rs) }
        // a member's agent: its model goes back to the agent's default
        "swarm-row-provider" if open => { let (i, p) = value.split_once('\u{1f}')?; if !known(p) { return None } let mut rs = rows(session); let r = rs.get_mut(index(i)?)?; r.provider = p.into(); r.model.clear(); set(&rs) }
        "swarm-row-model" if open => { let (i, m) = value.split_once('\u{1f}')?; let mut rs = rows(session); rs.get_mut(index(i)?)?.model = m.into(); set(&rs) }
        "swarm-start" if open => {
            let topic = draft(session, "@f.stopic").trim();
            let rs = rows(session);
            if topic.is_empty() || !(MIN..=MAX).contains(&rs.len()) { return Some(Out { patch: json!({}), cmds }) }
            let members: Vec<Value> = rs.iter().map(|r| { let mut m = json!({"provider": r.provider}); if !r.model.is_empty() { m["model"] = json!(r.model) } if !r.role.trim().is_empty() { m["role"] = json!(r.role.trim()) } m }).collect();
            let mut p = json!({"project": draft(session, "@snew"), "topic": topic, "members": members});
            for (k, f) in [("goal", "@f.sgoal"), ("seed", "@f.sseed")] { let t = draft(session, f).trim(); if !t.is_empty() { p[k] = json!(t) } }
            cmds.push(json!({"type": "send", "hub": hub, "text": json!({"id": rid, "m": "swarm.create", "p": p}).to_string()})); rid += 1;
            json!({"drafts": {"@snew": "", "@f.stopic": "", "@f.sgoal": "", "@f.sseed": "", "@snew.members": ""}})
        }
        // the swarm page's add-member picker
        "swarm-add-open" => json!({"drafts": {"@sadd": "1", "@sadd.provider": default_provider(st), "@sadd.model": ""}}),
        "swarm-add-provider" if known(value) => json!({"drafts": {"@sadd.provider": value, "@sadd.model": ""}}),
        "swarm-add-model" => json!({"drafts": {"@sadd.model": value}}),
        _ => return None,
    };
    let mut patch = patch;
    if !cmds.is_empty() { patch["rid"] = json!(rid); }
    Some(Out { patch, cmds })
}

/// An agent's choices (`agents` is `keys::picker_agents`): each agent, whether it can be used here, and which is on.
fn providers(agents: &Value, on: &str) -> Value {
    Value::Array(super::keys::AGENTS.iter().map(|(p, what)| json!({"label": p, "value": p, "note": what, "on": *p == on, "ok": agents[*p]["ok"] != false})).collect())
}
/// The agent's models, the one chosen on.
fn models(agents: &Value, provider: &str, model: &str) -> Value {
    Value::Array(agents[provider]["models"].as_array().into_iter().flatten().map(|m| json!({"label": m["label"], "value": m["value"], "note": m["note"], "on": s(m, "value") == model})).collect())
}

/// The screen's `newSwarm` object: None while the form is shut.
pub fn new_swarm(st: &State, session: &Value, agents: &Value) -> Option<Value> {
    let pid = draft(session, "@snew"); if pid.is_empty() { return None }
    let rs = rows(session);
    let topic = draft(session, "@f.stopic");
    Some(json!({"project": pid, "title": st.projects.get(pid).map_or("", |p| p.title.as_str()), "topic": topic, "goal": draft(session, "@f.sgoal"), "seed": draft(session, "@f.sseed"),
        "members": rs.iter().map(|r| json!({"provider": r.provider, "model": r.model, "role": r.role, "providers": providers(agents, &r.provider), "models": models(agents, &r.provider, &r.model)})).collect::<Vec<_>>(),
        "canAdd": rs.len() < MAX, "canRemove": rs.len() > MIN, "ok": !topic.trim().is_empty() && (MIN..=MAX).contains(&rs.len())}))
}
/// A swarm page's `add` object (under `roomPage.swarm`): the picker's state; absent once the swarm is over or full.
pub fn add_block(session: &Value, swarm: &Value, agents: &Value) -> Option<Value> {
    if swarm["ended"] == true { return None }
    let live = swarm["members"].as_array().map_or(0, |m| m.iter().filter(|m| s(m, "state") != "removed").count());
    if live >= MAX { return None }
    let (p, m) = (draft(session, "@sadd.provider"), draft(session, "@sadd.model"));
    let p = if known(p) { p } else { "claude" };
    Some(json!({"open": draft(session, "@sadd") == "1", "provider": p, "model": m, "providers": providers(agents, p), "models": models(agents, p, m)}))
}
/// The thread a swarm's page draws its viewer for (Board, Schematic, 3D, Split, Mech, Files: they are a thread's, and the swarm's own
/// is its project's design): its first member still in it, "" for a plain room or a swarm with none. Its members are top-level threads of
/// the swarm's project, so the canonical design they share is the one shown.
pub fn viewer_thread(room_page: &Value) -> String {
    room_page["swarm"]["members"].as_array().into_iter().flatten().find(|m| !s(m, "thread").is_empty() && s(m, "state") != "removed").map(|m| s(m, "thread").to_string()).unwrap_or_default()
}
/// The `swarm.act` params of a swarm page's `swarm-act` value: pause, resume, stop; `add|<provider>|<model>|<role>`; `remove|<thread>` (a member's).
/// None for a value the page does not offer.
pub fn act_params(swarm: &Value, room: &str, value: &str) -> Option<Value> {
    if swarm["ended"] == true { return None }
    match value {
        "pause" | "resume" | "stop" => return Some(json!({"room": room, "action": value})),
        _ => {}
    }
    if let Some(rest) = value.strip_prefix("add|") {
        let mut it = rest.splitn(3, '|');
        let (p, m, r) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or("").trim());
        if !known(p) { return None }
        let mut q = json!({"room": room, "action": "add", "provider": p});
        if !m.is_empty() { q["model"] = json!(m) } if !r.is_empty() { q["role"] = json!(r) }
        return Some(q)
    }
    let t = value.strip_prefix("remove|")?;
    let member = swarm["members"].as_array()?.iter().any(|m| !t.is_empty() && s(m, "thread") == t && s(m, "state") != "removed");
    member.then(|| json!({"room": room, "action": "remove", "thread": t}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_swarm_page_draws_its_viewer_for_its_first_member_still_in_it() {
        let page = json!({"kind": "room", "swarm": {"members": [{"member": "cat", "thread": "", "state": "bot"}, {"member": "thread:t1", "thread": "t1", "state": "removed"}, {"member": "thread:t2", "thread": "t2", "state": "idle"}, {"member": "thread:t3", "thread": "t3", "state": "working"}]}});
        assert_eq!(viewer_thread(&page), "t2");
        assert_eq!(viewer_thread(&json!({"kind": "room", "posts": []})), "", "a plain room has no viewer");
        assert_eq!(viewer_thread(&json!({"kind": "room", "swarm": {"members": [{"thread": "t1", "state": "removed"}]}})), "", "nobody left");
    }
    use crate::session::patch::merge;
    fn sess(d: Value) -> Value { json!({"rid": 5, "drafts": d}) }
    fn st() -> State { let mut st = State::default(); st.projects.insert("p1".into(), crate::state::Project { title: "Motor".into(), root: "/r".into(), created: 1, removed: false }); st }
    fn agents() -> Value { super::super::keys::picker_agents(Some(&json!({"claude": ["opus", "sonnet"], "codex": ["gpt-6"]}))) }
    fn req(o: &Out) -> Value { serde_json::from_str(o.cmds[0]["text"].as_str().unwrap()).unwrap() }
    fn run(st: &State, s: &mut Value, a: &str, v: &str) -> Out { let o = act(st, "h", s, a, v).unwrap(); merge(s, &o.patch); o }

    #[test] fn new_swarm_opens_on_a_project_with_two_members_and_starts_swarm_create() {
        let st = st(); let mut s = sess(json!({}));
        assert!(act(&st, "h", &s, "swarm-new", "nope").is_none(), "an unknown project opens nothing");
        run(&st, &mut s, "swarm-new", "h|p1");
        let f = new_swarm(&st, &s, &agents()).unwrap();
        assert_eq!((f["project"].as_str(), f["title"].as_str(), f["members"].as_array().unwrap().len(), f["ok"].as_bool(), f["canRemove"].as_bool()), (Some("p1"), Some("Motor"), 2, Some(false), Some(false)));
        assert!(f["members"][0]["models"].as_array().unwrap().iter().any(|m| m["value"] == "opus"), "the composer's model list");
        assert!(act(&st, "h", &s, "swarm-start", "").unwrap().cmds.is_empty(), "no topic: nothing is sent");
        for (k, v) in [("stopic", "debounce"), ("sgoal", "a clean button"), ("sseed", "see src/btn.rs")] { run(&st, &mut s, "bfield", &format!("{k}\u{1f}{v}")); }
        run(&st, &mut s, "swarm-row-provider", "1\u{1f}codex"); run(&st, &mut s, "swarm-row-model", "0\u{1f}opus"); run(&st, &mut s, "bfield", "srole.1\u{1f}tester");
        run(&st, &mut s, "swarm-row-add", ""); assert_eq!(rows(&s).len(), 3); run(&st, &mut s, "swarm-row-del", "2");
        let f = new_swarm(&st, &s, &agents()).unwrap(); assert_eq!(f["ok"], true); assert_eq!(f["members"][1]["providers"].as_array().unwrap().iter().find(|p| p["on"] == true).unwrap()["value"], "codex");
        let o = run(&st, &mut s, "swarm-start", "");
        let r = req(&o);
        assert_eq!(r["m"], "swarm.create"); assert_eq!(r["id"], 5);
        assert_eq!(r["p"], json!({"project": "p1", "topic": "debounce", "goal": "a clean button", "seed": "see src/btn.rs", "members": [{"provider": "claude", "model": "opus"}, {"provider": "codex", "role": "tester"}]}));
        assert!(new_swarm(&st, &s, &agents()).is_none() && s["rid"] == 6, "the form shuts");
    }
    #[test] fn a_swarm_keeps_two_to_eight_members() {
        let st = st(); let mut s = sess(json!({}));
        run(&st, &mut s, "swarm-new", "p1");
        run(&st, &mut s, "swarm-row-del", "0"); assert_eq!(rows(&s).len(), 2, "never fewer than two");
        for _ in 0..10 { run(&st, &mut s, "swarm-row-add", ""); }
        assert_eq!(rows(&s).len(), 8); assert_eq!(new_swarm(&st, &s, &agents()).unwrap()["canAdd"], false);
        assert!(act(&st, "h", &s, "swarm-row-provider", "0\u{1f}nobody").is_none(), "an unknown agent is not ours");
        run(&st, &mut s, "form-close", "@snew"); assert!(new_swarm(&st, &s, &agents()).is_none());
        assert!(act(&st, "h", &s, "swarm-row-add", "").is_none(), "a row action with the form shut is not ours");
    }
    #[test] fn the_page_actions_map_onto_swarm_act() {
        let sw = json!({"ended": false, "members": [{"thread": "t1", "state": "idle"}, {"thread": "t2", "state": "removed"}]});
        assert_eq!(act_params(&sw, "r1", "pause"), Some(json!({"room": "r1", "action": "pause"})));
        assert_eq!(act_params(&sw, "r1", "add|codex|gpt-6|critic"), Some(json!({"room": "r1", "action": "add", "provider": "codex", "model": "gpt-6", "role": "critic"})));
        assert_eq!(act_params(&sw, "r1", "add|claude||"), Some(json!({"room": "r1", "action": "add", "provider": "claude"})));
        assert_eq!(act_params(&sw, "r1", "add|claude|opus|a|b"), Some(json!({"room": "r1", "action": "add", "provider": "claude", "model": "opus", "role": "a|b"})), "a role may hold a bar");
        assert_eq!(act_params(&sw, "r1", "remove|t1"), Some(json!({"room": "r1", "action": "remove", "thread": "t1"})));
        for bad in ["remove|t2", "remove|t9", "remove|", "add|nobody|x|", "delete", "budget"] { assert_eq!(act_params(&sw, "r1", bad), None, "{bad}"); }
        assert_eq!(act_params(&json!({"ended": true, "members": []}), "r1", "resume"), None, "an ended swarm offers nothing");
    }
    #[test] fn the_add_picker_follows_its_choice_and_leaves_with_the_swarm() {
        let st = st(); let mut s = sess(json!({}));
        let sw = json!({"ended": false, "members": [{"thread": "t1", "state": "idle"}]});
        assert_eq!(add_block(&s, &sw, &agents()).unwrap()["open"], false);
        run(&st, &mut s, "swarm-add-open", ""); run(&st, &mut s, "swarm-add-provider", "codex"); run(&st, &mut s, "swarm-add-model", "gpt-6");
        let a = add_block(&s, &sw, &agents()).unwrap();
        assert_eq!((a["open"].as_bool(), a["provider"].as_str(), a["model"].as_str()), (Some(true), Some("codex"), Some("gpt-6")));
        assert!(a["models"].as_array().unwrap().iter().any(|m| m["value"] == "gpt-6" && m["on"] == true));
        run(&st, &mut s, "form-close", "@sadd"); assert_eq!(add_block(&s, &sw, &agents()).unwrap()["open"], false);
        assert!(add_block(&s, &json!({"ended": true}), &agents()).is_none());
        let full = json!({"members": (0..8).map(|i| json!({"thread": format!("t{i}"), "state": "idle"})).collect::<Vec<_>>()}); assert!(add_block(&s, &full, &agents()).is_none());
    }
}

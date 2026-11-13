//! Live co-editing with a co-editing KiCad (bp.11, capability `coedit`; docs/kicad-coedit.md): the person sees what each agent works on,
//! and can pause, resume or steer it from KiCad's Agents panel without stopping it.
//!   - `announce`: before an agent's KiCad call, tell the open editor its task (`SetAgentActivity`, at most every 30 s per thread);
//!     `idle` at the turn's end. KiCad draws the area the agent's recent changes cover, labelled with the task.
//!   - ET_AGENT_CONTROL (23) from the editor: `pause` holds that thread's KiCad writes here until `resume` (the turn keeps running: its
//!     tool call just waits), `steer` goes into the thread as a steering message.
//!   - ET_AGENT_NOTICE (21) from the editor: what happened to an agent's change (the person's undo took it back, it waited for the person
//!     and was rebased, it was dropped, a settings merge conflicted). Published as info `kicad.coedit.<digest of the .kicad_pro>`
//!     {project, seq, kind, actor, agent, doc, text, at} (the pane's strip and the phones show it) and handed to that agent's thread as a
//!     note it reads with its next tool result.
//!   - What the person is doing (`person`): their selection (ET_SELECTION_CHANGED, bp.9+) and their own recent edits (ET_DOCUMENT_EDITED
//!     with actor "", bp.11), per project; every live kicad_ipc result carries the last two minutes of it, so an agent sees when it is
//!     about to touch what the person is working on and asks them instead of editing over them.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::{Arc, Weak};

static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);
pub fn start(hub: &Arc<Hub>) { *HUB.lock().unwrap() = Some(Arc::downgrade(hub)); }
use std::{collections::{HashMap, HashSet}, sync::{Mutex, OnceLock}, time::{Duration, Instant}};
use tokio::sync::Notify;

struct State { paused: HashSet<String>, announced: HashMap<String, Instant>, wake: std::sync::Arc<Notify>, person: HashMap<String, Person> }
fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State { paused: HashSet::new(), announced: HashMap::new(), wake: std::sync::Arc::new(Notify::new()), person: HashMap::new() }))
}

/// The person's activity in one project's editors.
#[derive(Default)]
struct Person { selection: Option<(String, Value, Instant)>, edits: std::collections::VecDeque<(Instant, Value)> }

/// How long the person's activity is news to an agent.
const PERSON_RECENT: Duration = Duration::from_secs(120);

/// The person selected something in KiCad (ET_SELECTION_CHANGED): `frame` "pcb"/"sch", `names` its ItemNames (empty: cleared).
pub fn person_selected(pro: &str, frame: &str, names: Value) {
    let mut s = state().lock().unwrap();
    let p = s.person.entry(pro.to_string()).or_default();
    let empty = names.as_object().map(|o| o.values().all(|v| v.as_array().map(|a| a.is_empty()).unwrap_or(true))).unwrap_or(true);
    p.selection = if empty { None } else { Some((frame.to_string(), names, Instant::now())) };
}

/// ET_DOCUMENT_EDITED (20): the person's own change (actor ""), kept for PERSON_RECENT. Returns whether `ev` was one.
pub fn on_edited(pro: &str, ev: &Value) -> bool {
    if !(ev["type"] == "ET_DOCUMENT_EDITED" || ev["type"].as_i64() == Some(20)) { return false }
    let Ok(d) = serde_json::from_str::<Value>(ev["detail"].as_str().unwrap_or("")) else { return true };
    if !d["actor"].as_str().unwrap_or("").is_empty() || d["undo"].as_bool().unwrap_or(false) { return true }
    let ids = |k: &str| d[k].as_array().cloned().unwrap_or_default();
    let mut items: Vec<Value> = ids("created"); items.extend(ids("modified")); items.extend(ids("deleted")); items.truncate(24);
    let edit = json!({"doc": d["doc"], "description": d["description"], "items": items});
    let mut s = state().lock().unwrap();
    let p = s.person.entry(pro.to_string()).or_default();
    let now = Instant::now();
    p.edits.retain(|(t, _)| now.duration_since(*t) < PERSON_RECENT);
    p.edits.push_back((now, edit));
    while p.edits.len() > 12 { p.edits.pop_front(); }
    true
}

/// What the person has been doing in this project's KiCad in the last two minutes, for an agent: None when nothing.
pub fn person_now(pro: &str) -> Option<Value> {
    let s = state().lock().unwrap();
    let p = s.person.get(pro)?;
    let now = Instant::now();
    let ago = |t: &Instant| now.duration_since(*t).as_secs();
    let sel = p.selection.as_ref().filter(|(_, _, t)| now.duration_since(*t) < PERSON_RECENT)
        .map(|(f, n, t)| json!({"editor": f, "items": n, "seconds_ago": ago(t)}));
    let edits: Vec<Value> = p.edits.iter().rev().filter(|(t, _)| now.duration_since(*t) < PERSON_RECENT)
        .map(|(t, e)| { let mut e = e.clone(); e["seconds_ago"] = json!(ago(t)); e }).collect();
    if sel.is_none() && edits.is_empty() { return None }
    let mut v = json!({"note": "The person is working in KiCad now. If your change touches these items or their area, ask them first (wait, or take it?) and do other work meanwhile."});
    if let Some(sel) = sel { v["selected"] = sel; }
    if !edits.is_empty() { v["recent_edits"] = json!(edits); }
    Some(v)
}

/// The thread an actor names ("agent:t123" -> "t123").
pub fn thread_of(actor: &str) -> Option<&str> { actor.strip_prefix("agent:").map(|t| t.split('#').next().unwrap_or(t)).filter(|t| !t.is_empty()) }

/// Whether `ev` is ET_AGENT_CONTROL.
pub fn is_control(ev: &Value) -> bool { ev["type"] == "ET_AGENT_CONTROL" || ev["type"].as_i64() == Some(23) }

/// The control in an event: (thread, action, text).
pub fn control_of(ev: &Value) -> Option<(String, String, String)> {
    let d: Value = serde_json::from_str(ev["detail"].as_str().unwrap_or("")).ok()?;
    let th = thread_of(d["actor"].as_str()?)?.to_string();
    Some((th, d["action"].as_str().unwrap_or("").to_string(), d["text"].as_str().unwrap_or("").to_string()))
}

pub fn paused(thread: &str) -> bool { state().lock().unwrap().paused.contains(thread) }

/// Apply a control: pause / resume here; steer goes to the thread.
pub fn on_event(ev: &Value) -> bool {
    if !is_control(ev) { return false }
    let Some((th, action, text)) = control_of(ev) else { return true };
    match action.as_str() {
        "pause" => { state().lock().unwrap().paused.insert(th); }
        "resume" => { let mut s = state().lock().unwrap(); s.paused.remove(&th); s.wake.notify_waiters(); }
        "steer" if !text.trim().is_empty() => {
            if let Some(ag) = crate::agents::handle() {
                let msg = format!("The person, from KiCad (keep going with this in mind): {text}");
                tokio::spawn(async move { let _ = ag.steer(&th, &msg).await; });
            }
        }
        _ => {}
    }
    true
}

/// A write from a paused thread waits here until the person resumes it (at most 30 minutes; then it goes ahead with a note).
pub async fn wait_resumed(thread: &str) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(30 * 60);
    loop {
        let wake = { let s = state().lock().unwrap(); if !s.paused.contains(thread) { return None } s.wake.clone() };
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() { return Some("the person paused you in KiCad 30 minutes ago and has not resumed you; your change went ahead".into()) }
        let _ = tokio::time::timeout(left.min(Duration::from_secs(5)), wake.notified()).await;
    }
}

/// Whether to announce the thread's task now (first call, or 30 s since the last).
pub fn due(thread: &str) -> bool {
    let mut s = state().lock().unwrap();
    let now = Instant::now();
    match s.announced.get(thread) { Some(t) if now.duration_since(*t) < Duration::from_secs(30) => false, _ => { s.announced.insert(thread.to_string(), now); true } }
}
pub fn forget(thread: &str) { let mut s = state().lock().unwrap(); s.announced.remove(thread); }

/// Who a request to the open KiCad is from, for its co-editing (the change log, the Agents panel, whose undo takes it back).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Who<'a> {
    /// An agent turn: its own actor, `agent:<thread>`. The person's Ctrl+Z leaves its changes; UndoAgentChange and the Agents panel take them back.
    Agent(&'a str),
    /// The person, through Backplane's UI (a BOM panel edit, fab-profile Apply, a panel's button): `person:backplane-<pid>`, which a
    /// co-editing KiCad (bp.13+, `person:` prefix) puts in the person's own undo, so their Ctrl+Z takes it back.
    Person,
    /// The hub's own housekeeping (3D-model migration and the like): its own actor `hub:<job>`, never the person's, so it never
    /// eats a Ctrl+Z they meant for their own edit; it shows in the change log under its job's name.
    Hub(&'a str),
}

/// The client name for a request from `who`: the one place the hub names KiCad callers (docs/kicad-coedit.md).
pub fn client_name(who: &Who) -> String {
    match who {
        Who::Agent(t) if !t.is_empty() => format!("agent:{t}"),
        Who::Agent(_) => "agent".into(),
        Who::Person => format!("person:backplane-{}", std::process::id()),
        Who::Hub(job) => format!("hub:{}", if job.is_empty() { "backplane" } else { job }),
    }
}

/// Whether `ev` is ET_AGENT_NOTICE.
pub fn is_notice(ev: &Value) -> bool { ev["type"] == "ET_AGENT_NOTICE" || ev["type"].as_i64() == Some(21) }

/// The agent's name as the person reads it: the thread's title when there is one, else "an agent".
fn agent_label(actor: &str, title: &str) -> String {
    if !title.trim().is_empty() { return title.trim().to_string() }
    if thread_of(actor).is_some() { "an agent".into() } else if actor.is_empty() { "you".into() } else { actor.to_string() }
}

/// A notice in words, for the person and for the agent (English: the hub's own sentences). `d`: {kind, actor, doc, details}.
pub fn notice_text(d: &Value, agent: &str) -> String {
    let det: Value = match &d["details"] { Value::String(t) => serde_json::from_str(t).unwrap_or(Value::Null), v => v.clone() };
    let what = det["description"].as_str().filter(|t| !t.is_empty()).map(|t| format!(" \u{201c}{t}\u{201d}")).unwrap_or_default();
    match d["kind"].as_str().unwrap_or("") {
        "undo_took_back" => {
            let by = det["by"].as_str().unwrap_or("person");
            let because = det["because"].as_str().filter(|t| !t.is_empty()).map(|t| format!(" (it touched what \u{201c}{t}\u{201d} changed)")).unwrap_or_default();
            if by == "person" { format!("Your undo also took back {agent}'s change{what}{because}.") } else { format!("An undo took back {agent}'s change{what}{because}.") }
        }
        "deferred" => format!("{agent}'s change{what} waits until you finish editing those items, then merges with your edit."),
        "rebased" => format!("{agent}'s change{what} was applied after your edit, merged with it."),
        "dropped" => format!("{agent}'s change{what} was dropped: it no longer fit what you had changed."),
        "settings_conflict" => format!("Board settings: you and {agent} changed the same value; yours was kept."),
        other => format!("{agent}: {other}{what}"),
    }
}

/// The same notice for the agent whose change it is (to its thread).
pub fn agent_text(d: &Value) -> String {
    let det: Value = match &d["details"] { Value::String(t) => serde_json::from_str(t).unwrap_or(Value::Null), v => v.clone() };
    let what = det["description"].as_str().filter(|t| !t.is_empty()).map(|t| format!(" \u{201c}{t}\u{201d}")).unwrap_or_default();
    let body = match d["kind"].as_str().unwrap_or("") {
        "undo_took_back" => {
            let because = det["because"].as_str().filter(|t| !t.is_empty()).map(|t| format!(", because it touched what \u{201c}{t}\u{201d} changed")).unwrap_or_default();
            if det["by"].as_str().unwrap_or("person") == "person" { format!("the person's undo took back your change{what}{because}. Re-read those items before changing them again; ask the person if you are unsure they want it.") }
            else { format!("an undo took back your change{what}{because}.") }
        }
        "deferred" => format!("your change{what} waits while the person edits those items; it is applied, merged with their edit, when they finish."),
        "rebased" => format!("your change{what} was applied after the person's edit, merged with it. Re-read the items if you depend on their exact state."),
        "dropped" => format!("your change{what} was dropped because it no longer fit the person's edit. Re-read the items and decide again."),
        "settings_conflict" => "the person and you changed the same board setting; theirs was kept.".to_string(),
        other => format!("{other}{what}."),
    };
    format!("KiCad co-editing: {body}")
}

/// The published value for one notice.
pub fn notice_value(pro: &str, seq: u64, d: &Value, title: &str, at: u64) -> Value {
    let actor = d["actor"].as_str().unwrap_or("");
    let agent = agent_label(actor, title);
    json!({"project": pro, "seq": seq, "kind": d["kind"], "actor": actor, "agent": agent, "doc": d["doc"], "text": notice_text(d, &agent), "at": at})
}

/// ET_AGENT_NOTICE: publish it for the pane and the phones, and tell the agent's thread. Returns whether `ev` was one.
pub fn on_notice(pro: &str, ev: &Value) -> bool {
    if !is_notice(ev) { return false }
    let Ok(d) = serde_json::from_str::<Value>(ev["detail"].as_str().unwrap_or("")) else { return true };
    let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return true };
    let pro = pro.to_string();
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    tokio::spawn(async move {
        let actor = d["actor"].as_str().unwrap_or("").to_string();
        let th = thread_of(&actor).map(String::from);
        let ag = crate::agents::handle();
        let title = match (&ag, &th) { (Some(a), Some(t)) => a.thread_title(t).await, _ => String::new() };
        let v = notice_value(&pro, seq, &d, &title, crate::hub::now_ms());
        hub.set_info(&format!("kicad.coedit.{}", super::kserve::digest(&pro)), &v.to_string()).await;
        // the agent learns what happened to its change, in its own terms
        if let (Some(a), Some(t)) = (ag, th) {
            let _ = a.ride_note(&t, &format!("kicad-notice-{seq}"), &agent_text(&d)).await;
        }
    });
    true
}

/// SetAgentActivity's message.
pub fn activity(thread: &str, task: &str, state_: &str) -> Value { json!({"actor": client_name(&Who::Agent(thread)), "task": task, "state": state_}) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn notices_read_as_sentences_for_the_person_and_the_agent() {
        let d = json!({"kind": "undo_took_back", "actor": "agent:t7", "doc": "pcb",
                       "details": json!({"seq": 9, "description": "Route USB", "because": "Move U3", "by": "person"}).to_string()});
        let v = notice_value("/p/b.kicad_pro", 3, &d, "Route the USB pair", 1000);
        assert_eq!(v["text"], "Your undo also took back Route the USB pair's change \u{201c}Route USB\u{201d} (it touched what \u{201c}Move U3\u{201d} changed).");
        assert_eq!((v["seq"].clone(), v["kind"].clone(), v["actor"].clone(), v["agent"].clone(), v["at"].clone()), (json!(3), json!("undo_took_back"), json!("agent:t7"), json!("Route the USB pair"), json!(1000)));
        assert!(agent_text(&d).starts_with("KiCad co-editing: the person's undo took back your change \u{201c}Route USB\u{201d}, because it touched what \u{201c}Move U3\u{201d} changed."), "{}", agent_text(&d));
        assert!(is_notice(&json!({"type": 21})) && is_notice(&json!({"type": "ET_AGENT_NOTICE"})) && !is_notice(&json!({"type": 20})));
        assert_eq!(notice_value("/p", 1, &json!({"kind": "deferred", "actor": "agent:t1"}), "", 0)["text"], "an agent's change waits until you finish editing those items, then merges with your edit.");
    }
    #[test] fn client_names_say_who() {
        assert_eq!(client_name(&Who::Agent("t42")), "agent:t42");
        assert_eq!(thread_of(&client_name(&Who::Agent("t42"))), Some("t42"));
        assert_eq!(client_name(&Who::Person), format!("person:backplane-{}", std::process::id()));
        assert_eq!(client_name(&Who::Hub("migrate-3d")), "hub:migrate-3d");
        assert!(thread_of(&client_name(&Who::Person)).is_none() && thread_of(&client_name(&Who::Hub("x"))).is_none());
    }
    #[test] fn actors_name_threads() {
        assert_eq!(thread_of("agent:t12"), Some("t12"));
        assert_eq!(thread_of("agent:t12#rebased"), Some("t12"));
        assert_eq!(thread_of("person"), None);
        assert_eq!(thread_of("agent:"), None);
    }
    #[test] fn a_pause_holds_until_resume_and_steer_needs_text() {
        let ev = |a: &str| json!({"type": 23, "detail": json!({"actor": "agent:tp1", "action": a, "text": ""}).to_string()});
        assert!(on_event(&ev("pause"))); assert!(paused("tp1"));
        assert!(on_event(&ev("resume"))); assert!(!paused("tp1"));
        assert!(!on_event(&json!({"type": "ET_COMMENT"})));
        assert_eq!(control_of(&json!({"type":23,"detail":"{\"actor\":\"agent:t9\",\"action\":\"steer\",\"text\":\"keep away from U2\"}"})), Some(("t9".into(), "steer".into(), "keep away from U2".into())));
    }
    #[test] fn announcing_is_throttled() { assert!(due("ta1")); assert!(!due("ta1")); forget("ta1"); assert!(due("ta1")); }
    #[tokio::test] async fn an_unpaused_thread_never_waits() { assert_eq!(wait_resumed("tz").await, None); }
    #[test] fn the_persons_selection_and_edits_reach_agents() {
        let pro = "/p/person-test.kicad_pro";
        assert!(person_now(pro).is_none());
        person_selected(pro, "sch", json!({"references": ["U3"], "pins": [], "nets": []}));
        let ev = |actor: &str| json!({"type": "ET_DOCUMENT_EDITED", "detail": json!({"seq": 4, "doc": "sch", "actor": actor, "description": "Move", "modified": ["k1"]}).to_string()});
        assert!(on_edited(pro, &ev("")) && on_edited(pro, &ev("agent:t9")) && !on_edited(pro, &json!({"type": "ET_FRAME_SHOWN"})));
        let v = person_now(pro).unwrap();
        assert_eq!(v["selected"]["items"]["references"][0], "U3");
        assert_eq!(v["recent_edits"].as_array().unwrap().len(), 1, "an agent's own edit is not the person's");
        assert_eq!(v["recent_edits"][0]["items"][0], "k1");
        person_selected(pro, "sch", json!({"references": []}));
        assert!(person_now(pro).unwrap().get("selected").is_none());
    }
}

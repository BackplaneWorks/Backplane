//! Live co-editing with a co-editing KiCad (bp.11, capability `coedit`; docs/kicad-coedit.md): the person sees what each agent works on,
//! and can pause, resume or steer it from KiCad's Agents panel without stopping it.
//!   - `announce`: before an agent's KiCad call, tell the open editor its task (`SetAgentActivity`, at most every 30 s per thread);
//!     `idle` at the turn's end. KiCad draws the area the agent's recent changes cover, labelled with the task.
//!   - ET_AGENT_CONTROL (23) from the editor: `pause` holds that thread's KiCad writes here until `resume` (the turn keeps running: its
//!     tool call just waits), `steer` goes into the thread as a steering message.
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, sync::{Mutex, OnceLock}, time::{Duration, Instant}};
use tokio::sync::Notify;

struct State { paused: HashSet<String>, announced: HashMap<String, Instant>, wake: std::sync::Arc<Notify> }
fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(State { paused: HashSet::new(), announced: HashMap::new(), wake: std::sync::Arc::new(Notify::new()) }))
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

/// SetAgentActivity's message.
pub fn activity(thread: &str, task: &str, state_: &str) -> Value { json!({"actor": format!("agent:{thread}"), "task": task, "state": state_}) }

#[cfg(test)]
mod tests {
    use super::*;
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
}

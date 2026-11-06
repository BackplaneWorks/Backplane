//! What the hub-side policies read of a thread's entries, kept as a small summary per thread instead of the entries
//! (src/core/autosettle.bend `Summary`): the newest time, whether the person wrote in it, which of their messages count as
//! theirs, the newest reply and its time, and when it last wrote to each thread. Bounded by threads and thread pairs.
use super::json::nat_read;
use crate::state::{link_from, State};
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Sum {
    /// the newest `at` of any entry filed under the thread (messages, acts, links it sent)
    pub quiet: u64,
    /// the person (or anyone) wrote a user message in it
    pub found: bool,
    /// the first two user messages of its own that Mail.classify reads as the person's (the seed excludes at most one)
    pub person: Vec<String>,
    /// id of its newest own assistant message, and the time of the newest
    pub output: String,
    pub output_at: u64,
    /// the text (first 2000 characters) of its newest own user message and assistant reply: what the summary writer reads (Sum.last)
    pub last_user: String,
    pub last_reply: String,
    /// the newest time it sent a link message into each thread (key: the target thread)
    pub sent: HashMap<String, u64>,
    /// id of the newest message the person typed in it (Policy.person: a user message of its own that Policy.direct reads as theirs)
    pub person_id: String,
    /// the first line (at most 200 characters) of its newest assistant message: what a "told" record says it was told (Told.first of Told.last)
    pub reply_line: String,
    /// the ids of its newest messages (at most 64): `Shelve.input.seen` asks whether a message was already posted to the thread
    pub recent_msgs: std::collections::VecDeque<String>,
}
#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Ext {
    /// TaskDelivered: tasks whose result was handed on
    pub delivered: HashSet<String>,
    pub sum: HashMap<String, Sum>,
    /// task index, kept by the fold so the writer-path checks do not scan the task table (it only grows): a parent's unfinished tasks, and
    /// the tasks whose child is a thread (smallest task id first: the one `parent_of` reads)
    pub kids_open: HashMap<String, u32>,
    pub child_tasks: HashMap<String, BTreeSet<String>>,
    /// MailPosted launch mails, newest time per "<sender thread>\t<to address>": the receipts `Auto.old.reports` reads before the one-time migration
    /// (autosettle.version "1"). Bounded by thread pairs. (Bend drops a mail whose id it already holds; the same id carries the same time.)
    pub reports: HashMap<String, u64>,
}

fn until(s: &str, end: char) -> String { s.chars().take_while(|c| *c != end).collect() }
fn drop_chars(s: &str, n: usize) -> String { s.chars().skip(n).collect() }
/// Mail.classify's kind for a message that is not a system one (core/mail.bend), the first rule that fits
pub fn classify_kind(from: &str, msg: &str, text: &str) -> &'static str {
    let head: String = text.chars().take(160).collect();
    let label = until(&head, ']');
    let base = |from: &str| if from.is_empty() { "person" } else if from.starts_with("bot:") { "bot" } else { "thread" };
    let has_room = label.contains(" in #");
    if head.starts_with("[message from ") && !has_room {
        if !from.is_empty() { return base(from) }
        let name = drop_chars(&label, 14);
        return if name.starts_with("person@") { "person" } else if name.contains('@') { "peer" } else { "bot" };
    }
    if head.starts_with("Backplane restarted while you were working") { return "carry" }
    if head.starts_with("[message from ") && has_room { return "room" }
    if head.starts_with("[routine: ") { return "routine" }
    if head.starts_with("[webhook: ") { return "hook" }
    if msg.starts_with("t:backplane:l") { return "lost" }
    let wake = { let w = until(&drop_chars(&head, 5), ':'); msg.contains(":lr-") && head.starts_with("Wake ") && head.contains(':') && !w.is_empty() && !w.contains(' ') };
    if wake { return "wake" }
    if from == "deploy" { return "deploy" }
    if from.starts_with("lock:") || msg.starts_with("t:lock:") { return "lock" }
    if msg.contains(":td-") || head.starts_with("[Backplane] Delegated task") { return "task" }
    if msg.contains(":lr-") || head.starts_with("[Backplane] Launched thread") { return "launch" }
    base(from)
}
/// Auto.direct: a message the person wrote themselves
pub fn direct(id: &str, text: &str) -> bool { classify_kind(link_from(id), id, text) == "person" }

/// one entry filed under `th` (and under its sender when a link message): the summary the policies read
pub fn on_post(st: &mut State, th: &str, id: &str, at: u64, msg: Option<(&str, &str)>) {
    let from = link_from(id).to_string();
    let own = st.ext.sum.entry(th.to_string()).or_default();
    own.quiet = own.quiet.max(at);
    if msg.is_some() { own.recent_msgs.push_back(id.to_string()); while own.recent_msgs.len() > 64 { own.recent_msgs.pop_front(); } }
    if from == th { let e = own.sent.entry(th.to_string()).or_insert(0); *e = (*e).max(at); }
    if let Some((role, text)) = msg {
        if role == "user" {
            own.found = true;
            own.last_user = text.chars().take(2000).collect();
            if own.person.len() < 2 && !own.person.iter().any(|x| x == id) && direct(id, text) { own.person.push(id.to_string()); }
            if super::turnend::policy_direct(id, text) { own.person_id = id.to_string(); }
        } else if role == "assistant" { own.output = id.to_string(); own.output_at = own.output_at.max(at); own.reply_line = super::turnend::told_first(text); own.last_reply = text.chars().take(2000).collect(); }
    }
    if !from.is_empty() && from != th {
        let s = st.ext.sum.entry(from).or_default();
        s.quiet = s.quiet.max(at);
        let e = s.sent.entry(th.to_string()).or_insert(0); *e = (*e).max(at);
    }
}
/// the task's (parent, child, finished) before a change, for `task_index`
pub fn task_before(st: &State, task: &str) -> Option<(String, String, bool)> { st.tasks.get(task).map(|t| (t.0.clone(), t.1.clone(), t.2)) }
/// keep the task indexes in step with a change of task `id`: take back what it was counted as, count what it is now
pub fn task_index(st: &mut State, id: &str, before: Option<(String, String, bool)>) {
    if let Some((p, c, term)) = before {
        if !term && !p.is_empty() { if let Some(n) = st.ext.kids_open.get_mut(&p) { *n = n.saturating_sub(1); if *n == 0 { st.ext.kids_open.remove(&p); } } }
        if !c.is_empty() { if let Some(s) = st.ext.child_tasks.get_mut(&c) { s.remove(id); if s.is_empty() { st.ext.child_tasks.remove(&c); } } }
    }
    if let Some((p, c, term)) = st.tasks.get(id).map(|t| (t.0.clone(), t.1.clone(), t.2)) {
        if !term && !p.is_empty() { *st.ext.kids_open.entry(p).or_insert(0) += 1; }
        if !c.is_empty() { st.ext.child_tasks.entry(c).or_default().insert(id.to_string()); }
    }
}
/// Task.of_child through the index: the thread's task (smallest id when it has several), as (parent, finished)
pub fn task_of_child<'a>(st: &'a State, child: &str) -> Option<(&'a str, bool)> {
    let id = st.ext.child_tasks.get(child)?.iter().next()?;
    st.tasks.get(id).map(|t| (t.0.as_str(), t.2))
}
/// Work.kids through the index: a thread's unfinished child tasks
pub fn open_kids(st: &State, parent: &str) -> usize { st.ext.kids_open.get(parent).copied().unwrap_or(0) as usize }

pub fn mail_posted(st: &mut State, id: &str, to: &str, kind: &str, at: u64) {
    if kind != "launch" { return }
    let e = st.ext.reports.entry(format!("{}\t{}", link_from(id), to)).or_insert(0); *e = (*e).max(at);
}
pub fn delivered(st: &mut State, task: &str) { st.ext.delivered.insert(task.to_string()); }
#[allow(dead_code)] pub fn digits(s: &str) -> Option<u64> { nat_read(s) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn classify_reads_the_person_and_the_envelopes() {
        // rust classify_reads_the_person_and_the_envelopes
        assert_eq!(classify_kind("", "m1-2", "hello"), "person");
        assert_eq!(classify_kind("", "m1", "[message from person@box] hi"), "person");
        assert_eq!(classify_kind("", "m1", "[message from bob@x] hi"), "peer");
        assert_eq!(classify_kind("", "m1", "[message from bot9] hi"), "bot");
        assert_eq!(classify_kind("a", "t:a:1", "[message from a] hi"), "thread");
        assert_eq!(classify_kind("", "m1", "[message from x in #room] hi"), "room");
        assert_eq!(classify_kind("", "m1", "[routine: daily] go"), "routine");
        assert_eq!(classify_kind("a", "t:a:lr-1", "x"), "launch");
        assert_eq!(classify_kind("a", "t:a:td-1", "x"), "task");
        assert_eq!(classify_kind("", "m1", "Backplane restarted while you were working"), "carry");
    }

    // the indexes against a plain scan of the task table, on a log with finished, cancelled, delivered, re-delegated and re-parented tasks
    #[test] fn the_task_indexes_equal_the_scan() {
        let mut st = State::new();
        let lines = [
            r#"{"$":"TaskDelegated","task":"k1","parent":"a","child":"b","depth":1,"provider":"x","model":"","at":1}"#,
            r#"{"$":"TaskDelegated","task":"k2","parent":"a","child":"c","depth":1,"provider":"x","model":"","at":2}"#,
            r#"{"$":"TaskDelegated","task":"k3","parent":"b","child":"d","depth":1,"provider":"x","model":"","at":3}"#,
            r#"{"$":"TaskStateSet","task":"k2","state":"cancelled","summary":"","at":4}"#,
            r#"{"$":"TaskStateSet","task":"k3","state":"running","summary":"","at":5}"#,
            r#"{"$":"TaskDelivered","task":"k2","at":6}"#,
            r#"{"$":"TaskDelegated","task":"k1","parent":"z","child":"b","depth":1,"provider":"x","model":"","at":7}"#, // k1 re-parented
            r#"{"$":"TaskDelegated","task":"k4","parent":"z","child":"b","depth":1,"provider":"x","model":"","at":8}"#, // a second task for child b
            r#"{"$":"TaskStateSet","task":"k4","state":"done","summary":"","at":9}"#,
            r#"{"$":"TaskStateSet","task":"ghost","state":"queued","summary":"","at":10}"#, // a task never delegated
            r#"{"$":"TaskDelegated","task":"k3","parent":"a","child":"e","depth":1,"provider":"x","model":"","at":11}"#, // k3 re-delegated to another child
            r#"{"$":"TaskStateSet","task":"k3","state":"failed","summary":"","at":12}"#,
            r#"{"$":"TaskDelegated","task":"k5","parent":"","child":"","depth":1,"provider":"x","model":"","at":13}"#,
        ];
        for (i, l) in lines.iter().enumerate() {
            st.apply_line(l);
            for th in ["a", "b", "c", "d", "e", "z", "ghost", "", "nobody"] {
                let kids = st.tasks.values().filter(|t| t.0 == th && !th.is_empty() && !t.2).count();
                assert_eq!(open_kids(&st, th), kids, "open kids of {th:?} after line {i}");
                let scan = st.tasks.iter().filter(|(_, t)| t.1 == th && !th.is_empty()).min_by(|a, b| a.0.cmp(b.0)).map(|(_, t)| (t.0.as_str(), t.2));
                assert_eq!(task_of_child(&st, th), scan, "task of child {th:?} after line {i}");
            }
        }
        assert_eq!(open_kids(&st, "z"), 1, "k1 (re-parented, unfinished) counts under z; k4 is done");
        assert_eq!(task_of_child(&st, "b"), Some(("z", false)), "child b: k1 sorts before k4");
    }
}

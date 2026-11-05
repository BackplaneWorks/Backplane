//! What the hub-side policies read of a thread's entries, kept as a small summary per thread instead of the entries
//! (src/core/autosettle.bend `Summary`): the newest time, whether the person wrote in it, which of their messages count as
//! theirs, the newest reply and its time, and when it last wrote to each thread. Bounded by threads and thread pairs.
use super::json::nat_read;
use crate::state::{link_from, State};
use std::collections::{HashMap, HashSet};

#[derive(Default, Debug, Clone)]
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
    /// the newest time it sent a link message into each thread (key: the target thread)
    pub sent: HashMap<String, u64>,
}
#[derive(Default, Debug)]
pub struct Ext {
    /// TaskDelivered: tasks whose result was handed on
    pub delivered: HashSet<String>,
    pub sum: HashMap<String, Sum>,
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
    if from == th { let e = own.sent.entry(th.to_string()).or_insert(0); *e = (*e).max(at); }
    if let Some((role, text)) = msg {
        if role == "user" {
            own.found = true;
            if own.person.len() < 2 && !own.person.iter().any(|x| x == id) && direct(id, text) { own.person.push(id.to_string()); }
        } else if role == "assistant" { own.output = id.to_string(); own.output_at = own.output_at.max(at); }
    }
    if !from.is_empty() && from != th {
        let s = st.ext.sum.entry(from).or_default();
        s.quiet = s.quiet.max(at);
        let e = s.sent.entry(th.to_string()).or_insert(0); *e = (*e).max(at);
    }
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
}

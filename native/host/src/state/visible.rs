//! A thread's status word and whether the sidebar shows it (docs/ALWAYS-ON.md D11; the sections are present/sections.rs). Pure on the State,
//! so the policy core (state/policy.rs: the look and cut marks) and the binaries that fold a log without the presenter share it.
use crate::shelf::{shelf_of, Shelf};
use crate::state::{State, Turn};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// how long a stop from before the look marks still shows (`looked`), and how recent a project is Active (present/sections.rs)
pub const ACTIVE: u64 = 3 * 86400;
fn get<'a>(st: &'a State, k: &str) -> &'a str { st.settings.get(k).map(String::as_str).unwrap_or("") }
fn num(st: &State, k: &str) -> u64 { get(st, k).parse().unwrap_or(0) }

// ---- one thread ------------------------------------------------------------------------------------------------------------
const ENDED: &str = "\u{1}ended:";
fn bg_lines(v: &str) -> impl Iterator<Item = &str> { v.split('\n').filter(|l| !l.is_empty()) }
fn doing(l: &str) -> &str { l.split('\t').nth(1).unwrap_or("") }
/// Work the thread owns runs after its turn: a background shell, monitor or subagent, a watch, a delegated task not finished, an order waited on.
pub fn owns_work(st: &State, id: &str) -> bool {
    bg_lines(get(st, &format!("bg.{id}"))).any(|l| !doing(l).starts_with(ENDED))
        || bg_lines(get(st, &format!("watch.{id}"))).any(|l| !doing(l).starts_with(ENDED))
        || st.tasks.values().any(|(p, _, term, _)| p == id && !*term)
        || !get(st, &format!("order.wait.{id}")).is_empty()
}
/// The person is asked something: an open ask (approval, question, plan, browser), a permission pending, a sign-in or a secret it waits on.
pub fn wants_person(st: &State, id: &str) -> bool {
    st.asks.values().any(|(th, open)| *open && th == id)
        || st.threads.get(id).map_or(false, |t| t.pending > 0)
        || !get(st, &format!("auth.wait.{id}")).is_empty()
        || secret_asks(st, id)
}
/// Whether a secret request (`secret.ask.<name>` = {thread,...}) waits on the person for this thread. The settings are read once per state and
/// fold position (a sidebar asks for every thread of several states each time it draws).
fn secret_asks(st: &State, id: &str) -> bool {
    thread_local! { static C: std::cell::RefCell<HashMap<usize, (u64, usize, HashSet<String>)>> = Default::default(); }
    let key = st as *const State as usize;
    C.with(|c| {
        let mut c = c.borrow_mut();
        let fresh = c.get(&key).map_or(false, |(seq, n, _)| *seq == st.seq && *n == st.settings.len());
        if !fresh {
            let set: HashSet<String> = st.settings.iter().filter(|(k, v)| k.starts_with("secret.ask.") && !v.is_empty())
                .filter_map(|(_, v)| serde_json::from_str::<Value>(v).ok().and_then(|j| j["thread"].as_str().map(String::from))).filter(|t| !t.is_empty()).collect();
            if c.len() > 16 { c.clear(); }
            c.insert(key, (st.seq, st.settings.len(), set));
        }
        c.get(&key).map_or(false, |(_, _, s)| s.contains(id))
    })
}
fn limit_held(st: &State, id: &str) -> bool {
    st.queue.get(id).map_or(false, |q| q.iter().any(|(m, _)| m.starts_with("fallback:held:"))) || !get(st, &format!("provider.resume.{id}")).is_empty()
}

/// A thread's status word, the one vocabulary of the sidebar and the registry (net/registry.rs stores it for a machine that is off):
/// `needs` (the person is asked), `running`, `limit` (held by a usage limit, it carries on at the reset), `failed`, `cut` (a restart cut its
/// turn), `interrupted`, `monitoring` (its turn is over, work it owns runs), `done` (its last turn completed), `idle`.
/// Ordered: the first that holds is the word.
pub fn status(st: &State, id: &str) -> &'static str {
    let Some(t) = st.threads.get(id) else { return "idle" };
    if t.deleted { return "idle" }
    if wants_person(st, id) { return "needs" }
    if limit_held(st, id) { return "limit" }
    // a turn the restart's line will carry on (`carry.wait.<t>`, agents/mod.rs) is resumed by itself: running work, not a stop
    // (a queued message alone is not running work: the hub starts it at once, and the log keeps many a TurnQueued nothing took, on threads long done)
    if t.turn == Turn::Running || (t.turn == Turn::Interrupted && get(st, &format!("carry.wait.{id}")) == "1") { return "running" }
    match t.turn {
        Turn::Failed => return "failed",
        Turn::Interrupted if !get(st, &format!("sidebar.cut.{id}")).is_empty() => return "cut",
        Turn::Interrupted => return "interrupted",
        _ => {}
    }
    if owns_work(st, id) { return "monitoring" }
    if t.turn == Turn::Completed { "done" } else { "idle" }
}
/// A word that is a stop the person has not chosen: it shows until they look at the thread or settle it.
pub fn abnormal(word: &str) -> bool { matches!(word, "failed" | "cut" | "interrupted") }
/// A word that always shows.
pub fn live(word: &str) -> bool { matches!(word, "needs" | "running" | "limit" | "monitoring") }
/// What Needs you lists: the person is asked, or a stop of one of the person's own threads (`own`) not looked at, written to or settled since.
pub fn needs_you(word: &str, own: bool, looked: bool, settled: bool) -> bool { word == "needs" || (abnormal(word) && own && !looked && !settled) }

/// A thread of the person's: one they started, or one a thread of theirs launched for them. Not a bot's chat or run, a mail run or its coordination,
/// a routine's run, a delegated child (its stop is its parent's to handle), or a thread a bot launched: backend chatter never shows. A bot's
/// failure shows once, as its bot row's state.
pub fn own(st: &State, id: &str) -> bool {
    let bot = |th: &str| st.bot_threads_has(th) || st.bots_nav.values().any(|b| b.thread == th) || st.runs.contains_key(th);
    let backend = |th: &str| bot(th) || st.run_hidden(th) || th.starts_with("run") || th.starts_with("mailcoord-");
    if backend(id) || st.swarm_of.contains_key(id) || st.tasks.values().any(|(_, c, _, _)| c == id) { return false }
    // launched by another thread (`launch.<t>` = kind + launcher): the person's when its launcher is
    // the uberbot's chat works for the person: what it launches is theirs (a lane of a lead, a thread asked for); any other bot's is backend work
    let by = get(st, &format!("launch.{id}")); let by = by.get(1..).unwrap_or("");
    let uber = st.bots_nav.get(get(st, "uber.bot")).map(|b| b.thread.as_str()).filter(|t| !t.is_empty());
    by.is_empty() || by == id || Some(by) == uber || !backend(by)
}

/// The person opened the thread after it last moved (the mark is written by `look`, at a viewer's view.open).
/// Every stop that exists when this build first runs (`sidebar.looked.since`, written once at that start, agents/groupname.rs) counts as seen: an
/// upgrade starts with an empty Needs you, and the threads stay on their project pages. A message the person sends moves the thread on (it runs,
/// then is done), so it leaves too.
pub fn looked(st: &State, id: &str) -> bool {
    st.threads.get(id).map_or(false, |t| {
        let l = num(st, &format!("sidebar.looked.{id}")); let since = num(st, "sidebar.looked.since");
        (l > 0 && l >= t.activity) || (since > 0 && t.activity < since)
    })
}
pub fn settled(st: &State, id: &str) -> bool { st.threads.get(id).map_or(false, |t| shelf_of(t) == Shelf::Settled || t.archived) }

/// D11 for a thread of a live state: whether the sidebar shows it.
pub fn visible(st: &State, id: &str) -> bool {
    let Some(t) = st.threads.get(id) else { return false };
    if t.deleted || st.run_hidden(id) { return false }
    // a swarm's member is no row of its own: its swarm's row stands for it, and says when a member asks the person (present/sections.rs)
    if st.swarm_of.contains_key(id) { return false }
    let w = status(st, id);
    // never hidden: what asks the person, whatever else is true of it (settled, archived, snoozed)
    if w == "needs" { return true }
    if t.archived { return false }
    t.pinned || live(w) || owns_work(st, id) || (abnormal(w) && own(st, id) && !looked(st, id) && !settled(st, id))
}
/// D11 for a row the registry holds (its machine is off): its word and pin alone. The look mark and the launcher are over there, so a stop
/// is listed under its project (dimmed) but never put in Needs you; an ask is.
pub fn visible_word(word: &str, pinned: bool) -> bool { pinned || live(word) || abnormal(word) }

/// Events a viewer's view.open of a thread adds (state/policy.rs `read`): the look mark, only for a thread whose stop it settles the sidebar for.
pub fn look(st: &State, id: &str, now: u64) -> Vec<Value> {
    let Some(t) = st.threads.get(id) else { return vec![] };
    if !abnormal(status(st, id)) || !own(st, id) || num(st, &format!("sidebar.looked.{id}")) >= t.activity.max(1) { return vec![] }
    vec![json!({"$":"SettingSet","key":format!("sidebar.looked.{id}"),"value":now.max(t.activity).to_string(),"at":now})]
}
/// What a committed change leaves for the sidebar (state/policy.rs `derive`): a turn that starts ends a restart's cut mark.
pub fn meta(st: &State, e: &Value) -> Vec<Value> {
    if e["$"] != "TurnChanged" || e["state"] != "running" { return vec![] }
    let th = e["thread"].as_str().unwrap_or("");
    if th.is_empty() || get(st, &format!("sidebar.cut.{th}")).is_empty() { return vec![] }
    vec![json!({"$":"SettingSet","key":format!("sidebar.cut.{th}"),"value":"","at":e["at"].as_u64().unwrap_or(0)})]
}


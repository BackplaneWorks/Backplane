//! Thread and project summaries (src/core/refs.bend `Sum.*`, hub.bend `Sum.out`/`Sum.got`/`Sum.next`): at a turn's end a due thread is summarized by the
//! text model; its answer is cleaned to one line and kept as `summary.<id>`, and the thread's project is summarized next when it is due. Pure: the
//! input texts, the cleaning and what a finished summary commits. The model call is the `agents` lane's `Summarizer`; when and what to ask is here.
use super::json::*;
use super::turnend::{bend_trim, sum_due};
use crate::state::State;
use serde_json::{json, Value};

fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }

/// Sum.thread.input: what the writer reads about a thread
pub fn thread_input(title: &str, project_title: &str, earlier: &str, last_user: &str, last_reply: &str) -> String {
    format!("Thread title: {title}\nProject: {project_title}\nEarlier summary: {earlier}\n\nThe user's latest message:\n{}\n\nThe agent's latest reply:\n{}", take(last_user, 2000), take(last_reply, 2000))
}
/// Sum.thread.input over the state: a thread's title, its project's, its summary so far and the newest user message and reply
pub fn thread_input_of(st: &State, id: &str) -> String {
    let t = st.threads.get(id);
    let title = t.map_or("", |t| t.title.as_str());
    let pt = t.and_then(|t| st.projects.get(&t.project)).map_or("", |p| p.title.as_str());
    let x = st.ext.sum.get(id);
    thread_input(title, pt, &st.get(&format!("summary.{id}"), ""), x.map_or("", |x| x.last_user.as_str()), x.map_or("", |x| x.last_reply.as_str()))
}
/// Sum.lines + Sum.threads: the newest threads of a project, a line each ("- title" and ": summary" when it has one); archived and deleted ones left out
pub fn project_lines(st: &State, pid: &str) -> Vec<String> {
    let mut ts: Vec<(&String, &crate::state::Thread)> = st.threads.iter().filter(|(_, t)| t.project == pid && !t.archived && !t.deleted).collect();
    ts.sort_by(|a, b| b.1.activity.cmp(&a.1.activity).then_with(|| a.0.cmp(b.0)));
    ts.into_iter().map(|(id, t)| { let s = st.get(&format!("summary.{id}"), ""); format!("- {}{}", t.title, if s.is_empty() { String::new() } else { format!(": {s}") }) }).take(12).collect()
}
/// Sum.project.input: what the writer reads about a project: its folder and newest threads
pub fn project_input(st: &State, pid: &str) -> String {
    let (title, root) = st.projects.get(pid).map_or((String::new(), String::new()), |p| (p.title.clone(), p.root.clone()));
    format!("Project: {title}\nFolder: {root}\n\nIts newest threads:\n{}", project_lines(st, pid).join("\n"))
}
/// Sum.clean: the writer's answer as a summary: its first line, spaces collapsed, at most 200 characters ("" for none)
pub fn clean(answer: &str) -> String {
    let first = bend_trim(answer).split('\n').next().unwrap_or("");
    let l = bend_trim(first).replace('\t', " ").split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    if l.chars().count() > 200 { format!("{}...", take(&l, 197)) } else { l }
}

/// A project summary to ask for next (Sum.next): its id and the writer's input
pub struct Next { pub project: String, pub input: String }
/// Sum.got for a finished thread summary: keep it (when not empty), and when the thread has a project that is due, stamp its clock and ask for
/// the project's summary. `st` is the state before (as Bend's `s`); events are committed together.
pub fn thread_done(st: &State, now: u64, id: &str, text: &str) -> (Vec<Value>, Option<Next>) {
    let mut ev = keep(id, text, now);
    let pid = st.threads.get(id).map_or("", |t| t.project.as_str()).to_string();
    if pid.is_empty() || !sum_due(st, &pid, now) { return (ev, None) }
    ev.push(json!({"$": "SettingSet", "key": format!("summary.at.{pid}"), "value": now.to_string(), "at": now}));
    let input = project_input(st, &pid);
    (ev, Some(Next { project: pid, input }))
}
/// Sum.got for a finished project summary
pub fn project_done(now: u64, id: &str, text: &str) -> Vec<Value> { keep(id, text, now) }
/// Sum.keep
fn keep(id: &str, text: &str, now: u64) -> Vec<Value> {
    let c = clean(text);
    if c.is_empty() { vec![] } else { vec![json!({"$": "SettingSet", "key": format!("summary.{id}"), "value": c, "at": now})] }
}
/// whether a summary must be asked for after this batch: the `summary.at.<id>` clocks the end wrote (ids of threads)
pub fn asked(events: &[Value]) -> Vec<String> {
    events.iter().filter(|e| gstr(e, "$", "") == "SettingSet").filter_map(|e| gstr(e, "key", "").strip_prefix("summary.at.").map(String::from)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_thread_input_reads_the_newest_user_message_and_reply() {
        let mut st = fold(&W);
        st.apply_line(r#"{"$":"MessagePosted","thread":"a","msg":"u1","role":"user","text":"fix the bug","at":30}"#);
        st.apply_line(r#"{"$":"MessagePosted","thread":"a","msg":"a1","role":"assistant","text":"fixed it","at":31}"#);
        assert_eq!(thread_input_of(&st, "a"), thread_input("Alpha", "Proj", "does alpha", "fix the bug", "fixed it"));
    }
    fn fold(lines: &[&str]) -> State { let mut s = State::new(); for l in lines { s.apply_line(l); } s }
    const W: [&str; 5] = [r#"{"$":"ProjectCreated","id":"p","title":"Proj","root":"/r","at":1}"#,
        r#"{"$":"ThreadCreated","id":"a","project":"p","title":"Alpha","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":10}"#,
        r#"{"$":"ThreadCreated","id":"b","project":"p","title":"Beta","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":20}"#,
        r#"{"$":"SettingSet","key":"summary.a","value":"does alpha","at":11}"#,
        r#"{"$":"ThreadArchived","id":"b","archived":false,"at":21}"#];
    #[test] fn summary_clean_is_one_cleaned_line_of_at_most_200() {
        assert_eq!(clean("  Fixes the \t login   flow.\nSecond line"), "Fixes the login flow."); assert_eq!(clean(""), ""); assert_eq!(clean(" \n x"), "x", "Bend trims the whole answer first, then takes its first line"); assert_eq!(clean(" \n \n"), "");
        let long = "w ".repeat(150); let c = clean(&long); assert_eq!(c.chars().count(), 200); assert!(c.ends_with("..."));
        assert_eq!(clean(&"a".repeat(200)).chars().count(), 200, "exactly 200 is not cut");
    }
    #[test] fn summary_inputs_have_bends_shape() {
        assert_eq!(thread_input("T", "P", "old", "hi", "yo"), "Thread title: T\nProject: P\nEarlier summary: old\n\nThe user's latest message:\nhi\n\nThe agent's latest reply:\nyo");
        let st = fold(&W);
        assert_eq!(project_input(&st, "p"), "Project: Proj\nFolder: /r\n\nIts newest threads:\n- Beta\n- Alpha: does alpha", "newest activity first; a summary after its title");
    }
    #[test] fn a_finished_thread_summary_is_kept_and_asks_for_the_project_when_due() {
        let st = fold(&W);
        let (ev, next) = thread_done(&st, 1000, "a", "Does alpha well.\nmore");
        assert_eq!(ev[0]["key"], "summary.a"); assert_eq!(ev[0]["value"], "Does alpha well.");
        assert_eq!(ev[1]["key"], "summary.at.p"); assert_eq!(next.unwrap().project, "p");
        let st = fold(&[W[0], W[1], r#"{"$":"SettingSet","key":"summary.at.p","value":"900","at":1}"#]);
        let (ev, next) = thread_done(&st, 1000, "a", "x"); assert_eq!(ev.len(), 1); assert!(next.is_none(), "the project was summarized 100 s ago");
        let (ev, _) = thread_done(&st, 1000, "a", "   "); assert!(ev.is_empty(), "an empty answer keeps nothing");
        assert!(thread_done(&fold(&W), 1000, "ghost", "x").1.is_none(), "a thread with no project asks for none");
        assert_eq!(project_done(5, "p", "A project.")[0]["key"], "summary.p");
    }
    #[test] fn the_summary_clocks_in_a_batch_name_the_threads_to_ask_for() {
        let evs = vec![json!({"$":"SettingSet","key":"summary.at.t1","value":"9","at":9}), json!({"$":"SettingSet","key":"alert.t1","value":"x","at":9})];
        assert_eq!(asked(&evs), vec!["t1".to_string()]);
    }
}

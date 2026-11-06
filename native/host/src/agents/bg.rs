//! A thread's background work as Claude reports it (model.bend's Bg.*, hub.bend's Bg.system / Sub.* / Prog.*):
//! rows "id<TAB>doing<TAB>call<TAB>title<TAB>kind<TAB>tokens" kept as a WorkSet "bg"; each task's own steps go to the log
//! "@sub:<thread>". Pure; `Cx::line` calls it.
pub const ENDED: &str = "\u{1}ended:";

pub fn clean(t: &str) -> String { t.replace('\t', " ").replace('\n', " ") }
pub fn line(id: &str, doing: &str) -> String { format!("{}\t{}", clean(id), clean(doing)) }
fn is(l: &str, id: &str) -> bool { l.starts_with(&format!("{}\t", clean(id))) }
pub fn has(ls: &[String], id: &str) -> bool { ls.iter().any(|l| is(l, id)) }
pub fn field(l: &str, i: usize) -> String { l.split('\t').nth(i).unwrap_or("").to_string() }
fn rest(l: &str) -> String { let r = l.split('\t').skip(2).collect::<Vec<_>>().join("\t"); if r.is_empty() { r } else { format!("\t{r}") } }
pub fn doing(l: &str) -> String { field(l, 1) }
pub fn ended(l: &str) -> bool { doing(l).starts_with(ENDED) }
pub fn lines(text: &str) -> Vec<String> { text.split('\n').filter(|l| !l.is_empty()).map(str::to_string).collect() }
pub fn show(ls: &[String]) -> String { ls.join("\n") }
pub fn log(th: &str) -> String { format!("@sub:{th}") }
/// Bg.kind: the word for a task type.
pub fn kind(tt: &str) -> &'static str { match tt { "local_agent" | "remote_agent" | "in_process_teammate" => "subagent", "local_bash" => "shell", "local_workflow" => "workflow", t if t.starts_with("monitor_") => "monitor", _ => "task" } }
/// Bg.add: a task started (listed once, last)
pub fn add(ls: &[String], id: &str, doing: &str) -> Vec<String> { let mut v = ls.to_vec(); if !has(ls, id) { v.push(line(id, doing)) } v }
/// Bg.set: what task id does now (a task not listed stays out)
pub fn set(ls: &[String], id: &str, doing: &str) -> Vec<String> { ls.iter().map(|l| if is(l, id) { format!("{}{}", line(id, doing), rest(l)) } else { l.clone() }).collect() }
/// Bg.start: listed once, last, with its call, title and kind.
pub fn start(ls: &[String], id: &str, doing: &str, call: &str, title: &str, kind: &str) -> Vec<String> {
    let mut v = ls.to_vec(); if !has(ls, id) { v.push(format!("{}\t{}\t{}\t{}", line(id, doing), clean(call), clean(title), clean(kind))) } v
}
pub fn drop(ls: &[String], id: &str) -> Vec<String> { ls.iter().filter(|l| !is(l, id)).cloned().collect() }
pub fn end(ls: &[String], id: &str, status: &str) -> Vec<String> { ls.iter().map(|l| if is(l, id) && !ended(l) { format!("{}{}", line(id, &format!("{ENDED}{status}")), rest(l)) } else { l.clone() }).collect() }
pub fn progress(ls: &[String], id: &str, doing: &str) -> Vec<String> { ls.iter().map(|l| if is(l, id) && !ended(l) { format!("{}{}", line(id, doing), rest(l)) } else { l.clone() }).collect() }
pub fn used(ls: &[String], id: &str, tokens: &str) -> Vec<String> {
    ls.iter().map(|l| if is(l, id) && !ended(l) { format!("{}\t{}\t{}\t{}\t{}\t{tokens}", field(l, 0), field(l, 1), field(l, 2), field(l, 3), field(l, 4)) } else { l.clone() }).collect()
}
pub fn unended(ls: &[String]) -> Vec<String> { ls.iter().filter(|l| !ended(l)).cloned().collect() }
/// Bg.sync: Claude's list of open tasks; a row at work that is not on it has ended.
pub fn sync(ls: &[String], open: &[String]) -> Vec<String> {
    ls.iter().map(|l| if !ended(l) && !open.iter().any(|x| clean(x) == clean(&field(l, 0))) { format!("{}{}", line(&field(l, 0), &format!("{ENDED}done")), rest(l)) } else { l.clone() }).collect()
}
/// Bg.running: rows at work.
pub fn running(ls: &[String]) -> usize { ls.iter().filter(|l| !ended(l)).count() }

/// Bg.listed: a task the agent left running (not one a subagent runs for itself, not a foreground shell).
pub fn listed(o: &serde_json::Value) -> bool {
    let tt = o.get("task_type").and_then(|v| v.as_str()).unwrap_or("");
    ["local_agent", "local_bash", "local_workflow", "remote_agent", "monitor_mcp", "monitor_ws", "in_process_teammate"].contains(&tt)
        && !o.get("owned_by_subagent").and_then(|v| v.as_bool()).unwrap_or(false) && o.get("is_backgrounded").and_then(|v| v.as_bool()).unwrap_or(true)
}

/// Lost.why / Lost.row / Lost.text / Lost.new / Lost.unknown (hub.bend): the work lost when the process that owned it goes.
pub fn lost_why(why: &str) -> String { match why { "restart" => "A restart of Backplane".into(), "exit" => "The agent process exiting".into(), "replaced" => "Restarting the agent for a new model or mode".into(), w => w.to_string() } }
/// Work.row.title: a row's title, else what it does
pub fn work_title(l: &str) -> String { let t = field(l, 3); if t.is_empty() { doing(l) } else { t } }
pub fn lost_text(ls: &[String], why: &str) -> String {
    let rows: String = ls.iter().map(|l| format!("\n- {} {}: {}{}", { let k = field(l, 4); if k.is_empty() { "task".to_string() } else { k } }, field(l, 0), work_title(l), if ended(l) { format!(" (it had ended: {}, its outcome not yet handed to you)", doing(l).strip_prefix(ENDED).unwrap_or("")) } else { String::new() })).collect();
    format!("[Backplane] {} ended your background work. Ownership was lost; the outcome is unknown.{rows}\nCheck what finished before rerunning.", lost_why(why))
}
/// a row not already ended "unknown"
pub fn lost_new(ls: &[String]) -> bool { ls.iter().any(|l| doing(l).strip_prefix(ENDED) != Some("unknown")) }
pub fn unknown(ls: &[String]) -> Vec<String> { ls.iter().map(|l| format!("{}{}", line(&field(l, 0), &format!("{ENDED}unknown")), rest(l))).collect() }

/// Prog: progress lines are written at most once every GAP seconds; a line held lands on the beat once the gap is over,
/// and the newest of each task always lands (laws bg_progress_coalesced, bg_progress_latest_lands).
pub const GAP: u64 = 5;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prog { pub at: u64, pub pend: Vec<serde_json::Value> }
pub struct PStep { pub at: u64, pub pend: Vec<serde_json::Value>, pub flush: Vec<serde_json::Value> }
fn task(v: &serde_json::Value) -> &str { v.get("task_id").and_then(|x| x.as_str()).unwrap_or("") }
impl Prog {
    /// Prog.line: a progress line heard at `now`: let through with every held one when the gap since the last write is over, else held.
    pub fn line(&self, now: u64, o: serde_json::Value) -> PStep {
        let mut pend = self.pend.clone(); match pend.iter().position(|p| task(p) == task(&o)) { Some(i) => pend[i] = o, None => pend.push(o) }
        if now >= self.at + GAP { PStep { at: now, pend: vec![], flush: pend } } else { PStep { at: self.at, pend, flush: vec![] } }
    }
    /// Prog.beat
    pub fn beat(&self, now: u64) -> PStep { if now >= self.at + GAP && !self.pend.is_empty() { PStep { at: now, pend: vec![], flush: self.pend.clone() } } else { PStep { at: self.at, pend: self.pend.clone(), flush: vec![] } } }
}
/// Bg.progress.o: a task_progress line on the rows (what it does now, and its tokens when Claude says).
pub fn progress_line(rows: &[String], o: &serde_json::Value) -> Vec<String> {
    let id = task(o); let r = progress(rows, id, o.get("description").and_then(|v| v.as_str()).unwrap_or(""));
    let tokens = o.get("usage").and_then(|u| u.get("total_tokens")).and_then(|v| v.as_u64()).unwrap_or(0);
    if tokens == 0 { r } else { used(&r, id, &tokens.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn rows() -> Vec<String> { start(&start(&[], "a", "Survey", "c1", "Survey", "subagent"), "b", "watch", "", "watch", "shell") }
    #[test] fn a_task_is_listed_once_with_its_call_title_and_kind() { let r = rows(); assert_eq!(r, ["a\tSurvey\tc1\tSurvey\tsubagent", "b\twatch\t\twatch\tshell"]); assert_eq!(start(&r, "a", "x", "y", "z", "k"), r); }
    #[test] fn rows_have_no_tab_or_newline_in_their_fields() { assert_eq!(line("a\tb", "x\ny"), "a b\tx y"); }
    #[test] fn a_row_ends_once_and_keeps_its_fields() { let r = end(&rows(), "a", "completed"); assert_eq!(r[0], format!("a\t{ENDED}completed\tc1\tSurvey\tsubagent")); assert_eq!(end(&r, "a", "failed")[0], r[0]); assert!(ended(&r[0]) && !ended(&r[1])); assert_eq!(running(&r), 1); assert_eq!(unended(&r), [r[1].clone()]); }
    #[test] fn progress_and_tokens_touch_only_rows_at_work() { let r = end(&rows(), "a", "completed"); assert_eq!(progress(&r, "a", "x"), r); let p = progress(&r, "b", "reading"); assert_eq!(p[1], "b\treading\t\twatch\tshell"); assert_eq!(used(&p, "b", "900")[1], "b\treading\t\twatch\tshell\t900"); }
    #[test] fn claudes_open_list_ends_what_is_not_on_it() { let r = sync(&rows(), &["b".to_string()]); assert!(ended(&r[0]) && !ended(&r[1])); assert_eq!(doing(&r[0]), format!("{ENDED}done")); assert_eq!(sync(&r, &["b".to_string()]), r); }
    #[test] fn task_kinds_have_words() { assert_eq!((kind("local_agent"), kind("local_bash"), kind("local_workflow"), kind("monitor_ws"), kind("zzz")), ("subagent", "shell", "workflow", "monitor", "task")); }
    #[test] fn only_tasks_the_agent_left_running_are_listed() {
        assert!(listed(&json!({"task_type":"local_agent"}))); assert!(!listed(&json!({"task_type":"local_agent","owned_by_subagent":true}))); assert!(!listed(&json!({"task_type":"local_bash","is_backgrounded":false}))); assert!(!listed(&json!({"task_type":"other"})));
    }
    #[test] fn lost_work_is_told_once_with_each_task() {
        let r = end(&rows(), "a", "completed"); let t = lost_text(&r, "exit");
        assert_eq!(t, "[Backplane] The agent process exiting ended your background work. Ownership was lost; the outcome is unknown.\n- subagent a: Survey (it had ended: completed, its outcome not yet handed to you)\n- shell b: watch\nCheck what finished before rerunning.");
        assert!(lost_new(&r)); assert!(!lost_new(&unknown(&r))); assert!(lost_text(&[], "restart").starts_with("[Backplane] A restart of Backplane ended"));
    }
    // law bg_progress_coalesced: lines inside the gap are held, one write lets them all through
    #[test] fn progress_lines_wait_out_the_gap() {
        let p = Prog::default(); let l = |d: &str| json!({"task_id":"a","description":d});
        let s = p.line(100, l("one")); assert_eq!(s.flush.len(), 1); let p = Prog { at: s.at, pend: s.pend };
        let s = p.line(101, l("two")); assert!(s.flush.is_empty()); let p = Prog { at: s.at, pend: s.pend }; let s = p.line(102, l("three")); assert!(s.flush.is_empty());
        let p = Prog { at: s.at, pend: s.pend }; assert!(p.beat(103).flush.is_empty()); let b = p.beat(105); assert_eq!(b.flush, [l("three")]); assert!(b.pend.is_empty()); // bg_progress_latest_lands: the newest lands
    }
    #[test] fn a_progress_line_names_what_the_task_does_and_its_tokens() { let r = progress_line(&rows(), &json!({"task_id":"b","description":"building","usage":{"total_tokens":42}})); assert_eq!(r[1], "b\tbuilding\t\twatch\tshell\t42"); }
}

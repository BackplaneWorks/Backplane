//! The thread's top panels as the native client draws them (client `Todos.*`, subs `Subs.*`; build's phones take the same rows through `Mob.todos`
//! and `Mob.subs`): the agent's to-do list ("To-do 3/7") and its background work and subagents ("2 at work"). Pure over the model's settings: `todos.<t>` (the list),
//! `todos.visible.<t>` and `bg.<t>` (rows "id TAB doing TAB call TAB title TAB kind TAB tokens"; `doing` starting with the ended mark is a finished one).
//! Whether a panel is open is the client's own (kept per thread on the device); the rows are always sent.
use crate::agents::{bg, lockrec::{age, Lock}};
use crate::state::State;
use serde_json::{json, Value};

fn set<'a>(st: &'a State, k: String) -> &'a str { st.settings.get(&k).map(String::as_str).unwrap_or("") }

/// `{count: "3/7", head: "To-do 3/7", now: "the step in progress", rows: [{text, status}]}`, or null while the list is empty or hidden (a new turn of the agent hides a finished list: `todos.visible.<t>` is "0").
pub fn todos(st: &State, thread: &str) -> Value {
    let list: Value = serde_json::from_str(set(st, format!("todos.{thread}"))).unwrap_or(Value::Null);
    let items: Vec<&Value> = list.as_array().into_iter().flatten().collect();
    if items.is_empty() || set(st, format!("todos.visible.{thread}")) == "0" { return Value::Null }
    let status = |x: &Value| match x["status"].as_str().unwrap_or("") { "completed" => "completed", "in_progress" => "in_progress", _ => "pending" };
    let done = items.iter().filter(|x| status(x) == "completed").count();
    let now = items.iter().find(|x| status(x) == "in_progress").map(|x| { let f = x["form"].as_str().unwrap_or(""); if f.is_empty() { x["text"].as_str().unwrap_or("") } else { f } }).unwrap_or("");
    let count = format!("{}/{}", done, items.len());
    json!({"count": count, "head": format!("To-do {count}"), "now": now, "rows": items.iter().map(|x| json!({"id": x["id"].as_str().unwrap_or(""), "text": x["text"].as_str().unwrap_or(""), "status": status(x)})).collect::<Vec<_>>()})
}

/// A row of a thread's work the person may act on: `act` is what its button sends, with `value`; `label` the button's word.
fn acting(title: String, kind: &str, state: &str, doing: String, act: &str, value: &str, label: &str) -> Value { json!({"title": title, "kind": kind, "state": state, "doing": doing, "act": act, "value": value, "label": label}) }
/// Subs.watch.row: a job or a watch Backplane owns for the thread. Stop while it is at work and can still be stopped; a queued job's Stop takes it out of the queue.
fn watch_rows(st: &State, thread: &str) -> Vec<Value> {
    bg::lines(set(st, format!("watch.{thread}"))).iter().map(|l| {
        let ended = bg::ended(l); let doing = bg::doing(l);
        let word = if ended { doing.trim_start_matches(bg::ENDED).to_string() } else { doing };
        let stop = !ended && word != "cancelling" && word != "unknown";
        acting(format!("watch: {}", bg::work_title(l)), "watch", if ended { "ended" } else { "working" }, word, if stop { "work-cancel" } else { "" }, &bg::field(l, 0), if stop { "Stop" } else { "" })
    }).collect()
}
/// Who holds a lock, in words: its thread's title (a job's: "a job in <title>"), a shell helper's note, else its id.
fn holder_name(st: &State, l: &Lock) -> String {
    let Some(h) = &l.holder else { return String::new() };
    let title = st.threads.get(&h.thread).map(|t| t.title.clone()).filter(|t| !t.is_empty());
    match (h.kind.as_str(), title) { ("work", Some(t)) => format!("a job in {t}"), (_, Some(t)) => t, _ if !h.note.is_empty() => h.note.clone(), _ => h.id.clone() }
}
/// Subs.lock.row: a thread's own place on each lock, held (Release) or queued (Cancel). A job's place shows on its watch row.
fn lock_rows(st: &State, thread: &str) -> Vec<Value> {
    bg::lines(set(st, "locks.names".into())).iter().filter_map(|name| {
        let l = Lock::read(set(st, format!("lock.{name}"))); if !l.has(thread) { return None }
        let held = l.owner() == thread && l.ready;
        let doing = if held { "Release when done".to_string() } else if l.owner() == thread { "waiting for a flock outside Backplane".to_string() } else { format!("position {} · held by {}", l.position(thread), holder_name(st, &l)) };
        Some(acting(format!("lock: {name}"), "lock", if held { "held" } else { "waiting" }, doing, if l.owner() == thread { "lock-release" } else { "lock-cancel" }, name, if l.owner() == thread { "Release" } else { "Cancel" }))
    }).collect()
}
/// Subs.window: how many entries of a thread's subagent log (`@sub:<thread>`) are read, newest first. Subs.shown: the steps an open row lists.
pub const SUB_WINDOW: usize = 600;
const SHOWN: usize = 6;
fn first_line(t: &str, n: usize) -> String { let l = t.trim().split('\n').next().unwrap_or("").trim(); if l.chars().count() > n { format!("{}…", l.chars().take(n).collect::<String>()) } else { l.to_string() } }
/// Subs.walk for every call at once: a thread's subagent log, its rows `(call, kind, text, at)` newest first (lines.rs's Sub.about, Sub.blocks,
/// Bg.mark and Bg.outcome write them), as each call's `{type, model, ask, since, until, steps, tokens, outcome, last, shown}`: what it was asked, its
/// type and model, when it started and ended, how many steps it took, its newest step and newest few (oldest first). Nothing guessed: a field the log has not got is empty.
pub fn sub_log(rows: &[(String, String, String, u64)]) -> Value {
    let mut m = serde_json::Map::new();
    for (call, kind, text, at) in rows {
        if call.is_empty() { continue }
        let o = m.entry(call.clone()).or_insert_with(|| json!({"type": "", "model": "", "ask": "", "since": 0, "until": 0, "steps": 0, "tokens": 0, "outcome": "", "last": null, "shown": [], "title": "", "kind": "", "end": ""}));
        // newest first: the oldest start stays, with what it was called and its kind ("start:subagent")
        if kind.starts_with("start") { o["since"] = json!(at); o["title"] = json!(first_line(text, 160)); o["kind"] = json!(kind.strip_prefix("start:").unwrap_or("")); continue }
        match kind.as_str() {
            "end" => { if o["until"] == 0 { o["until"] = json!(at); o["end"] = json!(text.trim()) } }
            "about" => { if o["ask"] == "" { let a: Value = serde_json::from_str(text).unwrap_or(Value::Null);
                o["type"] = json!(a["type"].as_str().unwrap_or("")); o["model"] = json!(a["model"].as_str().unwrap_or("")); o["ask"] = json!(a["prompt"].as_str().unwrap_or("").trim()); } }
            "outcome" => { if o["outcome"] == "" { let a: Value = serde_json::from_str(text).unwrap_or(Value::Null);
                o["outcome"] = json!(a["summary"].as_str().unwrap_or("").trim()); o["tokens"] = json!(a["usage"]["total_tokens"].as_u64().unwrap_or(0)); } }
            _ => {
                let n = o["steps"].as_u64().unwrap_or(0); o["steps"] = json!(n + 1);
                if o["since"] == 0 || o["since"].as_u64().unwrap_or(0) > *at { o["since"] = json!(at) }
                let step = json!({"kind": if kind == "say" { "Said".to_string() } else { super::thread::shown_name(kind) }, "text": first_line(text, 200)});
                if n == 0 { o["last"] = step.clone() }
                if (n as usize) < SHOWN { if let Some(a) = o["shown"].as_array_mut() { a.insert(0, step) } }
            }
        }
    }
    Value::Object(m)
}
/// Subs.dur / Subs.took: how long it has been at work ("<1m", "12m", "1h 4m") or took ("45s", "4m"); "" with no start known.
fn took(live: bool, since: u64, until: u64, now: u64) -> String {
    let d = |s: u64| if s < 60 { format!("{s}s") } else if s < 3600 { format!("{}m", s / 60) } else if s < 86400 { let m = (s / 60) % 60; format!("{}h{}", s / 3600, if m == 0 { String::new() } else { format!(" {m}m") }) } else { format!("{}d", s / 86400) };
    if since == 0 { return String::new() }
    if live { if now <= since { return String::new() } let s = now - since; return if s < 60 { "<1m".into() } else { d(s) } }
    if until < since { String::new() } else { d(until - since) }
}
/// Subs.tokens: "812 tokens", "18.2k tokens", "1.4M tokens"
fn tokens_word(n: u64) -> String {
    let t = |w: u64, f: u64| if f == 0 { w.to_string() } else { format!("{w}.{f}") };
    if n == 0 { String::new() } else if n < 1000 { format!("{n} tokens") } else if n < 1_000_000 { format!("{}k tokens", t(n / 1000, (n % 1000) / 100)) } else { format!("{}M tokens", t(n / 1_000_000, (n % 1_000_000) / 100_000)) }
}
/// What a subagent row says beside its name (Subs.info): the harness and its type, the model, its steps and tokens; how long; its newest step;
/// and, for the open row, where it runs, what it was asked, its newest steps and what it reported. From its log entry `info` (sub_log) and its thread.
fn sub_info(st: &State, thread: &str, live: bool, line: &str, info: &Value, host: &str, now: u64) -> serde_json::Map<String, Value> {
    let t = st.threads.get(thread);
    let provider = t.map_or("claude", |t| if t.provider.is_empty() { "claude" } else { t.provider.as_str() });
    let harness = super::keys::AGENTS.iter().find(|a| a.0 == provider).map_or(provider, |a| a.1);
    let ty = info["type"].as_str().unwrap_or("");
    let model = { let m = info["model"].as_str().unwrap_or(""); let m = if m.is_empty() || m == "inherit" { t.map_or("", |t| t.model.as_str()) } else { m }; if m.is_empty() { String::new() } else { super::keys::model_short(m) } };
    let steps = info["steps"].as_u64().unwrap_or(0);
    let tok = { let live_t: u64 = bg::field(line, 5).trim().parse().unwrap_or(0); let o = info["tokens"].as_u64().unwrap_or(0); if o > 0 { o } else { live_t } };
    let parts: Vec<String> = [format!("{harness}{}", if ty.is_empty() { String::new() } else { format!(" · {ty}") }), model.clone(),
        if steps == 0 { String::new() } else { format!("{steps} step{}", if steps == 1 { "" } else { "s" }) }, tokens_word(tok)].into_iter().filter(|p| !p.is_empty()).collect();
    let mut o = serde_json::Map::new();
    o.insert("meta".into(), json!(parts.join(" · ")));
    o.insert("took".into(), json!(took(live, info["since"].as_u64().unwrap_or(0), info["until"].as_u64().unwrap_or(0), now)));
    let last = &info["last"];
    o.insert("last".into(), json!(if last.is_object() { format!("{} {}", last["kind"].as_str().unwrap_or(""), last["text"].as_str().unwrap_or("")).trim().to_string() } else { String::new() }));
    // where: the harness's subagent inside this thread's agent process, so on this thread's machine, in its worktree when it has one
    let wt = t.map_or("", |t| t.branch.as_str());
    let place = format!("{harness} subagent{} inside this thread's agent{}{}", if ty.is_empty() { String::new() } else { format!(" ({ty})") },
        if host.is_empty() { String::new() } else { format!(", on {host}") }, if wt.is_empty() { String::new() } else { format!(", worktree {wt}") });
    let ask = info["ask"].as_str().unwrap_or("");
    let ask = if ask.chars().count() > 600 { format!("{}…", ask.chars().take(600).collect::<String>()) } else { ask.to_string() };
    o.insert("detail".into(), json!({"where": place, "model": model, "ask": ask, "steps": info["shown"].clone(), "outcome": info["outcome"].clone(), "more": steps.saturating_sub(SHOWN as u64)}));
    o
}
/// `{busy: "2 at work" | "", word: "2 at work" | "3 done", rows: [{title, kind, state, doing, act?, value?, label?}]}`, or null with nothing to show. A row is a subagent, a background shell, a monitor or a workflow the agent left running, a thread's lock, or work Backplane watches for it.
pub fn subs(st: &State, thread: &str) -> Value { subs_at(st, thread, &Value::Null, 0) }
/// `subs` with what the thread's subagent log says (`log`: `{host, calls: sub_log}`; null when it was not read): a subagent's row gets its `key` (the call
/// that started it, for the client's open/shut choice), `meta`, `took`, `last` and its `detail` (the drill-down); `now` in seconds.
pub fn subs_at(st: &State, thread: &str, log: &Value, now: u64) -> Value {
    let text = set(st, format!("bg.{thread}"));
    let lines = bg::lines(text);
    let own: Vec<Value> = lock_rows(st, thread).into_iter().chain(watch_rows(st, thread)).collect();
    if lines.is_empty() && own.is_empty() && !log["calls"].as_object().map_or(false, |c| !c.is_empty()) { return Value::Null }
    let live = bg::running(&lines) + own.iter().filter(|r| r["state"] != "ended").count();
    let host = log["host"].as_str().unwrap_or("");
    let rows: Vec<Value> = lines.iter().map(|l| {
        let ended = bg::ended(l); let doing = bg::doing(l);
        let title = { let t = bg::field(l, 3); if t.is_empty() { bg::field(l, 0) } else { t } };
        let kind = { let k = bg::field(l, 4); if k.is_empty() { "subagent".to_string() } else { k } };
        let state = if !ended { "working".to_string() } else { doing.trim_start_matches(bg::ENDED).to_string() };
        let mut r = json!({"title": title, "kind": kind, "state": state, "doing": if ended || doing == title { String::new() } else { doing }});
        // a task at work can be stopped alone (agents' stop_task), the turn and the other tasks untouched
        if !ended { r["act"] = json!("task-stop"); r["value"] = json!(bg::field(l, 0)); r["label"] = json!("Stop"); }
        let call = bg::field(l, 2);
        if kind == "subagent" && log.is_object() {
            r["key"] = json!(if call.is_empty() { bg::field(l, 0) } else { call.clone() });
            for (k, v) in sub_info(st, thread, !ended, l, &log["calls"][call.as_str()], host, now) { r[k] = v; }
        }
        r
    }).chain(own).collect();
    // Subs.done: a subagent of the agent's own that ended in the last day stays listed after its turn (Bg.settle took its line), folded under the head
    // with what it reported, so what the subagents did can still be read; one whose start the log window no longer holds is left out
    let mut rows = rows;
    if let Some(calls) = log["calls"].as_object() {
        let listed: Vec<String> = lines.iter().map(|l| bg::field(l, 2)).collect();
        let mut done: Vec<(&String, &Value)> = calls.iter().filter(|(c, i)| !listed.contains(c) && i["kind"] == "subagent" && i["since"].as_u64().unwrap_or(0) > 0
            && i["until"].as_u64().map_or(false, |u| u > 0 && now.saturating_sub(u) < 86400)).collect();
        done.sort_by_key(|(_, i)| std::cmp::Reverse(i["until"].as_u64().unwrap_or(0)));
        for (call, i) in done {
            let end = i["end"].as_str().unwrap_or(""); let state = if end.is_empty() { "completed" } else { end };
            let title = i["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("Subagent");
            let mut r = json!({"title": title, "kind": "subagent", "state": state, "doing": "", "key": call});
            for (k, v) in sub_info(st, thread, false, "", i, host, now) { r[k] = v; }
            rows.push(r);
        }
    }
    if rows.is_empty() { return Value::Null }
    let word = if live > 0 { format!("{live} at work") } else { format!("{} done", rows.len()) };
    json!({"busy": if live > 0 { format!("{live} at work") } else { String::new() }, "word": word, "live": live, "rows": rows})
}
/// Plane.lock.row for the clients: every lock with a holder or a queue, its holder's title, how long held, who waits. Clients draw these as they come.
pub fn locks(st: &State, now: u64, title: impl Fn(&str) -> String) -> Vec<Value> {
    bg::lines(set(st, "locks.names".into())).iter().filter_map(|name| {
        let l = Lock::read(set(st, format!("lock.{name}"))); if l.idle() { return None }
        let h = l.holder.clone().unwrap_or_default();
        let who = if h.thread.is_empty() { h.note.clone() } else { title(&h.thread) };
        let held = if let Some(o) = &l.outside { format!("held outside Backplane by pid {}", o["pid"]) } else if l.ready { format!("{who} · {}", age(now, h.since)) } else { format!("{who} · starting") };
        Some(json!({"name": name, "holder": who, "thread": h.thread, "line": held, "waiting": l.waiters.len()}))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn st(kv: &[(&str, &str)]) -> State { let mut s = State::new(); for (k, v) in kv { s.settings.insert(k.to_string(), v.to_string()); } s }
    #[test] fn a_todo_list_counts_its_done_steps_names_the_one_in_progress_and_hides_while_empty_or_hidden() {
        let list = r#"[{"id":"1","text":"Survey","form":"Surveying","status":"completed"},{"id":"2","text":"Route","form":"Routing","status":"in_progress"},{"id":"3","text":"DRC","form":"","status":"weird"}]"#;
        let v = todos(&st(&[("todos.t1", list), ("todos.visible.t1", "1")]), "t1");
        assert_eq!((v["count"].as_str(), v["head"].as_str(), v["now"].as_str()), (Some("1/3"), Some("To-do 1/3"), Some("Routing")));
        assert_eq!(v["rows"][2]["status"], "pending", "an unknown status is pending");
        assert!(todos(&st(&[("todos.t1", list), ("todos.visible.t1", "0")]), "t1").is_null());
        assert!(todos(&st(&[]), "t1").is_null() && todos(&st(&[("todos.t1", "[]")]), "t1").is_null());
    }
    #[test] fn background_work_counts_who_is_at_work_and_says_how_the_rest_ended() {
        let rows = format!("a1\tReading footprint 3\tc1\tSurvey the footprints\tsubagent\n{}\tc2\tnpm run watch\tshell", bg::line("b1", &format!("{}completed", bg::ENDED)));
        let v = subs(&st(&[("bg.t1", &rows)]), "t1");
        assert_eq!((v["busy"].as_str(), v["live"].as_u64()), (Some("1 at work"), Some(1)));
        assert_eq!((v["rows"][0]["title"].as_str(), v["rows"][0]["state"].as_str(), v["rows"][0]["doing"].as_str()), (Some("Survey the footprints"), Some("working"), Some("Reading footprint 3")));
        assert_eq!((v["rows"][1]["state"].as_str(), v["rows"][1]["kind"].as_str()), (Some("completed"), Some("shell")));
        let done = subs(&st(&[("bg.t1", &bg::line("b1", &format!("{}completed", bg::ENDED)))]), "t1"); assert_eq!((done["busy"].as_str(), done["word"].as_str()), (Some(""), Some("1 done")));
        assert!(subs(&st(&[]), "t1").is_null());
    }
    // 11-02: a subagent's row says which harness and type, its model, steps, tokens and how long; its drill-down where it runs, what it was asked, its newest steps
    #[test] fn a_subagents_row_says_what_it_is_where_it_runs_and_what_it_did() {
        let r = |c: &str, k: &str, t: &str, at: u64| (c.to_string(), k.to_string(), t.to_string(), at);
        let mut log = vec![r("c1", "about", r#"{"type":"Explore","model":"","prompt":"Find every footprint without a 3D model."}"#, 100), r("c1", "start:subagent", "Survey", 100)];
        for i in 0..8 { log.push(r("c1", if i == 7 { "say" } else { "Read" }, &format!("/p/f{i}.kicad_mod\nmore"), 110 + i)); }
        log.reverse(); // newest first, as the log is read
        let calls = sub_log(&log);
        assert_eq!((calls["c1"]["steps"].as_u64(), calls["c1"]["since"].as_u64(), calls["c1"]["type"].as_str()), (Some(8), Some(100), Some("Explore")));
        let shown = calls["c1"]["shown"].as_array().unwrap(); assert_eq!(shown.len(), 6); assert_eq!((shown[5]["kind"].as_str(), shown[5]["text"].as_str()), (Some("Said"), Some("/p/f7.kicad_mod")));
        let mut s = st(&[("bg.t1", "a1\tReading footprint 3\tc1\tSurvey the footprints\tsubagent\t18234")]);
        { let t = s.threads.entry("t1".into()).or_insert_with(crate::state::Thread::blank); t.provider = "claude".into(); t.model = "claude-opus-4-6".into(); t.branch = "bp/fix".into(); }
        let v = subs_at(&s, "t1", &json!({"host": "desk", "calls": calls}), 400);
        let row = &v["rows"][0];
        assert_eq!((row["key"].as_str(), row["title"].as_str(), row["doing"].as_str()), (Some("c1"), Some("Survey the footprints"), Some("Reading footprint 3")));
        assert_eq!(row["meta"], "Claude Code · Explore · Opus 4.6 · 8 steps · 18.2k tokens");
        assert_eq!((row["took"].as_str(), row["last"].as_str()), (Some("5m"), Some("Said /p/f7.kicad_mod")));
        assert_eq!((row["act"].as_str(), row["value"].as_str(), row["label"].as_str()), (Some("task-stop"), Some("a1"), Some("Stop")), "a subagent at work can be stopped alone");
        assert_eq!(row["detail"]["where"], "Claude Code subagent (Explore) inside this thread's agent, on desk, worktree bp/fix");
        assert_eq!((row["detail"]["ask"].as_str(), row["detail"]["more"].as_u64()), (Some("Find every footprint without a 3D model."), Some(2)));
        assert!(subs(&s, "t1")["rows"][0].get("detail").is_none(), "without its log a row is as before");
        // its turn over, its line gone (Bg.settle): it stays listed for a day with how it ended and what it reported; then it is gone
        let mut log2 = vec![r("c1", "outcome", r#"{"summary":"Two footprints lack models: TUBE, XFMR.","usage":{"total_tokens":20500}}"#, 130), r("c1", "end", "completed", 130)]; log2.extend(log.clone());
        let calls2 = sub_log(&log2); s.settings.insert("bg.t1".into(), String::new());
        let v = subs_at(&s, "t1", &json!({"host": "desk", "calls": calls2}), 400);
        assert_eq!((v["word"].as_str(), v["live"].as_u64(), v["rows"][0]["state"].as_str(), v["rows"][0]["took"].as_str()), (Some("1 done"), Some(0), Some("completed"), Some("30s")));
        assert_eq!((v["rows"][0]["title"].as_str(), v["rows"][0]["detail"]["outcome"].as_str()), (Some("Survey"), Some("Two footprints lack models: TUBE, XFMR.")));
        assert!(subs_at(&s, "t1", &json!({"host": "desk", "calls": calls2}), 130 + 86400).is_null(), "a day later it is gone");
    }
    #[test] fn a_waiter_reads_who_holds_by_name() {
        let lock = |h: &str| format!(r#"{{"holder":{h},"waiters":[{{"id":"t2","kind":"thread","thread":"t2","note":"","since":1,"lease":0}}],"ready":true}}"#);
        let mut s = st(&[("locks.names", "a"), ("lock.a", &lock(r#"{"id":"t1","kind":"thread","thread":"t1","note":"","since":1,"lease":0}"#))]);
        s.threads.entry("t1".into()).or_insert_with(crate::state::Thread::blank).title = "Board work".into();
        assert_eq!(subs(&s, "t2")["rows"][0]["doing"], "position 1 · held by Board work");
        s.settings.insert("lock.a".into(), lock(r#"{"id":"w-1","kind":"work","thread":"t1","note":"","since":1,"lease":0}"#)); assert_eq!(subs(&s, "t2")["rows"][0]["doing"], "position 1 · held by a job in Board work");
        s.settings.insert("lock.a".into(), lock(r#"{"id":"pid:9","kind":"client","thread":"","note":"render","since":1,"lease":0}"#)); assert_eq!(subs(&s, "t2")["rows"][0]["doing"], "position 1 · held by render");
    }
    #[test] fn a_threads_locks_and_jobs_carry_what_their_buttons_send() {
        let held = r#"{"holder":{"id":"t1","kind":"thread","thread":"t1","note":"","since":1,"lease":0},"waiters":[{"id":"t2","kind":"thread","thread":"t2","note":"","since":2,"lease":0}],"ready":true}"#;
        let flock = r#"{"holder":{"id":"t1","kind":"thread","thread":"t1","note":"","since":1,"lease":0},"waiters":[],"ready":false,"outside":{"pid":7,"cmd":"flock x","since":1}}"#;
        let job = "w-1\twaiting for lock b (2nd)\t\tthe build\twatch\trun\t\\emake\t-\t0";
        let s1 = st(&[("locks.names", "a\nb"), ("lock.a", held), ("lock.b", flock), ("watch.t1", job)]);
        let v = subs(&s1, "t1"); let rows = v["rows"].as_array().unwrap();
        assert_eq!((rows[0]["act"].as_str(), rows[0]["value"].as_str(), rows[0]["label"].as_str(), rows[0]["state"].as_str()), (Some("lock-release"), Some("a"), Some("Release"), Some("held")));
        assert_eq!((rows[1]["act"].as_str(), rows[1]["state"].as_str()), (Some("lock-release"), Some("waiting")), "a holder held back by a flock can still let go");
        assert_eq!((rows[2]["act"].as_str(), rows[2]["value"].as_str(), rows[2]["label"].as_str(), rows[2]["doing"].as_str()), (Some("work-cancel"), Some("w-1"), Some("Stop"), Some("waiting for lock b (2nd)")));
        assert_eq!(v["live"], 3);
        let w = subs(&s1, "t2"); assert_eq!((w["rows"][0]["act"].as_str(), w["rows"][0]["label"].as_str(), w["rows"][0]["doing"].as_str()), (Some("lock-cancel"), Some("Cancel"), Some("position 1 · held by t1")));
        assert!(subs(&s1, "t3").is_null(), "another thread's lock is not its row");
        let ls = locks(&s1, 301, |t| format!("title of {t}"));
        assert_eq!((ls[0]["holder"].as_str(), ls[0]["line"].as_str(), ls[0]["waiting"].as_u64()), (Some("title of t1"), Some("title of t1 · 5m"), Some(1)));
        assert_eq!(ls[1]["line"], "held outside Backplane by pid 7");
    }
}

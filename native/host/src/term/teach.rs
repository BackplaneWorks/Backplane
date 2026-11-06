//! Teaching a bot's browser, the hub's side, pure. Per bot: who drives its page ("" the bot, "teach", "takeover")
//! and the teach state ("" nothing, "rec" recording, "review" after stop) with the draft steps. None of it is in the event log: a restart ends a
//! teach. It reaches clients as the info keys drive.<bot>, teach.<bot> and teach.draft.<bot>. Bounded: at most MAX_BOTS bots with a state,
//! MAX_STEPS draft steps, a recording lasts at most LIMIT seconds, the newest 64 pause alerts are remembered.
use super::task::{self, Step, Task, Value};
use serde_json::{json, Value as J};
use std::collections::HashMap;

pub const MAX_STEPS: usize = 400;
pub const MAX_BOTS: usize = 1024;
pub const LIMIT: u64 = 600;
/// The most bytes the draft takes in an info value (Hub::set_info allows 64 KiB).
pub const INFO_MAX: usize = 60_000;
pub const REFUSAL: &str = super::browser::REFUSAL;

#[derive(Clone, Debug, PartialEq)]
pub struct TeachBot { pub bot: String, pub drive: String, pub mode: String, pub start: u64, pub steps: Vec<Step> }
impl TeachBot {
    pub fn blank(bot: &str) -> TeachBot { TeachBot { bot: bot.into(), drive: String::new(), mode: String::new(), start: 0, steps: vec![] } }
    pub fn idle(&self) -> bool { self.drive.is_empty() && self.mode.is_empty() }
    pub fn recording(&self) -> bool { self.mode == "rec" }
    /// Teach.start: the person drives, recording, an empty draft.
    pub fn started(bot: &str, now: u64) -> TeachBot { TeachBot { bot: bot.into(), drive: "teach".into(), mode: "rec".into(), start: now, steps: vec![] } }
    /// Teach.add: a record event (or a note) joins the draft while recording, numbered from 1; a secret's step carries its label only.
    pub fn add(mut self, r: &J) -> TeachBot { if self.recording() && self.steps.len() < MAX_STEPS { let n = self.steps.len() as u64 + 1; self.steps.push(task::step_of_record(r, n)) } self }
    /// Teach.stop: recording off, the draft kept for review.
    pub fn stop(mut self) -> TeachBot { if self.recording() { self.mode = "review".into() } self }
    /// Teach.takeover: the person takes the browser (not while teaching: they have it).
    pub fn takeover(mut self) -> TeachBot { if self.drive.is_empty() { self.drive = "takeover".into() } self }
    /// Teach.handback: the bot drives again (a take over ends; a teach ends only by stop/save).
    pub fn handback(mut self) -> TeachBot { if self.drive == "takeover" { self.drive.clear() } self }
    pub fn expired(&self, now: u64) -> bool { self.recording() && now.saturating_sub(self.start) >= LIMIT }
    pub fn mode_text(&self) -> String { if self.recording() { format!("rec {} {}", self.start, self.steps.len()) } else if self.mode == "review" { "review".into() } else { String::new() } }
    pub fn draft_text(&self) -> String { self.draft_text_max(usize::MAX) }
    /// The draft as the info text; when it would pass `max` bytes the oldest steps are left out until it fits (the info value is bounded).
    pub fn draft_text_max(&self, max: usize) -> String {
        if self.mode.is_empty() { return String::new() }
        let mut from = 0; loop { let t = J::Array(self.steps[from..].iter().map(task::step_json).collect()).to_string(); if t.len() <= max || from >= self.steps.len() { return if t.len() <= max { t } else { "[]".into() } } from += 1 + (self.steps.len() - from) / 8 }
    }
    /// The three info keys of a bot (empty values when idle: clients clear them).
    pub fn kvs(&self) -> Vec<(String, String)> { vec![(format!("drive.{}", self.bot), self.drive.clone()), (format!("teach.{}", self.bot), self.mode_text()), (format!("teach.draft.{}", self.bot), self.draft_text_max(INFO_MAX))] }
    /// {"t":"info","info":{...}} for one bot, sent to every client when it changes.
    pub fn msg(&self) -> J { json!({"t":"info","info": self.kvs().into_iter().map(|(k, v)| (k, J::String(v))).collect::<serde_json::Map<_, _>>()}) }
    /// The newest draft step as JSON text ("" when none): a line of the draft file.
    pub fn last(&self) -> String { self.steps.last().map(|s| task::step_json(s).to_string()).unwrap_or_default() }
}
/// A typed note as a record.
pub fn note(text: &str) -> J { json!({"do":"note","value":text}) }

#[derive(Default)]
pub struct Teach { bots: HashMap<String, TeachBot>, alerted: Vec<String> }
impl Teach {
    pub fn get(&self, bot: &str) -> TeachBot { self.bots.get(bot).cloned().unwrap_or_else(|| TeachBot::blank(bot)) }
    /// The bot's state replaced; an idle bot leaves the map. False when the map is full and the bot is new (the caller refuses the teach).
    pub fn put(&mut self, b: TeachBot) -> bool {
        if b.idle() { self.bots.remove(&b.bot); return true }
        if !self.bots.contains_key(&b.bot) && self.bots.len() >= MAX_BOTS { return false }
        self.bots.insert(b.bot.clone(), b); true
    }
    /// Teach.bot_may: whether the bot may use its own browser now (preview_* calls).
    pub fn bot_may(&self, bot: &str) -> bool { task::can_bot(&self.get(bot).drive) }
    /// Every bot with something to say, for a joining client's info (sorted for a stable answer).
    pub fn info(&self) -> Vec<(String, String)> { let mut ids: Vec<&String> = self.bots.keys().collect(); ids.sort(); ids.into_iter().flat_map(|i| self.bots[i].kvs()).collect() }
    pub fn alerted(&self, key: &str) -> bool { self.alerted.iter().any(|k| k == key) }
    /// The key noted as alerted (the newest 64 kept).
    pub fn alert_mark(&mut self, key: &str) { self.alerted.insert(0, key.into()); self.alerted.truncate(64) }
    /// Bots whose recording has run out (Hub.tch.tick).
    pub fn expired(&self, now: u64) -> Vec<String> { self.bots.values().filter(|b| b.expired(now)).map(|b| b.bot.clone()).collect() }
}

// ---- saving a draft as a task ----
/// A parameter's name: letters, digits and _ (spaces and - become _).
pub fn pname(s: &str) -> String { s.chars().map(|c| if c == ' ' || c == '-' { '_' } else { c }).filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect() }
struct Mark { step: u64, name: String, label: String }
fn marks(xs: &[J]) -> Vec<Mark> { xs.iter().map(|j| Mark { step: j.get("step").and_then(J::as_u64).unwrap_or(0), name: pname(j.get("name").and_then(J::as_str).unwrap_or("").trim()), label: j.get("label").and_then(J::as_str).unwrap_or("").into() }).collect() }
fn urls(steps: &[Step]) -> Vec<String> { let mut o: Vec<String> = vec![]; for s in steps { if s.act == "navigate" && !s.url.is_empty() && !o.contains(&s.url) { o.push(s.url.clone()) } } o }
/// Teach.build: the draft with its marked values as parameters, as a task shown by the person. A secret never becomes a parameter.
pub fn build(b: &TeachBot, name: &str, description: &str, marks_json: &[J], now: u64) -> Task {
    let ms = marks(marks_json); let mut steps = b.steps.clone();
    for s in &mut steps { if let Value::Lit(_) = s.value { if let Some(m) = ms.iter().find(|m| m.step == s.n) { if !m.name.is_empty() { s.value = Value::Param(m.name.clone()) } } } }
    let mut params: Vec<task::Param> = vec![]; let mut seen: Vec<&str> = vec![];
    for m in &ms {
        if m.name.is_empty() || seen.contains(&m.name.as_str()) { continue }
        let Some(Value::Lit(ex)) = b.steps.iter().find(|s| s.n == m.step).map(|s| s.value.clone()) else { continue };
        seen.push(&m.name); params.push(task::Param { name: m.name.clone(), label: if m.label.is_empty() { m.name.clone() } else { m.label.clone() }, kind: "text".into(), example: ex });
    }
    let us = urls(&steps);
    Task { slug: task::slug(name), name: name.into(), description: description.into(), origin: "show".into(), start: us.first().cloned().unwrap_or_default(), params, steps, urls: us, created: now, updated: now }
}
/// The message that wakes the bot after a task was saved from a recording.
pub fn taught(name: &str, t: &Task) -> String {
    format!("[taught: {name}] The person just showed you this task in your browser. Learn it from the recording: read it with task_get, improve the description and notes, mark decision points, and save it with task_save. Steps: {}", t.steps.iter().map(line).collect::<Vec<_>>().join("; "))
}
pub fn run_msg(slug: &str, params: &str) -> String { format!("[task: {slug}] Run the task skill {slug} with {params}.") }
fn cut(s: &str, n: usize) -> String { if s.chars().count() > n { format!("{}...", s.chars().take(n).collect::<String>()) } else { s.into() } }
fn line(s: &Step) -> String {
    let what = [&s.target.name, &s.target.label, &s.target.placeholder, &s.target.text, &s.target.css, &s.url].into_iter().find(|x| !x.is_empty()).cloned().unwrap_or_default();
    let v = match &s.value { Value::Lit(t) => format!(" \"{}\"", cut(t, 40)), Value::Param(n) => format!(" {{{{{n}}}}}"), Value::Secret(l) => format!(" <secret: {l}>"), Value::None => String::new() };
    if s.act == "note" { format!("{} note {}", s.n, cut(&s.note, 80)) } else { format!("{} {} {}{}", s.n, s.act, cut(&what, 60), v) }
}
/// The `<tasks>` block in the bot's turn: its task skills (empty when none). `rows` are (slug, stored JSON).
pub fn block(rows: &[(String, String)]) -> String {
    let ls: Vec<String> = rows.iter().map(|(slug, js)| match task::read(js) {
        Some(t) => format!("- {slug}: {}{}{}", t.name, if t.description.is_empty() { String::new() } else { format!(" - {}", cut(&t.description, 120)) }, if t.params.is_empty() { String::new() } else { format!(" (params: {})", t.params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")) }),
        None => format!("- {slug}") }).collect();
    if ls.is_empty() { String::new() } else { format!("<tasks>\n{}\n</tasks>", ls.join("\n")) }
}

// ---- task_save ----
/// Teach.save: merge `a` onto the stored task (`existing`, its JSON text) or make one. Err is the text for the agent.
pub fn save(existing: Option<&str>, a: &J, now: u64) -> Result<(String, String), String> {
    let given = a.get("slug").and_then(J::as_str).unwrap_or(""); let nm = a.get("name").and_then(J::as_str).unwrap_or("");
    let slug = task::slug(if given.trim().is_empty() { nm } else { given });
    let base = existing.and_then(task::read).unwrap_or(Task { slug: slug.clone(), name: String::new(), description: String::new(), origin: "tell".into(), start: String::new(), params: vec![], steps: vec![], urls: vec![], created: now, updated: now });
    let s = |k: &str, d: &str| a.get(k).and_then(J::as_str).map(str::to_string).unwrap_or_else(|| d.to_string());
    let steps: Vec<Step> = match a.get("steps").and_then(J::as_array) { Some(x) => x.iter().map(task::step_of).collect(), None => base.steps.clone() };
    let params = match a.get("params").and_then(J::as_array) { Some(x) => x.iter().map(|j| task::parse(&json!({"slug":"x","params":[j]})).unwrap().params.remove(0)).collect(), None => base.params.clone() };
    let start = s("start", &base.start); let us = urls(&steps);
    let renum: Vec<Step> = steps.into_iter().enumerate().map(|(i, mut st)| { st.n = i as u64 + 1; st }).collect();
    let t = Task { slug: slug.clone(), name: s("name", &base.name), description: s("description", &base.description), origin: base.origin.clone(), start: if start.is_empty() { us.first().cloned().unwrap_or_default() } else { start }, params, steps: renum, urls: us, created: base.created, updated: now };
    if t.name.trim().is_empty() { return Err("name the task".into()) }
    if t.steps.len() > 200 { return Err("at most 200 steps".into()) }
    Ok((t.slug.clone(), task::json(&t).to_string()))
}
/// task_run's plan: one step of a filled task with the selectors to try, best first.
pub fn plan(t: &Task) -> J {
    json!({"slug":t.slug,"name":t.name,"description":t.description,"start":t.start,"steps":t.steps.iter().map(|s| json!({"n":s.n,"do":s.act,"locators":task::locators(&s.target),"value":task::value_json(&s.value),"url":s.url,"wait":{"url":s.wait_url,"text":s.wait_text},"risk":task::risk(s),"note":s.note})).collect::<Vec<_>>()})
}

// ---- run ids and files ----
pub fn run_id(slug: &str, at: u64) -> String { format!("{slug}-{at}") }
/// The slug read back from a run id (everything before the last dash).
pub fn run_slug(run: &str) -> String { let ps: Vec<&str> = run.split('-').collect(); ps[..ps.len().saturating_sub(1)].join("-") }
/// A name safe to use as one path part: letters, digits, - _ and (when `dot`) one inner . ; never empty, never ..
pub fn part_ok(s: &str, dot: bool) -> bool { !s.is_empty() && !s.contains("..") && s.chars().count() <= 120 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || (dot && c == '.')) }
pub fn run_ok(run: &str) -> bool { part_ok(run, false) && !run_slug(run).is_empty() }
pub fn file_ok(f: &str) -> bool { part_ok(f, true) }
/// Where a run's files are, relative to the bot's folder.
pub fn run_dir(run: &str) -> String { format!("tasks/{}/runs/{run}", run_slug(run)) }
pub fn pause_file(n: u64) -> String { format!("pause-{n}.jpg") }
pub fn step_file(n: u64) -> String { format!("{n}.jpg") }
/// A run's state, kept as an Act's text.
pub fn act_text(slug: &str, run: &str, status: &str, n: u64, summary: &str, shots: &[String]) -> String { json!({"slug":slug,"run":run,"status":status,"n":n,"summary":summary,"shots":shots}).to_string() }

// ---- the pause ask ----
/// The answers a pause offers: a risky step goes on only by confirm.
pub fn pause_options(risky: bool) -> &'static str { if risky { "confirm\nskip\nstop" } else { "continue\nskip\nstop" } }
pub fn shot_url(bot: &str, run: &str, n: u64) -> String { format!("/bots/{bot}/tasks/shot?run={run}&n={n}") }
pub fn pause_detail(bot: &str, run: &str, slug: &str, n: u64, kind: &str, question: &str, risky: bool) -> String {
    json!({"bot":bot,"run":run,"slug":slug,"n":n,"kind":kind,"question":question,"risky":risky,"shot":shot_url(bot, run, n)}).to_string()
}
/// What the bot hears when the pause is answered.
pub fn pause_reply(answer: &str) -> String {
    match answer {
        "confirm" => "The person confirmed. Do the step, then go on. The browser is yours again.".into(),
        "continue" => "The person says continue: go on from this step. The browser is yours again.".into(),
        "skip" => "The person says skip this step: go on with the next one. The browser is yours again.".into(),
        "stop" => "The person says stop: end the task now with task_end (status stopped).".into(),
        o => format!("The person answered: {o}"),
    }
}
pub const TOOLS: &[&str] = &["task_list", "task_get", "task_save", "task_delete", "task_run", "task_step", "task_pause", "task_end"];

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn teach_bot_may_follows_the_driver() {
        let mut t = Teach::default(); assert!(t.bot_may("b1"));
        t.put(TeachBot::started("b1", 5)); assert!(!t.bot_may("b1")); assert!(t.bot_may("b2"), "another bot's browser is its own");
        t.put(TeachBot::blank("b1").takeover()); assert!(!t.bot_may("b1")); t.put(TeachBot::blank("b1").takeover().handback()); assert!(t.bot_may("b1"));
        assert!(t.info().is_empty(), "an idle bot leaves the map");
    }
    #[test] fn teach_modes_and_info() {
        let b = TeachBot::started("b1", 5); assert_eq!(b.mode_text(), "rec 5 0"); assert_eq!(b.clone().stop().mode_text(), "review"); assert_eq!(TeachBot::blank("b1").mode_text(), "");
        assert_eq!(TeachBot::blank("b1").takeover().takeover().drive, "takeover"); assert_eq!(TeachBot::started("b1", 1).takeover().drive, "teach", "not while teaching");
        assert_eq!(TeachBot::started("b1", 1).handback().drive, "teach", "a teach ends only by stop/save");
        let m = b.msg(); assert_eq!(m["info"]["drive.b1"], "teach"); assert_eq!(m["info"]["teach.b1"], "rec 5 0"); assert_eq!(m["info"]["teach.draft.b1"], "[]");
        assert_eq!(TeachBot::blank("b1").kvs()[2].1, "");
    }
    #[test] fn teach_recording_expires_after_ten_minutes_not_before() { let b = TeachBot::started("b1", 100); assert!(b.expired(700)); assert!(!b.expired(699)); assert!(!b.clone().stop().expired(9999)); let mut t = Teach::default(); t.put(b); assert_eq!(t.expired(700), vec!["b1"]) }
    #[test] fn teach_draft_secret_no_value() {
        let b = TeachBot::started("b1", 1).add(&json!({"do":"type","secret":true,"value":"hunter2"})); assert!(!b.draft_text().contains("hunter2")); assert_eq!(b.steps.len(), 1);
        assert_eq!(TeachBot::blank("b1").add(&json!({"do":"click"})).steps.len(), 0, "only while recording");
        let mut f = TeachBot::started("b1", 1); for _ in 0..MAX_STEPS + 5 { f = f.add(&note("x")) } assert_eq!(f.steps.len(), MAX_STEPS);
        assert_eq!(b.last().contains("\"n\":1"), true);
    }
    #[test] fn a_long_draft_in_the_info_keeps_its_newest_steps_within_the_bound() {
        let mut b = TeachBot::started("b1", 1); for i in 0..400 { b = b.add(&note(&format!("{i} {}", "x".repeat(900)))) }
        let t = b.draft_text_max(INFO_MAX); assert!(t.len() <= INFO_MAX && t.len() > 10_000, "{}", t.len()); assert!(t.contains("399 xxx") && !t.contains("\"0 xxx"), "newest kept, oldest left out");
        assert_eq!(b.draft_text().len() > INFO_MAX, true); assert_eq!(TeachBot::blank("b1").draft_text_max(10), "");
    }
    #[test] fn teach_pause_options() { assert_eq!(pause_options(true), "confirm\nskip\nstop"); assert_eq!(pause_options(false), "continue\nskip\nstop"); assert!(pause_reply("stop").contains("task_end")); assert_eq!(pause_reply("later"), "The person answered: later") }
    #[test] fn teach_run_ids_and_files_are_plain_names() {
        assert!(!run_ok("a/b-1")); assert!(!run_ok("..-1")); assert!(!file_ok("../x.jpg")); assert!(file_ok("pause-3.jpg")); assert!(run_ok("a-b-5")); assert!(!run_ok("nodash")); assert_eq!(run_slug(&run_id("a-b", 5)), "a-b");
        assert_eq!(run_dir("a-b-5"), "tasks/a-b/runs/a-b-5"); assert!(!part_ok(&"a".repeat(121), false));
    }
    #[test] fn teach_save_makes_marked_values_parameters_and_never_a_secrets() {
        let b = TeachBot::started("b1", 1).add(&json!({"do":"navigate","url":"https://x.test/o"})).add(&json!({"do":"type","value":"1042","target":{"name":"Order"}})).add(&json!({"do":"type","secret":true,"target":{"name":"Password"}}));
        let t = build(&b, "Order status", "d", &[json!({"step":2,"name":"order number","label":"Order"}), json!({"step":3,"name":"pw"}), json!({"step":2,"name":"again"})], 9);
        assert_eq!(t.slug, "order-status"); assert_eq!(t.steps[1].value, Value::Param("order_number".into())); assert_eq!(t.steps[2].value, Value::Secret("Password".into()));
        assert_eq!(t.params.len(), 2); assert_eq!(t.params[0].example, "1042"); assert_eq!(t.params[0].label, "Order"); assert_eq!(t.start, "https://x.test/o"); assert_eq!(t.urls, vec!["https://x.test/o"]);
        assert!(taught("Order status", &t).contains("<secret: Password>") && !taught("x", &t).contains("pw"));
    }
    #[test] fn teach_save_merges_renumbers_and_checks() {
        assert_eq!(save(None, &json!({"name":" "}), 1).unwrap_err(), "name the task");
        let (slug, js) = save(None, &json!({"name":"A b","steps":[{"n":9,"do":"navigate","url":"u"},{"n":9,"do":"click"}]}), 5).unwrap(); assert_eq!(slug, "a-b");
        let t = task::read(&js).unwrap(); assert_eq!((t.steps[0].n, t.steps[1].n, t.origin.as_str(), t.start.as_str(), t.created, t.updated), (1, 2, "tell", "u", 5, 5));
        let (_, js2) = save(Some(&js), &json!({"slug":"a-b","description":"new"}), 8).unwrap(); let t2 = task::read(&js2).unwrap(); assert_eq!((t2.name.as_str(), t2.description.as_str(), t2.steps.len(), t2.created, t2.updated), ("A b", "new", 2, 5, 8));
        let many: Vec<J> = (0..201).map(|_| json!({"do":"click"})).collect(); assert_eq!(save(None, &json!({"name":"x","steps":many}), 1).unwrap_err(), "at most 200 steps");
    }
    #[test] fn the_tasks_block_lists_the_bots_skills() {
        let rows = vec![("a".to_string(), task::json(&Task { slug: "a".into(), name: "A".into(), description: "does a".into(), origin: "".into(), start: "".into(), params: vec![task::Param { name: "p".into(), label: "".into(), kind: "text".into(), example: "".into() }], steps: vec![], urls: vec![], created: 0, updated: 0 }).to_string()), ("b".to_string(), "garbage".to_string())];
        assert_eq!(block(&rows), "<tasks>\n- a: A - does a (params: p)\n- b\n</tasks>"); assert_eq!(block(&[]), "");
    }
    #[test] fn alerts_are_remembered_64_deep() { let mut t = Teach::default(); for i in 0..70 { t.alert_mark(&format!("k{i}")) } assert!(t.alerted("k69") && !t.alerted("k0") && t.alerted("k6")) }
    #[test] fn the_teach_map_is_bounded() { let mut t = Teach::default(); for i in 0..MAX_BOTS { assert!(t.put(TeachBot::started(&format!("b{i}"), 1))) } assert!(!t.put(TeachBot::started("new", 1))); assert!(t.put(TeachBot::started("b0", 2))) }
    #[test] fn plan_has_locators_and_risk() { let t = task::parse(&json!({"slug":"s","name":"n","steps":[{"n":1,"do":"click","target":{"role":"button","name":"Pay now"}}]})).unwrap(); let p = plan(&t); assert_eq!(p["steps"][0]["risk"], "purchase"); assert_eq!(p["steps"][0]["locators"][0], "role=button[name=\"Pay now\"]") }
}

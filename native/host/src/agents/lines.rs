//! Claude's stream-json lines -> the events the JS hub writes (src/core/hub: Claude.line.k,
//! Claude.assistant, Claude.result, Usage.*, Turn.claude, Speed.*, Mu.*, Lim.claude). Pure: a `Cx` (the
//! thread's slice of the read model these rules look at) and a line in, events and live text out.
//! Not ported (said, not silent): subagent lines (`parent_tool_use_id`), background tasks (`Bg`), todo tools,
//! tool-call tracking for deploys, the allowance stream, thinking blocks while `Think` is on is ported.
use super::bg;
use serde_json::{json, Value};

pub fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
pub fn n(v: &Value, k: &str) -> u64 { v.get(k).and_then(Value::as_u64).unwrap_or(0) }
/// J.Json.as_raw: the text of a JSON number ("" when absent), as hub reads it.
pub fn raw(v: &Value, k: &str) -> String { match v.get(k) { Some(Value::Number(x)) => x.to_string(), Some(Value::String(x)) => x.clone(), _ => String::new() } }

/// Digits only, as M.Switch.num reads a setting (anything else ends the number).
pub fn num(s: &str) -> u64 { let mut a = 0u64; for c in s.chars() { match c.to_digit(10) { Some(d) => a = a.saturating_mul(10).saturating_add(d as u64), None => break } } a }
/// M.Tok.part: the i-th "/"-separated number of a setting.
pub fn part(v: &str, i: usize) -> u64 { v.split('/').nth(i).map(num).unwrap_or(0) }

/// Turn.nusd: dollars as written ("0.00070325", "7.5e-7") in nano-dollars.
pub fn nusd(rawv: &str) -> u64 {
    let mut it = rawv.splitn(2, 'e'); let m = it.next().unwrap_or(""); let e = it.next().unwrap_or("");
    let mant = |m: &str| { let mut p = m.splitn(2, '.'); let a = num(p.next().unwrap_or("")); let f = format!("{}000000000", p.next().unwrap_or("")); a.saturating_mul(1_000_000_000).saturating_add(num(&f[..9])) };
    if let Some(neg) = e.strip_prefix('-') { let mut v = mant(m); for _ in 0..num(neg) { v /= 10 } v } else { mant(m) }
}

/// The thread's state these rules read and write. `tok_*`, `speed`, `models` are the settings of the same names.
#[derive(Clone, Debug, Default)]
pub struct Cx {
    pub thread: String, pub running: bool, pub session: String, pub model: String, pub think: bool,
    pub used: u64, pub window: u64,
    pub tok_basis: String, pub tok_key: String, pub speed: String, pub models: String,
    pub tok_epoch_set: bool, pub tok_recount_set: bool,
    pub limits: std::collections::HashMap<String, String>,
    pub seq: u64,
    /// the thread's background work rows (a WorkSet "bg"), as text and parsed
    pub bg: Vec<String>, pub bg_text: String,
    /// work.deadline.<thread>.<task> keys already set, and the work.timeout.seconds setting
    pub deadlines: std::collections::HashSet<String>, pub timeout_setting: String,
    /// the allowance.claude setting, and whether the thread uses the default account (named accounts keep their own collector)
    pub allowance: String, pub default_account: bool,
    /// Sub.fg: the Agent/Task calls of this process that run in the foreground; their tool result is what the subagent answered
    pub fg_subs: std::collections::HashSet<String>,
}

/// How a line is judged by the command ledger (cmds.rs): the legacy rule (the result ends the turn), or the ledger's:
/// quiet (a stopped or foreign turn's line) and stream (current work) change no turn from a result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flow { Legacy, Quiet, Stream }

#[derive(Debug, Default, PartialEq)]
pub struct Fx { pub events: Vec<Value>, pub deltas: Vec<String>, pub delta_end: bool }

fn setting(k: String, v: String, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
pub fn hub_id(prefix: &str, now: u64, seq: u64) -> String { format!("{prefix}{now}-{seq}") }

/// Claude.tool_summary.any / TodoWrite is not ported (its summary reads the list).
pub fn tool_summary(name: &str, input: &Value) -> String {
    let pick = |k: &str| input.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string);
    // s02: a row says what the call did; a task's update says which and what became of it ("#3 completed")
    if name == "TaskUpdate" { let id = input.get("taskId").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default(); let st = pick("status").unwrap_or_default();
        let t = format!("{}{}", if id.is_empty() || id == "null" { String::new() } else { format!("#{id} ") }, st); if !t.trim().is_empty() { return t.trim().to_string() } }
    pick("command").or_else(|| pick("file_path")).or_else(|| pick("path")).or_else(|| pick("pattern")).or_else(|| pick("description")).or_else(|| pick("question"))
        .or_else(|| pick("query")).or_else(|| pick("url")).or_else(|| pick("subject")).or_else(|| pick("keyword")).or_else(|| pick("q")).unwrap_or_else(|| name.to_string())
}

/// Claude.uid: a line's uuid adds its first 8 characters to a block's id.
fn uid(uuid: &str) -> String { if uuid.is_empty() { String::new() } else { format!(":{}", uuid.chars().take(8).collect::<String>()) } }

impl Cx {
    pub fn new(thread: &str) -> Cx { Cx { thread: thread.into(), default_account: true, ..Default::default() } }

    pub(super) fn used_event(&mut self, used: u64, at: u64, out: &mut Vec<Value>) {
        if used > 0 && used != self.used { self.used = used; out.push(json!({"$":"UsageSet","thread":self.thread,"used":used,"window":0,"at":at})) }
    }

    /// Claude.init: the session id, recorded once; a new session's cost and speed start at 0.
    pub(super) fn init(&mut self, o: &Value, at: u64, out: &mut Vec<Value>) {
        let sid = s(o, "session_id"); let same = self.session == sid;
        if !same { out.push(json!({"$":"SessionSet","id":self.thread,"session":sid,"at":at})); }
        if !same && !sid.is_empty() {
            self.tok_basis = "c:0".into(); out.push(setting(format!("tok.basis.{}", self.thread), "c:0".into(), at));
            let o = &self.speed; let v = ["0".to_string(), "0".into(), part(o, 2).to_string(), part(o, 3).to_string(), part(o, 4).to_string(), part(o, 5).to_string(), part(o, 6).to_string(), part(o, 7).to_string()].join("/");
            self.speed = v.clone(); out.push(setting(format!("turn.speed.{}", self.thread), v, at));
        }
        if !same { self.session = sid.to_string() }
    }

    fn blocks(&mut self, m: &Value, uuid: &str, at: u64, out: &mut Vec<Value>) {
        let msg = { let i = s(m, "id"); if i.is_empty() { "msg" } else { i } };
        let Some(bs) = m.get("content").and_then(Value::as_array) else { return };
        for (i, b) in bs.iter().enumerate() {
            match s(b, "type") {
                "text" => out.push(json!({"$":"MessagePosted","thread":self.thread,"msg":format!("{msg}:{i}{}", uid(uuid)),"role":"assistant","text":s(b, "text"),"at":at})),
                "tool_use" => {
                    let id = { let x = s(b, "id"); if x.is_empty() { hub_id("a", at, self.seq) } else { x.to_string() } };
                    let name = { let x = s(b, "name"); if x.is_empty() { "tool" } else { x } };
                    let input = b.get("input").unwrap_or(&Value::Null); let summary = if name == "TodoWrite" { todo_summary(input) } else { tool_summary(name, input) };
                    out.push(json!({"$":"ActivityLogged","thread":self.thread,"act":id,"tone":"tool","kind":name,"summary":summary,"at":at}));
                    self.sub_about(b, at, out);
                }
                "thinking" if self.think && !s(b, "thinking").trim().is_empty() =>
                    out.push(json!({"$":"ActivityLogged","thread":self.thread,"act":format!("think:{msg}:{i}{}", uid(uuid)),"tone":"thinking","kind":"thinking","summary":s(b, "thinking"),"at":at})),
                _ => {}
            }
        }
    }

    /// Usage.window / Usage.widest: the result's widest context window.
    fn window(&mut self, o: &Value, at: u64, out: &mut Vec<Value>) {
        let w = o.get("modelUsage").and_then(Value::as_object).map(|m| m.values().map(|v| n(v, "contextWindow")).max().unwrap_or(0)).unwrap_or(0);
        if w > 0 && w != self.window { self.window = w; out.push(json!({"$":"UsageSet","thread":self.thread,"used":0,"window":w,"at":at})) }
    }

    /// Turn.claude: a result's turn usage once (by the result's uuid), with the speed and per-model settings.
    fn turn_usage(&mut self, o: &Value, at: u64, out: &mut Vec<Value>) {
        let u = o.get("usage").unwrap_or(&Value::Null);
        if u.get("output_tokens").is_none() || s(o, "uuid").is_empty() || s(o, "uuid") == self.tok_key { return }
        let has_cost = o.get("total_cost_usd").is_some();
        let total = nusd(&raw(o, "total_cost_usd"));
        let base = if self.tok_basis.starts_with("c:") { num(&self.tok_basis[2..]) } else { 0 };
        let basis_known = self.tok_basis.starts_with("c:");
        let hasc = has_cost && basis_known && total >= base;
        let think = u.get("output_tokens_details").map_or(false, |d| d.get("thinking_tokens").is_some());
        if !self.tok_epoch_set { self.tok_epoch_set = true; out.push(setting("tok.epoch".into(), "1".into(), at)) }
        if !self.tok_recount_set { self.tok_recount_set = true; out.push(setting("tok.recount".into(), "b".into(), at)) }
        let model = if !self.model.is_empty() { self.model.clone() } else { // Turn.main: the widest window
            let mut best = (String::new(), 0u64);
            if let Some(m) = o.get("modelUsage").and_then(Value::as_object) { for (k, v) in m { if best.0.is_empty() || n(v, "contextWindow") > best.1 { best = (k.clone(), n(v, "contextWindow")) } } }
            best.0 };
        let new_basis = if has_cost { format!("c:{total}") } else { self.tok_basis.clone() };
        out.push(json!({"$":"TurnUsage","thread":self.thread,"provider":"claude","model":model,"key":s(o, "uuid"),
            "input":n(u, "input_tokens"),"cread":n(u, "cache_read_input_tokens"),"cwrite":n(u, "cache_creation_input_tokens"),
            "output":n(u, "output_tokens"),"reasoning":u.get("output_tokens_details").map_or(0, |d| n(d, "thinking_tokens")),
            "cost":if hasc { total - base } else { 0 },"known":format!("irwo{}{}", if think { "t" } else { "" }, if hasc { "c" } else { "" }),"basis":new_basis,"at":at}));
        self.tok_key = s(o, "uuid").to_string(); self.tok_basis = new_basis;
        // Speed.claude
        if o.get("duration_api_ms").is_some() {
            let outt: u64 = o.get("modelUsage").and_then(Value::as_object).map_or(0, |m| m.values().map(|v| n(v, "outputTokens")).sum());
            let v = speed_value(&self.speed, n(o, "duration_api_ms"), outt, n(o, "duration_ms"), n(o, "num_turns"));
            self.speed = v.clone(); out.push(setting(format!("turn.speed.{}", self.thread), v, at));
        }
        // Mu.claude: the session's totals per model, logged when they change
        if let Some(m) = o.get("modelUsage").and_then(Value::as_object) {
            let lines: Vec<String> = m.iter().map(|(k, v)| format!("{k}|{}", [n(v, "inputTokens"), n(v, "outputTokens"), n(v, "cacheReadInputTokens"), n(v, "cacheCreationInputTokens"), nusd(&raw(v, "costUSD")), n(v, "webSearchRequests"), n(v, "contextWindow")].iter().map(u64::to_string).collect::<Vec<_>>().join("|"))).collect();
            let v = lines.join("\n");
            if !v.is_empty() && v != self.models { self.models = v.clone(); out.push(setting(format!("turn.models.{}", self.thread), v, at)) }
        }
    }

    /// Claude.result.why
    fn why(o: &Value) -> String {
        let r = s(o, "result").trim().to_string(); if !r.is_empty() { return r }
        let es = o.get("errors").and_then(Value::as_array).map(|a| a.iter().map(|e| e.as_str().map(str::to_string).unwrap_or_else(|| e.to_string())).collect::<Vec<_>>().join("; ")).unwrap_or_default();
        let es = es.trim().to_string(); if es.is_empty() { s(o, "subtype").to_string() } else { es }
    }

    /// Lim.claude: plan windows, logged when they change.
    fn limits(&mut self, o: &Value, at: u64, out: &mut Vec<Value>) {
        let info = o.get("rate_limit_info").unwrap_or(&Value::Null); let status = s(info, "status").to_string();
        let wins = info.get("unifiedWindows").unwrap_or(&Value::Null);
        for name in ["five_hour", "seven_day"] {
            let w = wins.get(name).unwrap_or(&Value::Null);
            if w.get("utilization").is_none() { continue }
            let permille = nusd(&raw(w, "utilization")) / 1_000_000 / 10 * 10;
            let v = format!("{permille}/{}/{status}", raw(w, "resetsAt")); let k = format!("limit.claude.{name}");
            if self.limits.get(&k) != Some(&v) { self.limits.insert(k.clone(), v.clone()); out.push(setting(k, v, at)) }
        }
    }

    // ---- background work, subagents, todos, allowance ----
    fn bg_put(&mut self, rows: Vec<String>, at: u64, out: &mut Vec<Value>) {
        let after = bg::show(&rows);
        if after != self.bg_text { out.push(json!({"$":"WorkSet","thread":self.thread,"kind":"bg","rows":after,"at":at})); self.bg_text = after }
        self.bg = rows;
    }
    /// Bg.mark: a mark in a task's log (it started, or ended with a status); none for one no tool call started
    fn bg_mark(&self, call: &str, kind: &str, text: &str, at: u64, out: &mut Vec<Value>) {
        if !call.is_empty() { out.push(json!({"$":"ActivityLogged","thread":bg::log(&self.thread),"act":format!("{call}/{kind}"),"tone":call,"kind":kind,"summary":text,"at":at})) }
    }
    /// WorkTimeout.start: a deadline for the task, reported only (never signalled): "work.deadline.<thread>.<id>" once.
    fn bg_deadline(&mut self, id: &str, o: &Value, at: u64, out: &mut Vec<Value>) {
        let key = format!("work.deadline.{}.{id}", self.thread); if self.deadlines.contains(&key) { return }
        let parse = |raw: &str, fb: u64| if !raw.is_empty() && raw.len() <= 10 && raw.chars().all(|c| c.is_ascii_digit()) { num(raw) } else { fb };
        let secs = parse(&raw(o, "timeoutSec"), parse(&self.timeout_setting, 1200));
        out.push(setting(key.clone(), if secs == 0 { "0".into() } else { (at + secs).to_string() }, at)); out.push(setting(format!("{key}.noted"), String::new(), at)); self.deadlines.insert(key);
    }
    /// Bg.outcome: what a task reported as it ended, noted in its log beside its end
    fn bg_outcome(&self, o: &Value, at: u64, out: &mut Vec<Value>) {
        let call = s(o, "tool_use_id"); let sum = s(o, "summary"); let usage = o.get("usage").cloned().unwrap_or(Value::Null);
        if call.is_empty() || (sum.is_empty() && usage.is_null()) { return }
        let summary = if sum.chars().count() > 600 { format!("{}…", sum.chars().take(600).collect::<String>()) } else { sum.to_string() };
        out.push(json!({"$":"ActivityLogged","thread":bg::log(&self.thread),"act":format!("{call}/outcome"),"tone":call,"kind":"outcome","summary":json!({"summary":summary,"usage":usage}).to_string(),"at":at}));
    }
    /// Bg.system
    fn bg_system(&mut self, o: &Value, at: u64, out: &mut Vec<Value>) {
        match s(o, "subtype") {
            "task_started" if bg::listed(o) => {
                let call = s(o, "tool_use_id"); let what = o.get("description").and_then(Value::as_str).unwrap_or("Subagent"); let kind = bg::kind(s(o, "task_type")); let id = s(o, "task_id");
                let rows = bg::start(&self.bg, id, what, call, what, kind); self.bg_put(rows, at, out);
                self.bg_mark(call, &format!("start:{kind}"), what, at, out); self.bg_deadline(id, o, at, out);
            }
            "task_progress" => { let rows = bg::progress_line(&self.bg, o); self.bg_put(rows, at, out) }
            "task_notification" => {
                let rows = bg::drop(&self.bg, s(o, "task_id")); self.bg_put(rows, at, out);
                let status = o.get("status").and_then(Value::as_str).unwrap_or("completed"); self.bg_mark(s(o, "tool_use_id"), "end", status, at, out); self.bg_outcome(o, at, out);
            }
            "task_updated" => {
                let patch = o.get("patch").unwrap_or(&Value::Null); let status = s(patch, "status"); let id = s(o, "task_id");
                if ["completed", "failed", "killed", "stopped", "cancelled"].contains(&status) { let rows = bg::end(&self.bg, id, status); self.bg_put(rows, at, out) }
                else if patch.get("is_backgrounded").and_then(Value::as_bool).unwrap_or(false) { let rows = bg::start(&self.bg, id, "Background shell", "", "Background shell", "shell"); self.bg_put(rows, at, out); self.bg_deadline(id, &Value::Null, at, out) }
            }
            "background_tasks_changed" => { let ids: Vec<String> = o.get("tasks").and_then(Value::as_array).map(|a| a.iter().map(|t| s(t, "task_id").to_string()).collect()).unwrap_or_default(); let rows = bg::sync(&self.bg, &ids); self.bg_put(rows, at, out) }
            _ => {}
        }
    }
    /// A burst of held progress lines, written once (Prog.flush).
    pub fn progress(&mut self, lines: &[Value], at: u64) -> Vec<Value> { let mut rows = self.bg.clone(); for l in lines { rows = bg::progress_line(&rows, l) } let mut out = vec![]; self.bg_put(rows, at, &mut out); out }
    pub fn bg_clear(&mut self, at: u64) -> Vec<Value> { let mut out = vec![]; self.bg_put(vec![], at, &mut out); out }
    pub fn bg_replace(&mut self, rows: Vec<String>, at: u64) -> Vec<Value> { let mut out = vec![]; self.bg_put(rows, at, &mut out); out }
    /// Claude.result.why of a line
    pub fn why_of(o: &Value) -> String { Cx::why(o) }
    /// Bg.settle: a turn ended: the outcomes of the rows that ended are the agent's now
    pub fn bg_settle(&mut self, at: u64) -> Vec<Value> { let rows = bg::unended(&self.bg); let mut out = vec![]; self.bg_put(rows, at, &mut out); out }
    /// Bg.stop.outs: the control requests that stop each task still at work
    pub fn bg_stop_lines(&self, rid: &str) -> Vec<String> {
        self.bg.iter().filter(|l| !bg::ended(l)).map(|l| { let id = bg::field(l, 0); json!({"type":"control_request","request_id":format!("{rid}-{id}"),"request":{"subtype":"stop_task","task_id":id}}).to_string() }).collect()
    }
    /// Sub.blocks: a subagent's own text and tool calls go to its task's log, not the thread's timeline
    fn sub_blocks(&self, o: &Value, call: &str, at: u64) -> Vec<Value> {
        let m = o.get("message").unwrap_or(&Value::Null); let msg = { let i = s(m, "id"); if i.is_empty() { "msg" } else { i } }; let log = bg::log(&self.thread); let mut out = vec![];
        for (n, b) in m.get("content").and_then(Value::as_array).into_iter().flatten().enumerate() {
            match s(b, "type") {
                "text" => out.push(json!({"$":"ActivityLogged","thread":log,"act":format!("{call}/{msg}:{n}"),"tone":call,"kind":"say","summary":s(b, "text"),"at":at})),
                "tool_use" => { let name = { let x = s(b, "name"); if x.is_empty() { "tool" } else { x } }; let id = { let x = s(b, "id"); if x.is_empty() { format!("{msg}:{n}") } else { x.to_string() } };
                    out.push(json!({"$":"ActivityLogged","thread":log,"act":format!("{call}/{id}"),"tone":call,"kind":name,"summary":tool_summary(name, b.get("input").unwrap_or(&Value::Null)),"at":at})) }
                _ => {}
            }
        }
        out
    }
    /// Sub.results: a subagent's tool results, each in its log beside its call (`{use, text, error}`; the text cut at RESULT_MAX characters)
    fn sub_results(&self, o: &Value, call: &str, at: u64) -> Vec<Value> {
        let log = bg::log(&self.thread);
        o.get("message").and_then(|m| m.get("content")).and_then(Value::as_array).into_iter().flatten().filter(|b| s(b, "type") == "tool_result" && !s(b, "tool_use_id").is_empty()).map(|b| {
            let id = s(b, "tool_use_id"); let err = b.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            json!({"$":"ActivityLogged","thread":log,"act":format!("{call}/{id}/result"),"tone":call,"kind":"result","summary":json!({"use":id,"text":cut(&all_text(b.get("content").unwrap_or(&Value::Null)), RESULT_MAX),"error":err}).to_string(),"at":at})
        }).collect()
    }
    /// Sub.reply: a foreground subagent's answer is its Agent call's result in the agent's own line; noted in its log (`{text, error}`) as it ends
    fn sub_replies(&mut self, o: &Value, at: u64, out: &mut Vec<Value>) {
        if self.fg_subs.is_empty() { return }
        for b in o.get("message").and_then(|m| m.get("content")).and_then(Value::as_array).into_iter().flatten() {
            let id = s(b, "tool_use_id"); if s(b, "type") != "tool_result" || !self.fg_subs.remove(id) { continue }
            let err = b.get("is_error").and_then(Value::as_bool).unwrap_or(false);
            out.push(json!({"$":"ActivityLogged","thread":bg::log(&self.thread),"act":format!("{id}/reply"),"tone":id,"kind":"reply","summary":json!({"text":cut(&all_text(b.get("content").unwrap_or(&Value::Null)), RESULT_MAX),"error":err}).to_string(),"at":at}));
        }
    }
    /// Sub.about: what an Agent call's subagent was asked, its type and model, noted in its log
    fn sub_about(&mut self, b: &Value, at: u64, out: &mut Vec<Value>) {
        let name = s(b, "name"); let id = s(b, "id"); if !is_sub_tool(name) || id.is_empty() { return }
        let i = b.get("input").unwrap_or(&Value::Null); let p = s(i, "prompt"); let p = if p.chars().count() > 1500 { format!("{}…", p.chars().take(1500).collect::<String>()) } else { p.to_string() };
        let mut about = json!({"type":s(i, "subagent_type"),"model":s(i, "model"),"prompt":p});
        // a foreground one has no task_started to name it (Bg.mark's start): its description is its title
        if !i.get("run_in_background").and_then(Value::as_bool).unwrap_or(false) { if self.fg_subs.len() > 256 { self.fg_subs.clear() } self.fg_subs.insert(id.to_string()); about["description"] = json!(s(i, "description")); }
        out.push(json!({"$":"ActivityLogged","thread":bg::log(&self.thread),"act":format!("{id}/about"),"tone":id,"kind":"about","summary":about.to_string(),"at":at}));
    }
    /// Claude.todos: an assistant message's todo tools, for the model to fold into the thread's list
    fn todos_of(&self, m: &Value, at: u64, out: &mut Vec<Value>) {
        let bs: Vec<&Value> = m.get("content").and_then(Value::as_array).into_iter().flatten().filter(|b| s(b, "type") == "tool_use" && ["TodoWrite", "TaskCreate", "TaskUpdate"].contains(&s(b, "name"))).collect();
        if !bs.is_empty() { out.push(json!({"$":"TodosChanged","thread":self.thread,"who":"tools","data":Value::Array(bs.into_iter().cloned().collect()).to_string(),"at":at})) }
    }
    /// Claude.named: a "user" line carries tool results; a TaskCreate's names its step
    fn named(&self, o: &Value, at: u64, out: &mut Vec<Value>) {
        let mut v: Vec<Value> = vec![];
        for b in o.get("message").and_then(|m| m.get("content")).and_then(Value::as_array).into_iter().flatten() {
            let n = todo_result(&result_text(b.get("content").unwrap_or(&Value::Null)));
            if !n.is_empty() { v.push(json!({"name":"TaskNamed","use":s(b, "tool_use_id"),"number":n})) }
        }
        if !v.is_empty() { out.push(json!({"$":"TodosChanged","thread":self.thread,"who":"tools","data":Value::Array(v).to_string(),"at":at})) }
    }
    /// AllowanceStream.changes: passive readings of the plan windows, never zero when missing
    fn allowance(&mut self, o: &Value, at: u64, out: &mut Vec<Value>) {
        if !self.default_account { return }
        let w = o.get("rate_limit_info").and_then(|r| r.get("unifiedWindows")).cloned().unwrap_or(Value::Null); let mut ws: Vec<Value> = vec![];
        for (key, name, scope, minutes) in [("five_hour", "5 h", "shared", 300u64), ("seven_day", "7 d", "shared", 10080), ("seven_day_opus", "7 d · Opus", "opus", 10080), ("seven_day_sonnet", "7 d · Sonnet", "sonnet", 10080)] {
            let Some(win) = w.get(key) else { continue };
            let used: f32 = match win.get("utilization") { Some(Value::Number(x)) => x.to_string().parse().unwrap_or(-1.0), _ => -1.0 };
            let resets = win.get("resetsAt").cloned().unwrap_or(Value::Null);
            ws.push(if (0.0..=1.0).contains(&used) { json!({"name":name,"scope":scope,"shared":true,"duration_minutes":minutes,"used_permille":(used * 1000.0).ceil() as u64,"reported_utilization":win["utilization"],"resets_at":resets}) }
                    else { json!({"name":name,"scope":scope,"shared":true,"duration_minutes":minutes,"used_permille":Value::Null,"resets_at":resets}) });
        }
        let old: Value = serde_json::from_str(&self.allowance).unwrap_or(Value::Null); let old_at = n(&old, "at");
        let different = Value::Array(ws.clone()).to_string() != old.get("windows").cloned().unwrap_or(Value::Null).to_string();
        if !ws.is_empty() && (different || old_at == 0 || at < old_at || at - old_at >= 60) {
            let snap = json!({"provider":"claude","source":"Claude stream","auth":"unknown","state":"ok","at":at,"windows":ws}).to_string();
            self.allowance = snap.clone(); out.push(setting("allowance.claude".into(), snap, at));
        }
    }

    /// One main-agent line judged by the legacy rule. `at` is the clock in seconds.
    pub fn line(&mut self, o: &Value, at: u64) -> Fx { self.line_flow(o, at, Flow::Legacy) }

    /// Claude.line.k / Claude.line.r: one line; `flow` says whether the command ledger lets its text stream and whether a result may end the turn.
    pub fn line_flow(&mut self, o: &Value, at: u64, flow: Flow) -> Fx {
        let mut fx = Fx::default();
        let call = s(o, "parent_tool_use_id");
        if !call.is_empty() { match s(o, "type") { "assistant" => fx.events = self.sub_blocks(o, call, at), "user" => fx.events = self.sub_results(o, call, at), _ => {} } return fx } // Sub.line
        self.seq += 1;
        match s(o, "type") {
            "system" => {
                let mut ev = vec![];
                match s(o, "subtype") {
                    "init" => self.init(o, at, &mut ev),
                    "compact_boundary" => { let post = o.get("compact_metadata").map_or(0, |m| n(m, "post_tokens")); self.used_event(post, at, &mut ev) }
                    _ => {}
                }
                self.bg_system(o, at, &mut ev); fx.events = ev;
            }
            "stream_event" => {
                let run = match flow { Flow::Legacy => self.running, Flow::Quiet => false, Flow::Stream => true };
                if run { let ev = o.get("event").unwrap_or(&Value::Null); let d = ev.get("delta").unwrap_or(&Value::Null);
                    if s(ev, "type") == "content_block_delta" && s(d, "type") == "text_delta" { fx.deltas.push(s(d, "text").to_string()) } }
            }
            "assistant" => {
                let m = o.get("message").unwrap_or(&Value::Null); let mut ev = vec![];
                self.blocks(m, s(o, "uuid"), at, &mut ev); self.todos_of(m, at, &mut ev);
                let u = m.get("usage").unwrap_or(&Value::Null);
                let used = n(u, "input_tokens") + n(u, "cache_creation_input_tokens") + n(u, "cache_read_input_tokens") + n(u, "output_tokens");
                self.used_event(used, at, &mut ev);
                if self.running && !s(o, "error").is_empty() { // Claude.api.err
                    let text = m.get("content").and_then(Value::as_array).and_then(|a| a.iter().find(|b| s(b, "type") == "text")).map(|b| s(b, "text")).unwrap_or("");
                    let text = if text.trim().is_empty() { s(o, "error") } else { text };
                    ev.push(json!({"$":"ActivityLogged","thread":self.thread,"act":hub_id("api", at, self.seq),"tone":"error","kind":"api","summary":text,"at":at}));
                }
                fx.events = ev; fx.delta_end = true;
            }
            "result" => {
                let mut ev = vec![];
                if flow == Flow::Legacy && self.running {
                    if o.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
                        ev.push(json!({"$":"ActivityLogged","thread":self.thread,"act":hub_id("e", at, 0),"tone":"error","kind":"result","summary":Cx::why(o),"at":at}));
                        ev.push(json!({"$":"TurnChanged","thread":self.thread,"state":"failed","at":at}));
                    } else { ev.push(json!({"$":"TurnChanged","thread":self.thread,"state":"completed","at":at})) }
                    self.running = false;
                }
                self.window(o, at, &mut ev); self.turn_usage(o, at, &mut ev);
                fx.events = ev;
            }
            "user" => { let mut ev = vec![]; self.named(o, at, &mut ev); self.sub_replies(o, at, &mut ev); fx.events = ev }
            "rate_limit_event" => { let mut ev = vec![]; self.limits(o, at, &mut ev); self.allowance(o, at, &mut ev); fx.events = ev }
            _ => {}
        }
        fx
    }

    /// Claude.line.v.k 6: Claude started a turn by itself (a background task ended, a wakeup): the thread runs again, with an Act saying why.
    /// `newest` is the thread's newest entry (the turn's start mark).
    pub fn wake_start(&mut self, why: &str, newest: &str, seq: u64, at: u64) -> Vec<Value> {
        let th = self.thread.clone(); self.running = true;
        vec![json!({"$":"TurnChanged","thread":th,"state":"running","at":at}), setting(format!("turn.from.{th}"), format!("@{newest}"), at), setting(format!("restart.retry.{th}"), String::new(), at), setting(format!("provider.resume.{th}"), String::new(), at),
            json!({"$":"ActivityLogged","thread":th,"act":hub_id("k", at, seq),"tone":"info","kind":"wake","summary":why,"at":at})]
    }
    /// The ledger ended the turn (verdicts 3 ok, 4 failed, 5 stopped): the turn's own events, then the rows settled.
    pub fn end(&mut self, verdict: u8, text: &str, at: u64) -> Vec<Value> {
        let mut ev = vec![];
        match verdict {
            3 => ev.push(json!({"$":"TurnChanged","thread":self.thread,"state":"completed","at":at})),
            4 => { ev.push(json!({"$":"ActivityLogged","thread":self.thread,"act":hub_id("e", at, 0),"tone":"error","kind":"result","summary":text,"at":at})); ev.push(json!({"$":"TurnChanged","thread":self.thread,"state":"failed","at":at})) }
            5 => ev.push(json!({"$":"TurnChanged","thread":self.thread,"state":"interrupted","at":at})),
            _ => return ev,
        }
        self.running = false; ev.extend(self.bg_settle(at)); ev
    }
}

/// The tool names that start a subagent (Claude's Task, renamed Agent)
pub fn is_sub_tool(name: &str) -> bool { name == "Agent" || name == "Task" }
/// Sub.result.max: how much of a subagent's tool result its log keeps
pub const RESULT_MAX: usize = 4000;
fn cut(t: &str, n: usize) -> String { if t.chars().count() > n { format!("{}…", t.chars().take(n).collect::<String>()) } else { t.to_string() } }
/// Every text block of a tool result, joined (a string result as it is)
fn all_text(c: &Value) -> String { match c { Value::Array(a) => a.iter().filter(|x| s(x, "type") == "text" || x.get("text").is_some()).map(|x| s(x, "text")).collect::<Vec<_>>().join("\n"), Value::String(x) => x.clone(), _ => String::new() } }
fn result_text(c: &Value) -> String { match c { Value::Array(a) => a.first().map(|x| s(x, "text").to_string()).unwrap_or_default(), Value::String(x) => x.clone(), _ => String::new() } }
/// Todo.result: the number in a TaskCreate result ("Task #3 created successfully: ..."), or "" for anything else
pub fn todo_result(text: &str) -> String { if !text.starts_with("Task #") { return String::new() } text.split('#').nth(1).map(|r| r.chars().take_while(|c| c.is_ascii_digit()).collect()).unwrap_or_default() }
/// Todo.summary of a TodoWrite call: "2 of 6 done · now: <the step in progress>"
pub fn todo_summary(input: &Value) -> String {
    let st = |x: &Value| match s(x, "status") { "completed" => "completed", "in_progress" => "in_progress", _ => "pending" };
    let ts: Vec<&Value> = input.get("todos").and_then(Value::as_array).into_iter().flatten().collect();
    let done = ts.iter().filter(|x| st(x) == "completed").count(); let now = ts.iter().find(|x| st(x) == "in_progress").map(|x| s(x, "content")).unwrap_or("");
    format!("{done} of {} done{}", ts.len(), if now.is_empty() { String::new() } else { format!(" · now: {now}") })
}

/// Speed.value: "<api ms>/<out>/<turn api ms>/<turn out>/<turn wall ms>/<timed api ms>/<timed out>/<steps>": the running totals, the last turn's share and its
/// wall time, the thread's turns whose share was told, and the turn's model calls. A turn's share is the rise since the last result; a session unseen before is not told.
pub fn speed_value(old: &str, api: u64, out: u64, wall: u64, steps: u64) -> String {
    let known = !old.is_empty(); let rise = |nowv: u64, before: u64| if nowv >= before { nowv - before } else { nowv };
    let a = if known { rise(api, part(old, 0)) } else { 0 }; let o = if known { rise(out, part(old, 1)) } else { 0 };
    let told = a > 0 && o > 0; let tt = |x: u64| if told { x } else { 0 };
    [api, out, tt(a), tt(o), wall, part(old, 5) + tt(a), part(old, 6) + tt(o), steps].iter().map(u64::to_string).collect::<Vec<_>>().join("/")
}
/// Wake.text: what the Act of a turn Claude started by itself says. A task killed by a Stop woke the agent too, so the status is named.
pub fn wake_text(lifecycle: bool, rows: &[String], o: &Value) -> String {
    if lifecycle { return "Resumed after Stop".into() }
    let sum = s(o, "summary").to_string();
    let title = if !sum.is_empty() { sum } else { let id = bg::clean(s(o, "task_id")); let r = rows.iter().find(|r| bg::field(r, 0) == id).map(|r| { let t = bg::field(r, 3); if t.is_empty() { bg::doing(r) } else { t } }).unwrap_or_default(); if r.is_empty() { "background work".into() } else { r } };
    let status = s(o, "status"); format!("Woken: {title}{}", if status.is_empty() || status == "completed" { String::new() } else { format!(" ({status})") })
}

/// Delta.merge: within one read's lines a run of the main agent's text deltas to one block becomes one line,
/// every other line stays in its place. Returns the lines out. Text is never lost or reordered.
pub fn merge(lines: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut run: Option<(String, Vec<String>)> = None; // block index, texts
    let flush = |run: &mut Option<(String, Vec<String>)>, out: &mut Vec<String>| {
        if let Some((_, ts)) = run.take() { out.push(json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":ts.concat()}}}).to_string()) }
    };
    for l in lines {
        let delta = if l.starts_with("{\"type\":\"stream_event\"") { serde_json::from_str::<Value>(&l).ok().and_then(|o| {
            let ev = o.get("event")?; let d = ev.get("delta")?;
            (s(&o, "parent_tool_use_id").is_empty() && s(ev, "type") == "content_block_delta" && s(d, "type") == "text_delta").then(|| (ev.get("index").map(|i| i.to_string()).unwrap_or_default(), s(d, "text").to_string()))
        }) } else { None };
        match delta {
            Some((k, t)) => {
                let same = matches!(&run, Some((rk, _)) if *rk == k);
                if same { if let Some((_, ts)) = run.as_mut() { ts.push(t) } } else { flush(&mut run, &mut out); run = Some((k, vec![t])) }
            }
            None => { flush(&mut run, &mut out); out.push(l) }
        }
    }
    flush(&mut run, &mut out);
    out
}

/// Delta.streaming: a read that ended in a stream event waits Delta.pace() before the next.
pub fn streaming(lines: &[String]) -> bool { lines.last().map_or(false, |l| l.starts_with("{\"type\":\"stream_event\"")) }
pub const PACE_MS: u64 = 200;

#[cfg(test)]
mod tests {
    use super::*;
    fn ev(l: &str) -> Value { serde_json::from_str(l).unwrap() }
    fn run(cx: &mut Cx, l: &str) -> Fx { cx.line(&ev(l), 100) }

    #[test] fn init_sets_the_session_once_and_starts_the_counters() {
        let mut c = Cx::new("t1"); let a = run(&mut c, r#"{"type":"system","subtype":"init","session_id":"S1"}"#);
        let ks: Vec<String> = a.events.iter().map(|e| format!("{}{}", e["$"], e["key"])).collect();
        assert_eq!(ks, ["\"SessionSet\"null", "\"SettingSet\"\"tok.basis.t1\"", "\"SettingSet\"\"turn.speed.t1\""]);
        assert_eq!(a.events[2]["value"], "0/0/0/0/0/0/0/0");
        assert!(run(&mut c, r#"{"type":"system","subtype":"init","session_id":"S1"}"#).events.is_empty());
    }
    #[test] fn assistant_blocks_become_messages_and_tool_rows_with_usage() {
        let mut c = Cx::new("t1");
        let a = run(&mut c, r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"text","text":"hi"},{"type":"tool_use","id":"tu1","name":"Bash","input":{"command":"ls"}}],"usage":{"input_tokens":10,"cache_read_input_tokens":100,"output_tokens":5}}}"#);
        assert_eq!(a.events[0], json!({"$":"MessagePosted","thread":"t1","msg":"m1:0","role":"assistant","text":"hi","at":100}));
        assert_eq!(a.events[1], json!({"$":"ActivityLogged","thread":"t1","act":"tu1","tone":"tool","kind":"Bash","summary":"ls","at":100}));
        assert_eq!(a.events[2], json!({"$":"UsageSet","thread":"t1","used":115,"window":0,"at":100})); assert!(a.delta_end);
        let again = run(&mut c, r#"{"type":"assistant","message":{"id":"m2","content":[],"usage":{"input_tokens":10,"cache_read_input_tokens":100,"output_tokens":5}}}"#);
        assert!(again.events.is_empty()); // usage_same_nothing
    }
    #[test] fn a_uuid_adds_eight_characters_to_a_block_id() {
        let mut c = Cx::new("t1");
        let a = run(&mut c, r#"{"type":"assistant","uuid":"abcdef0123456789","message":{"id":"m1","content":[{"type":"text","text":"x"}]}}"#);
        assert_eq!(a.events[0]["msg"], "m1:0:abcdef01");
    }
    #[test] fn claude_result_after_stop_quiet() { let mut c = Cx::new("t1"); assert!(run(&mut c, r#"{"type":"result","is_error":false}"#).events.is_empty()); }
    #[test] fn claude_delta_after_stop_quiet() { let mut c = Cx::new("t1"); assert!(run(&mut c, r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"late"}}}"#).deltas.is_empty()); }
    #[test] fn usage_same_nothing() { let mut c = Cx::new("t1"); c.used = 7; let a = run(&mut c, r#"{"type":"assistant","message":{"id":"m","content":[],"usage":{"output_tokens":7}}}"#); assert!(a.events.is_empty()); }
    #[test] fn result_completes_only_a_running_turn_and_failed_results_log_their_error() {
        let mut c = Cx::new("t1");
        assert!(run(&mut c, r#"{"type":"result","subtype":"success","is_error":false}"#).events.is_empty()); // claude_result_after_stop_quiet
        c.running = true;
        assert_eq!(run(&mut c, r#"{"type":"result","subtype":"success","is_error":false}"#).events, [json!({"$":"TurnChanged","thread":"t1","state":"completed","at":100})]);
        c.running = true;
        let f = run(&mut c, r#"{"type":"result","subtype":"error_max_turns","is_error":true,"errors":["a","b"]}"#);
        assert_eq!(f.events[0]["summary"], "a; b"); assert_eq!(f.events[1]["state"], "failed");
    }
    #[test] fn deltas_stream_only_while_the_turn_runs() {
        let mut c = Cx::new("t1"); let l = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"ab"}}}"#;
        assert!(run(&mut c, l).deltas.is_empty()); // claude_delta_after_stop_quiet
        c.running = true; assert_eq!(run(&mut c, l).deltas, ["ab"]);
    }
    #[test] fn a_subagents_lines_go_to_its_tasks_log_and_not_the_timeline() {
        let mut c = Cx::new("t1"); c.running = true;
        let a = run(&mut c, r#"{"type":"assistant","parent_tool_use_id":"call1","message":{"id":"ms1","content":[{"type":"text","text":"Checked it."},{"type":"tool_use","id":"st1","name":"Read","input":{"file_path":"/tmp/f.kicad_mod"}}],"usage":{"output_tokens":9}}}"#);
        assert_eq!(a.events, vec![json!({"$":"ActivityLogged","thread":"@sub:t1","act":"call1/ms1:0","tone":"call1","kind":"say","summary":"Checked it.","at":100}), json!({"$":"ActivityLogged","thread":"@sub:t1","act":"call1/st1","tone":"call1","kind":"Read","summary":"/tmp/f.kicad_mod","at":100})]);
        assert!(a.deltas.is_empty() && !a.delta_end && c.used == 0, "its text, partial text and usage are not the agent's");
        assert_eq!(run(&mut c, r#"{"type":"stream_event","parent_tool_use_id":"call1","event":{}}"#), Fx::default());
    }
    #[test] fn the_presenter_opens_a_subagent_from_the_same_tools() { for n in ["Agent", "Task", "Bash", "TaskCreate", ""] { assert_eq!(is_sub_tool(n), !crate::present::panels::sub_key(n, "x").is_empty(), "{n}") } }
    #[test] fn a_subagents_tool_results_and_a_foreground_ones_answer_go_to_its_log() {
        let mut c = Cx::new("t1"); c.running = true;
        let r = run(&mut c, r#"{"type":"user","parent_tool_use_id":"call1","message":{"content":[{"type":"tool_result","tool_use_id":"st1","content":[{"type":"text","text":"line 1"},{"type":"text","text":"line 2"}],"is_error":true}]}}"#);
        assert_eq!(r.events.len(), 1); let e = &r.events[0];
        assert_eq!((e["thread"].as_str(), e["act"].as_str(), e["tone"].as_str(), e["kind"].as_str()), (Some("@sub:t1"), Some("call1/st1/result"), Some("call1"), Some("result")));
        assert_eq!(serde_json::from_str::<Value>(e["summary"].as_str().unwrap()).unwrap(), json!({"use":"st1","text":"line 1\nline 2","error":true}));
        // a foreground Agent call: its result is the subagent's answer; a background one's ("Async agent launched") is not
        let a = run(&mut c, r#"{"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","id":"fg1","name":"Agent","input":{"description":"Survey","prompt":"Look.","subagent_type":"Explore"}},{"type":"tool_use","id":"bg1","name":"Agent","input":{"prompt":"Later.","run_in_background":true}}]}}"#);
        let about: Value = serde_json::from_str(a.events.iter().find(|e| e["act"] == "fg1/about").unwrap()["summary"].as_str().unwrap()).unwrap(); assert_eq!(about["description"], "Survey");
        let u = run(&mut c, r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"fg1","content":"Found two."},{"type":"tool_result","tool_use_id":"bg1","content":"Async agent launched: x"}]}}"#);
        let replies: Vec<&Value> = u.events.iter().filter(|e| e["kind"] == "reply").collect();
        assert_eq!(replies.len(), 1); assert_eq!((replies[0]["act"].as_str(), replies[0]["tone"].as_str()), (Some("fg1/reply"), Some("fg1")));
        assert_eq!(serde_json::from_str::<Value>(replies[0]["summary"].as_str().unwrap()).unwrap()["text"], "Found two.");
        assert!(run(&mut c, r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"fg1","content":"again"}]}}"#).events.iter().all(|e| e["kind"] != "reply"), "once");
    }
    #[test] fn the_result_records_window_turn_usage_speed_and_models_once() {
        let mut c = Cx::new("t1"); c.running = true; c.tok_basis = "c:0".into();
        let l = r#"{"type":"result","subtype":"success","is_error":false,"uuid":"u1","duration_ms":10,"duration_api_ms":7,"num_turns":1,"total_cost_usd":0.0123,"usage":{"input_tokens":1200,"cache_creation_input_tokens":300,"cache_read_input_tokens":4000,"output_tokens":450},"modelUsage":{"m":{"inputTokens":10,"outputTokens":5,"contextWindow":200000}}}"#;
        let a = run(&mut c, l); let ks: Vec<&str> = a.events.iter().map(|e| e["$"].as_str().unwrap()).collect();
        assert_eq!(ks, ["TurnChanged", "UsageSet", "SettingSet", "SettingSet", "TurnUsage", "SettingSet", "SettingSet"]);
        let tu = &a.events[4]; assert_eq!(tu["cost"], 12_300_000u64); assert_eq!(tu["known"], "irwoc"); assert_eq!(tu["basis"], "c:12300000");
        assert!(run(&mut c, l).events.iter().all(|e| e["$"] != "TurnUsage")); // a result line seen again is not counted twice
    }
    #[test] fn a_rate_limit_event_updates_the_limit_settings_and_the_allowance_snapshot_once() {
        let mut c = Cx::new("t1");
        let l = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"five_hour":{"utilization":0.42,"resetsAt":1800000000},"seven_day":{"utilization":"bad","resetsAt":1800500000}}}}"#;
        let a = run(&mut c, l); let keys: Vec<&str> = a.events.iter().map(|e| e["key"].as_str().unwrap()).collect();
        assert_eq!(keys, ["limit.claude.five_hour", "limit.claude.seven_day", "allowance.claude"]); assert_eq!(a.events[0]["value"], "420/1800000000/allowed");
        let snap: Value = serde_json::from_str(a.events[2]["value"].as_str().unwrap()).unwrap();
        assert_eq!(snap["windows"][0], json!({"name":"5 h","scope":"shared","shared":true,"duration_minutes":300,"used_permille":420,"reported_utilization":0.42,"resets_at":1800000000}));
        assert_eq!(snap["windows"][1], json!({"name":"7 d","scope":"shared","shared":true,"duration_minutes":10080,"used_permille":null,"resets_at":1800500000}), "an invalid window is an unknown row, never zero");
        assert!(run(&mut c, l).events.is_empty(), "the same reading inside a minute writes nothing");
        c.default_account = false; c.allowance.clear(); assert!(!run(&mut c, &l.replace("0.42", "0.43")).events.iter().any(|e| e["key"] == "allowance.claude"), "a named account has its own collector");
    }
    #[test] fn a_todo_write_says_where_the_list_stands_and_hands_the_model_the_tool_calls() {
        let mut c = Cx::new("t1");
        let a = run(&mut c, r#"{"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","id":"tu1","name":"TodoWrite","input":{"todos":[{"content":"a","status":"completed"},{"content":"b","status":"in_progress"},{"content":"c","status":"pending"}]}}]}}"#);
        assert_eq!(a.events[0]["summary"], "1 of 3 done · now: b"); assert_eq!(a.events[1]["$"], "TodosChanged"); assert_eq!(a.events[1]["who"], "tools"); assert!(a.events[1]["data"].as_str().unwrap().starts_with("[{\"type\":\"tool_use\""));
        let n = run(&mut c, r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu2","content":"Task #7 created successfully: x"}]}}"#);
        assert_eq!(n.events[0]["data"], json!([{"name":"TaskNamed","use":"tu2","number":"7"}]).to_string());
        assert!(run(&mut c, r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu3","content":[{"type":"text","text":"ok"}]}]}}"#).events.is_empty());
    }
    #[test] fn nusd_reads_plain_and_exponent_dollars() { assert_eq!(nusd("0.00070325"), 703_250); assert_eq!(nusd("7.5e-7"), 750); assert_eq!(nusd("1"), 1_000_000_000); assert_eq!(nusd(""), 0); }
    #[test] fn merge_joins_a_run_and_keeps_every_other_line_in_place() {
        let d = |i: u32, t: &str| format!(r#"{{"type":"stream_event","event":{{"type":"content_block_delta","index":{i},"delta":{{"type":"text_delta","text":"{t}"}}}}}}"#);
        let other = r#"{"type":"assistant","message":{}}"#.to_string();
        let out = merge(vec![d(0, "a"), d(0, "b"), other.clone(), d(0, "c"), d(1, "d"), d(1, "e")]);
        assert_eq!(out.len(), 4); assert_eq!(out[1], other);
        let texts: Vec<String> = [0usize, 2, 3].iter().map(|&i| ev(&out[i])["event"]["delta"]["text"].as_str().unwrap().to_string()).collect();
        assert_eq!(texts, ["ab", "c", "de"]);
    }
    // law delta_merge_exact: the text of the merged lines is the text of the lines, in order
    #[test] fn delta_merge_keeps_all_text_in_order() {
        let mut seed = 12345u64; let mut next = || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 33) as u32 };
        for _ in 0..200 {
            let k = 1 + next() % 40; let mut lines = vec![]; let mut want = String::new();
            for i in 0..k { if next() % 5 == 0 { lines.push(format!(r#"{{"type":"assistant","message":{{"id":"m{i}"}}}}"#)) } else { let t = format!("w{i} "); want += &t; lines.push(format!(r#"{{"type":"stream_event","event":{{"type":"content_block_delta","index":{},"delta":{{"type":"text_delta","text":"{t}"}}}}}}"#, next() % 2)) } }
            let others_in: Vec<&String> = lines.iter().filter(|l| !l.starts_with("{\"type\":\"stream_event\"")).collect();
            let out = merge(lines.clone());
            let got: String = out.iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|o| s(o, "type") == "stream_event").map(|o| s(&o["event"]["delta"], "text").to_string()).collect();
            assert_eq!(got, want); assert!(out.len() <= lines.len());
            let others_out: Vec<&String> = out.iter().filter(|l| !l.starts_with("{\"type\":\"stream_event\"")).collect(); assert_eq!(others_in, others_out);
        }
    }
    #[test] fn pacing_waits_after_a_read_that_ends_in_a_stream_event() { assert!(streaming(&["{\"type\":\"stream_event\"".into()])); assert!(!streaming(&["{\"type\":\"result\"}".into()])); assert!(!streaming(&[])); }
}

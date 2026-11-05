//! Claude's stream-json lines -> the events the JS hub writes (src/core/hub.bend: Claude.line.k,
//! Claude.assistant, Claude.result, Usage.*, Turn.claude, Speed.*, Mu.*, Lim.claude). Pure: a `Cx` (the
//! thread's slice of the read model these rules look at) and a line in, events and live text out.
//! Not ported (said, not silent): subagent lines (`parent_tool_use_id`), background tasks (`Bg`), todo tools,
//! tool-call tracking for deploys, the allowance stream, thinking blocks while `Think` is on is ported.
use serde_json::{json, Value};

pub fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
pub fn n(v: &Value, k: &str) -> u64 { v.get(k).and_then(Value::as_u64).unwrap_or(0) }
/// J.Json.as_raw: the text of a JSON number ("" when absent), as the Bend hub reads it.
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
}

#[derive(Debug, Default, PartialEq)]
pub struct Fx { pub events: Vec<Value>, pub deltas: Vec<String>, pub delta_end: bool }

fn setting(k: String, v: String, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
pub fn hub_id(prefix: &str, now: u64, seq: u64) -> String { format!("{prefix}{now}-{seq}") }

/// Claude.tool_summary.any / TodoWrite is not ported (its summary reads the list).
pub fn tool_summary(name: &str, input: &Value) -> String {
    let pick = |k: &str| input.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string);
    pick("command").or_else(|| pick("file_path")).or_else(|| pick("path")).or_else(|| pick("pattern")).or_else(|| pick("description")).or_else(|| pick("question")).unwrap_or_else(|| name.to_string())
}

/// Claude.uid: a line's uuid adds its first 8 characters to a block's id.
fn uid(uuid: &str) -> String { if uuid.is_empty() { String::new() } else { format!(":{}", uuid.chars().take(8).collect::<String>()) } }

impl Cx {
    pub fn new(thread: &str) -> Cx { Cx { thread: thread.into(), ..Default::default() } }

    fn used_event(&mut self, used: u64, at: u64, out: &mut Vec<Value>) {
        if used > 0 && used != self.used { self.used = used; out.push(json!({"$":"UsageSet","thread":self.thread,"used":used,"window":0,"at":at})) }
    }

    /// Claude.init: the session id, recorded once; a new session's cost and speed start at 0.
    fn init(&mut self, o: &Value, at: u64, out: &mut Vec<Value>) {
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
                    out.push(json!({"$":"ActivityLogged","thread":self.thread,"act":id,"tone":"tool","kind":name,"summary":tool_summary(name, b.get("input").unwrap_or(&Value::Null)),"at":at}));
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
            let api = n(o, "duration_api_ms"); let outt: u64 = o.get("modelUsage").and_then(Value::as_object).map_or(0, |m| m.values().map(|v| n(v, "outputTokens")).sum());
            let old = self.speed.clone(); let known = !old.is_empty();
            let rise = |nowv: u64, before: u64| if nowv >= before { nowv - before } else { nowv };
            let a = if known { rise(api, part(&old, 0)) } else { 0 }; let oo = if known { rise(outt, part(&old, 1)) } else { 0 };
            let told = a > 0 && oo > 0;
            let t = |x: u64| if told { x } else { 0 };
            let v = [api, outt, t(a), t(oo), n(o, "duration_ms"), part(&old, 5) + t(a), part(&old, 6) + t(oo), n(o, "num_turns")].iter().map(u64::to_string).collect::<Vec<_>>().join("/");
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

    /// One main-agent line. `at` is the clock in seconds; `ms` is only for live-buffer ids (not here).
    pub fn line(&mut self, o: &Value, at: u64) -> Fx {
        let mut fx = Fx::default();
        if !s(o, "parent_tool_use_id").is_empty() { return fx } // a subagent's line: not ported
        self.seq += 1;
        match s(o, "type") {
            "system" => {
                if s(o, "subtype") == "init" { let mut ev = vec![]; self.init(o, at, &mut ev); fx.events = ev }
                else if s(o, "subtype") == "compact_boundary" { let post = o.get("compact_metadata").map_or(0, |m| n(m, "post_tokens")); let mut ev = vec![]; self.used_event(post, at, &mut ev); fx.events = ev }
            }
            "stream_event" => if self.running {
                let ev = o.get("event").unwrap_or(&Value::Null); let d = ev.get("delta").unwrap_or(&Value::Null);
                if s(ev, "type") == "content_block_delta" && s(d, "type") == "text_delta" { fx.deltas.push(s(d, "text").to_string()) }
            },
            "assistant" => {
                let m = o.get("message").unwrap_or(&Value::Null); let mut ev = vec![];
                self.blocks(m, s(o, "uuid"), at, &mut ev);
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
                if self.running {
                    if o.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
                        ev.push(json!({"$":"ActivityLogged","thread":self.thread,"act":hub_id("e", at, 0),"tone":"error","kind":"result","summary":Cx::why(o),"at":at}));
                        ev.push(json!({"$":"TurnChanged","thread":self.thread,"state":"failed","at":at}));
                    } else { ev.push(json!({"$":"TurnChanged","thread":self.thread,"state":"completed","at":at})) }
                    self.running = false;
                }
                self.window(o, at, &mut ev); self.turn_usage(o, at, &mut ev);
                fx.events = ev;
            }
            "rate_limit_event" => { let mut ev = vec![]; self.limits(o, at, &mut ev); fx.events = ev }
            _ => {}
        }
        fx
    }
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
    #[test] fn subagent_lines_log_nothing_here() {
        let mut c = Cx::new("t1"); c.running = true;
        assert_eq!(run(&mut c, r#"{"type":"assistant","parent_tool_use_id":"x","message":{"id":"m","content":[{"type":"text","text":"sub"}]}}"#), Fx::default());
    }
    #[test] fn the_result_records_window_turn_usage_speed_and_models_once() {
        let mut c = Cx::new("t1"); c.running = true; c.tok_basis = "c:0".into();
        let l = r#"{"type":"result","subtype":"success","is_error":false,"uuid":"u1","duration_ms":10,"duration_api_ms":7,"num_turns":1,"total_cost_usd":0.0123,"usage":{"input_tokens":1200,"cache_creation_input_tokens":300,"cache_read_input_tokens":4000,"output_tokens":450},"modelUsage":{"m":{"inputTokens":10,"outputTokens":5,"contextWindow":200000}}}"#;
        let a = run(&mut c, l); let ks: Vec<&str> = a.events.iter().map(|e| e["$"].as_str().unwrap()).collect();
        assert_eq!(ks, ["TurnChanged", "UsageSet", "SettingSet", "SettingSet", "TurnUsage", "SettingSet", "SettingSet"]);
        let tu = &a.events[4]; assert_eq!(tu["cost"], 12_300_000u64); assert_eq!(tu["known"], "irwoc"); assert_eq!(tu["basis"], "c:12300000");
        assert!(run(&mut c, l).events.iter().all(|e| e["$"] != "TurnUsage")); // a result line seen again is not counted twice
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
    // law delta_merge_exact (hub.bend Delta.merge): the text of the merged lines is the text of the lines, in order
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

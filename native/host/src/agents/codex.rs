//! Codex (`codex exec --json`): one process per turn, its lines in "Codex shapes".
//! Grok, Muse, OpenCode, Antigravity, Pi and Oh My Pi report in the same shapes through their own adapters.
use serde_json::{json, Value};
use super::lines::{hub_id, n, s, Cx, Fx};

fn setting(k: String, v: String, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
/// Codex.sandbox
pub fn sandbox(runtime: &str) -> Vec<String> {
    match runtime { "full-access" | "" => vec!["--dangerously-bypass-approvals-and-sandbox".into()], "approval-required" => vec!["-s".into(), "read-only".into()], _ => vec!["-s".into(), "workspace-write".into()] }
}
pub struct ArgsIn<'a> { pub model: &'a str, pub session: &'a str, pub runtime: &'a str, pub effort: &'a str, pub think: bool, pub images: &'a [String], pub mcp_url: &'a str, pub pcbparts: &'a str, pub prompt: &'a str, pub text: &'a str, /** the list's servers (mcpx gen::codex) */ pub extra: &'a [String] }
/// The env var the Codex process carries the thread's MCP key in.
pub const BEARER_VAR: &str = "BACKPLANE_MCP_BEARER_TOKEN";
/// `url?k=KEY` as (`url`, `KEY`); no key: (url, "").
pub fn split_key(url: &str) -> (&str, &str) { match url.split_once("?k=") { Some((u, k)) => (u, k), None => (url, "") } }
/// What Codex's env carries for the thread's MCP key (nothing when the url has none).
pub fn bearer_env(url: &str) -> Vec<(String, String)> { let k = split_key(url).1; if k.is_empty() { vec![] } else { vec![(BEARER_VAR.to_string(), k.to_string())] } }
/// Codex.args: head (fresh or resume), sandbox, model, effort, summaries, images, the MCP servers, the instructions, the text.
pub fn args(a: &ArgsIn) -> Vec<String> {
    let mut v: Vec<String> = if a.session.is_empty() { vec!["exec".into(), "--json".into(), "--skip-git-repo-check".into()] } else { vec!["exec".into(), "resume".into(), a.session.into(), "--json".into(), "--skip-git-repo-check".into()] };
    v.extend(sandbox(a.runtime));
    if !a.model.is_empty() { v.push("-m".into()); v.push(a.model.into()) }
    if ["low", "medium", "high", "xhigh"].contains(&a.effort) { v.push("-c".into()); v.push(format!("model_reasoning_effort=\"{}\"", a.effort)) }
    if a.think { v.push("-c".into()); v.push("model_reasoning_summary=\"detailed\"".into()) }
    for i in a.images { v.push("--image".into()); v.push(i.clone()) }
    // the key is not in argv (F-009): the bare endpoint here, the key in the env var Codex sends as `Authorization: Bearer` (`bearer_env`)
    v.push("-c".into()); v.push(format!("mcp_servers.backplane.url=\"{}\"", split_key(a.mcp_url).0));
    v.push("-c".into()); v.push(format!("mcp_servers.backplane.bearer_token_env_var=\"{BEARER_VAR}\""));
    if !a.pcbparts.is_empty() { v.push("-c".into()); v.push(format!("mcp_servers.pcbparts.url=\"{}\"", a.pcbparts)); }
    v.extend(a.extra.iter().cloned());
    v.push("-c".into()); v.push(format!("developer_instructions={}", Value::String(a.prompt.into())));
    v.push(a.text.into()); v
}

/// Codex.kind: the line types read (others are ignored).
fn todo_data(it: &Value) -> String {
    let xs: Vec<Value> = it.get("items").and_then(Value::as_array).map(|a| a.iter().enumerate().map(|(i, x)| {
        let st = if x.get("completed").and_then(Value::as_bool).unwrap_or(false) { "completed" } else { let t = s(x, "status"); if t == "completed" || t == "in_progress" { t } else { "pending" } };
        json!({"id":(i + 1).to_string(),"text":s(x, "text"),"form":"","status":st}) }).collect()).unwrap_or_default();
    Value::Array(xs).to_string()
}

impl Cx {
    /// Turn.codex: a turn.completed's usage once (by session and totals), against the session's totals before it.
    fn codex_usage(&mut self, o: &Value, provider: &str, model: &str, at: u64, out: &mut Vec<Value>) {
        let u = o.get("usage").unwrap_or(&Value::Null);
        if u.get("output_tokens").is_none() { return }
        // the model the provider says ran the turn, when it says (grok's `_meta.modelId`), else the thread's
        let model = u.get("model").and_then(Value::as_str).filter(|m| !m.is_empty()).unwrap_or(model);
        // ACP's usage is the turn's own, not the session's running totals: counted as it is, the totals' basis untouched
        if u.get("turn").and_then(Value::as_bool) == Some(true) {
            let (ci, cc, cw, co, ct) = (n(u, "input_tokens"), n(u, "cached_input_tokens"), n(u, "cache_write_input_tokens"), n(u, "output_tokens"), n(u, "reasoning_output_tokens"));
            let key = format!("t:{}:{at}:{ci}/{co}", self.session); if key == self.tok_key { return }
            out.push(json!({"$":"TurnUsage","thread":self.thread,"provider":provider,"model":if model.is_empty() { "default" } else { model },"key":key,
                "input":ci.saturating_sub(cc),"cread":cc,"cwrite":cw,"output":co,"reasoning":ct,"cost":0,"known":format!("irwo{}", if u.get("reasoning_output_tokens").is_some() { "t" } else { "" }),"at":at}));
            self.tok_key = key; return
        }
        let (ci, cc, cw, co, ct) = (n(u, "input_tokens"), n(u, "cached_input_tokens"), n(u, "cache_write_input_tokens"), n(u, "output_tokens"), n(u, "reasoning_output_tokens"));
        let se = self.session.clone(); let key = format!("x:{se}:{ci}/{co}");
        if key == self.tok_key { return }
        if !self.tok_epoch_set { self.tok_epoch_set = true; out.push(setting("tok.epoch".into(), "1".into(), at)) }
        if !self.tok_recount_set { self.tok_recount_set = true; out.push(setting("tok.recount".into(), "b".into(), at)) }
        let cost = lines_nusd(u.get("cost_usd"));
        let pre = format!("x:{se}:"); let seen = self.tok_basis.starts_with(&pre);
        let was: Vec<u64> = if seen { self.tok_basis[pre.len()..].split('/').map(super::lines::num).collect() } else { vec![] };
        let w = |i: usize| was.get(i).copied().unwrap_or(0);
        let basis = format!("x:{se}:{}", [ci, cc, cw, co, ct, cost].iter().map(u64::to_string).collect::<Vec<_>>().join("/"));
        let model = if model.is_empty() { "default" } else { model };
        if !seen {
            out.push(json!({"$":"TurnUsage","thread":self.thread,"provider":provider,"model":model,"key":key,"input":0,"cread":0,"cwrite":0,"output":0,"reasoning":0,"cost":0,"known":"","basis":basis,"at":at}));
        } else {
            let up = ci >= w(0) && cc >= w(1) && cw >= w(2) && co >= w(3) && ct >= w(4);
            let d = |i: usize, v: u64| if up { v - w(i) } else { v };
            let hascost = u.get("cost_usd").is_some();
            out.push(json!({"$":"TurnUsage","thread":self.thread,"provider":provider,"model":model,"key":key,
                "input":d(0, ci).saturating_sub(d(1, cc)),"cread":d(1, cc),"cwrite":d(2, cw),"output":d(3, co),"reasoning":d(4, ct),"cost":if hascost { d(5, cost) } else { 0 },
                "known":format!("irwo{}{}", if u.get("reasoning_output_tokens").is_some() { "t" } else { "" }, if hascost { "c" } else { "" }),"basis":basis,"at":at}));
        }
        self.tok_key = key; self.tok_basis = basis;
    }

    /// Codex.line.k (Hub.agent_line for a Codex-shaped provider). `provider` and `model` are the thread's; `seq` numbers this thread's ids.
    pub fn codex_line(&mut self, o: &Value, provider: &str, model: &str, at: u64) -> Fx {
        let th = self.thread.clone(); let mut fx = Fx::default(); let out = &mut fx.events;
        match s(o, "type") {
            "thread.started" => {
                let sid = s(o, "thread_id"); let same = self.session == sid;
                if !same { out.push(json!({"$":"SessionSet","id":th,"session":sid,"at":at})); self.session = sid.to_string(); } // Claude.init.k only; Codex adds no c:0 basis or speed reset
                if !same { let b = format!("x:{sid}:0/0/0/0/0"); self.tok_basis = b.clone(); out.push(setting(format!("tok.basis.{th}"), b, at)) }
            }
            "item.completed" => {
                let it = o.get("item").unwrap_or(&Value::Null); self.seq += 1; let seq = self.seq; let id = s(it, "id");
                match s(it, "type") {
                    "agent_message" => { out.push(json!({"$":"MessagePosted","thread":th,"msg":format!("{}-{seq}", if id.is_empty() { hub_id("c", at, seq) } else { id.to_string() }),"role":"assistant","text":s(it, "text"),"at":at})); fx.delta_end = true }
                    "command_execution" => out.push(json!({"$":"ActivityLogged","thread":th,"act":format!("{}-{seq}", if id.is_empty() { hub_id("x", at, seq) } else { id.to_string() }),"tone":"tool","kind":"shell","summary":s(it, "command"),"at":at})),
                    "file_change" => out.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("f", at, seq),"tone":"tool","kind":"edit","summary":it.get("changes").map(|c| c.to_string()).unwrap_or_else(|| "null".into()),"at":at})),
                    "todo_list" => out.push(json!({"$":"TodosChanged","thread":th,"who":"agent","data":todo_data(it),"at":at})),
                    "reasoning" if self.think && !s(it, "text").trim().is_empty() => out.push(json!({"$":"ActivityLogged","thread":th,"act":format!("think:{}-{seq}", if id.is_empty() { hub_id("r", at, seq) } else { id.to_string() }),"tone":"thinking","kind":"thinking","summary":s(it, "text"),"at":at})),
                    _ => {}
                }
            }
            "item.started" | "item.updated" => { let it = o.get("item").unwrap_or(&Value::Null); if s(it, "type") == "todo_list" { out.push(json!({"$":"TodosChanged","thread":th,"who":"agent","data":todo_data(it),"at":at})) } }
            "turn.completed" => { out.push(json!({"$":"TurnChanged","thread":th,"state":"completed","at":at})); self.codex_usage(o, provider, model, at, out); fx.delta_end = true }
            "turn.failed" => {
                let m = o.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).or_else(|| o.get("message").and_then(Value::as_str)).unwrap_or("turn failed");
                self.seq += 1;
                out.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("e", at, self.seq),"tone":"error","kind":provider,"summary":m,"at":at}));
                out.push(json!({"$":"TurnChanged","thread":th,"state":"failed","at":at})); fx.delta_end = true;
            }
            "item.delta" => { let t = s(o, "text"); if !t.is_empty() { fx.deltas.push(t.to_string()) } }
            "notice" => { self.seq += 1; out.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("n", at, self.seq),"tone":"info","kind":"session","summary":s(o, "message"),"at":at})) }
            "error" => { self.seq += 1; out.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("n", at, self.seq),"tone":"info","kind":"provider","summary":error_text(o),"at":at})) }
            "context.usage" => { let used = n(o, "used"); if used > 0 { out.push(json!({"$":"UsageSet","thread":th,"used":used,"window":n(o, "window"),"at":at})) } }
            _ => {}
        }
        fx
    }
}

fn lines_nusd(v: Option<&Value>) -> u64 { match v { Some(Value::Number(x)) => super::lines::nusd(&x.to_string()), Some(Value::String(x)) => super::lines::nusd(x), _ => 0 } }

/// Codex.error.text: a bounded public notice, never the raw transport text.
pub fn error_text(o: &Value) -> &'static str {
    let m = o.get("message").and_then(Value::as_str).or_else(|| o.get("error").and_then(|e| e.get("message")).and_then(Value::as_str)).unwrap_or("").to_lowercase();
    if m.contains("too many requests") || m.contains("http 429") || m.contains("status: 429") { "Provider is rate limited (HTTP 429). Waiting for the provider to retry; your message is kept." }
    else if m.contains("reconnecting") { "Provider connection interrupted. The provider is retrying; your message is kept." }
    else { "Provider reported an error. Your message is kept; waiting for the provider's next result." }
}

/// A line that ends a Codex tool call: its step waits for it (tool.done), or it is a completed shell/edit item.
pub fn step_for(o: &Value) -> Option<String> {
    match s(o, "type") { "tool.done" => Some(s(o, "id").to_string()).filter(|x| !x.is_empty()), _ => None }
}

// ---- Riding mail: a message for a running Codex turn waits as the setting `ride.<thread>` ----
pub fn ride_read(v: &str) -> Vec<(String, String)> {
    serde_json::from_str::<Value>(v).ok().and_then(|j| j.as_array().map(|a| a.iter().map(|x| (s(x, "id").to_string(), s(x, "text").to_string())).collect())).unwrap_or_default()
}
pub fn ride_show(xs: &[(String, String)]) -> String { if xs.is_empty() { String::new() } else { Value::Array(xs.iter().map(|(i, t)| json!({"id":i,"text":t})).collect()).to_string() } }
/// Ride.add: once (a message already there is not added again). The mailroom's `told:` notes (bots/mail/escalate.rs `private_tell`: "your private mailroom just told
/// the person this") fold into ONE waiting entry, the newest's id: they start no turn, so before Nov 20 they waited until the person wrote and arrived 110 and 41 at a time.
pub fn ride_add(v: &str, id: &str, text: &str) -> String {
    let mut xs = ride_read(v); if xs.iter().any(|(i, _)| i == id) { return ride_show(&xs) }
    if !id.starts_with("told:") { xs.push((id.into(), text.into())); return ride_show(&xs) }
    let (mut earlier, mut told) = (0, vec![]);
    for t in xs.iter().filter(|(i, _)| i.starts_with("told:")).map(|(_, t)| t.as_str()).chain([text]) { let (n, mut ys) = told_items(t); earlier += n; told.append(&mut ys) }
    xs.retain(|(i, _)| !i.starts_with("told:"));
    let text = if told.len() == 1 && earlier == 0 { text.to_string() } else { told_fold(earlier, &told) };
    xs.push((id.into(), text)); ride_show(&xs)
}
const TOLD_HEAD: &str = "[Backplane] Your private mailroom told the person these in your conversation since your last turn. They are on screen there, and no phone alert went out: only your notify_person reaches their phone, so call it if they must know now. Do not repeat them otherwise, take them into account. If you have nothing to add, end with an empty reply.";
const TOLD_KEEP: usize = 8;
/// a told note's earlier count and told text(s): a single note's body after its first blank line, a folded note's items
fn told_items(t: &str) -> (usize, Vec<String>) {
    let Some(r) = t.strip_prefix(TOLD_HEAD) else { return (0, vec![t.split_once("\n\n").map_or(t, |(_, b)| b).trim().to_string()]) };
    let mut n = 0; let mut out = vec![];
    for x in r.split("\n\n- ").map(str::trim) {
        if let Some(c) = x.strip_prefix("(and ").and_then(|c| c.split(' ').next()).and_then(|c| c.parse::<usize>().ok()) { n = c } else if !x.is_empty() { out.push(x.to_string()) }
    }
    (n, out)
}
fn told_fold(earlier: usize, xs: &[String]) -> String {
    let skip = xs.len().saturating_sub(TOLD_KEEP); let one = |x: &String| x.chars().take(600).collect::<String>().replace("\n\n", "\n");
    let more = if earlier + skip > 0 { format!("\n\n- (and {} earlier, in your conversation)", earlier + skip) } else { String::new() };
    format!("{TOLD_HEAD}{more}\n\n{}", xs[skip..].iter().map(|x| format!("- {}", one(x))).collect::<Vec<_>>().join("\n\n"))
}

pub fn ride_input(v: &str) -> String { ride_read(v).into_iter().map(|(_, t)| t).collect::<Vec<_>>().join("\n\n") }
pub fn ride_block(v: &str) -> String {
    format!("[Backplane] Incoming messages, sent to you while this turn ran. Each is shown to you once; take them into account now:\n\n{}", ride_read(v).into_iter().map(|(_, t)| t).collect::<Vec<_>>().join("\n\n---\n\n"))
}
/// Ride.done: the ride emptied, and the log told how much went where.
pub fn ride_done(th: &str, v: &str, into: &str, at: u64) -> Vec<Value> {
    let xs = ride_read(v); let last = xs.last().map(|x| x.0.clone()).unwrap_or_default();
    vec![setting(format!("ride.{th}"), String::new(), at), json!({"$":"ActivityLogged","thread":th,"act":format!("ride:{last}"),"tone":"info","kind":"ride","summary":format!("Delivered {into}: {} waiting message(s)", xs.len()),"at":at})]
}
pub fn ride_last(v: &str) -> String { ride_read(v).last().map(|x| x.0.clone()).unwrap_or_default() }

#[cfg(test)]
mod tests {
    use super::*;
    fn ev(l: &str) -> Value { serde_json::from_str(l).unwrap() }
    #[test] fn codex_argv_has_no_mcp_key_but_its_env_does() {
        let canary = "CANARY0123456789abcdef"; let url = format!("http://127.0.0.1:9/mcp/t1.2?k={canary}");
        let a = ArgsIn { model: "m", session: "", runtime: "auto", effort: "", think: false, images: &[], mcp_url: &url, pcbparts: "https://p", prompt: "P", text: "hi", extra: &[] };
        let v = args(&a); assert!(!v.iter().any(|x| x.contains(canary) || x.contains("?k=")), "{v:?}");
        assert!(v.contains(&"mcp_servers.backplane.url=\"http://127.0.0.1:9/mcp/t1.2\"".to_string()) && v.contains(&format!("mcp_servers.backplane.bearer_token_env_var=\"{BEARER_VAR}\"")));
        assert_eq!(bearer_env(&url), vec![(BEARER_VAR.to_string(), canary.to_string())]); assert!(bearer_env("http://u").is_empty());
    }
    #[test] fn args_fresh_and_resumed() {
        let a = ArgsIn { model: "m", session: "", runtime: "auto", effort: "high", think: true, images: &["/i.png".into()], mcp_url: "http://u", pcbparts: "https://p", prompt: "P\"q", text: "hi", extra: &[] };
        let v = args(&a); assert_eq!(&v[..3], ["exec", "--json", "--skip-git-repo-check"]); assert_eq!(v.last().unwrap(), "hi");
        assert!(v.windows(2).any(|w| w == ["-s", "workspace-write"])); assert!(v.windows(2).any(|w| w == ["-m", "m"])); assert!(v.windows(2).any(|w| w == ["--image", "/i.png"]));
        assert!(v.contains(&"developer_instructions=\"P\\\"q\"".to_string())); assert!(v.contains(&"mcp_servers.backplane.url=\"http://u\"".to_string()));
        let b = args(&ArgsIn { session: "S", runtime: "full-access", ..a }); assert_eq!(&b[..5], ["exec", "resume", "S", "--json", "--skip-git-repo-check"]); assert!(b.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
    }
    #[test] fn a_turn_in_codex_shapes() {
        let mut c = Cx::new("t1"); c.running = true;
        let a = c.codex_line(&ev(r#"{"type":"thread.started","thread_id":"S1"}"#), "codex", "m", 10);
        assert_eq!(a.events[0]["$"], "SessionSet"); assert!(a.events.iter().any(|e| e["key"] == "tok.basis.t1" && e["value"] == "x:S1:0/0/0/0/0"));
        let b = c.codex_line(&ev(r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"hello"}}"#), "codex", "m", 11);
        assert_eq!(b.events[0]["msg"], "i1-1"); assert!(b.delta_end);
        let sh = c.codex_line(&ev(r#"{"type":"item.completed","item":{"id":"i2","type":"command_execution","command":"ls"}}"#), "codex", "m", 12);
        assert_eq!(sh.events[0]["kind"], "shell"); assert_eq!(sh.events[0]["act"], "i2-2");
        let d = c.codex_line(&ev(r#"{"type":"item.delta","text":"he"}"#), "codex", "m", 12); assert_eq!(d.deltas, vec!["he"]);
        let t = c.codex_line(&ev(r#"{"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":40,"output_tokens":10}}"#), "codex", "m", 13);
        assert_eq!(t.events[0]["state"], "completed"); let u = t.events.iter().find(|e| e["$"] == "TurnUsage").unwrap();
        assert_eq!((u["input"].as_u64(), u["cread"].as_u64(), u["output"].as_u64()), (Some(60), Some(40), Some(10)));
        assert_eq!(u["basis"], "x:S1:100/40/0/10/0/0");
        let t2 = c.codex_line(&ev(r#"{"type":"turn.completed","usage":{"input_tokens":250,"cached_input_tokens":100,"output_tokens":30}}"#), "codex", "m", 14);
        let u2 = t2.events.iter().find(|e| e["$"] == "TurnUsage").unwrap(); assert_eq!((u2["input"].as_u64(), u2["cread"].as_u64(), u2["output"].as_u64()), (Some(90), Some(60), Some(20)));
        let same = c.codex_line(&ev(r#"{"type":"turn.completed","usage":{"input_tokens":250,"cached_input_tokens":100,"output_tokens":30}}"#), "codex", "m", 15);
        assert!(same.events.iter().all(|e| e["$"] != "TurnUsage"), "the same totals count once");
    }
    #[test] fn failures_todos_notices() {
        let mut c = Cx::new("t1");
        let f = c.codex_line(&ev(r#"{"type":"turn.failed","error":{"message":"boom"}}"#), "codex", "m", 1);
        assert_eq!(f.events[0]["summary"], "boom"); assert_eq!(f.events[0]["kind"], "codex"); assert_eq!(f.events[1]["state"], "failed");
        let t = c.codex_line(&ev(r#"{"type":"item.started","item":{"type":"todo_list","items":[{"text":"a","completed":true},{"text":"b","completed":false}]}}"#), "codex", "m", 2);
        assert_eq!(t.events[0]["data"], r#"[{"id":"1","text":"a","form":"","status":"completed"},{"id":"2","text":"b","form":"","status":"pending"}]"#);
        let e = c.codex_line(&ev(r#"{"type":"error","message":"HTTP 429 slow down"}"#), "codex", "m", 3); assert!(e.events[0]["summary"].as_str().unwrap().contains("rate limited"));
        assert!(!e.events[0]["summary"].as_str().unwrap().contains("slow down"));
        let u = c.codex_line(&ev(r#"{"type":"context.usage","used":5,"window":100}"#), "codex", "m", 4); assert_eq!(u.events[0]["used"], 5);
    }
    #[test] fn the_ride_holds_each_message_once_and_empties() {
        let v = ride_add("", "m1", "one"); let v = ride_add(&v, "m2", "two"); assert_eq!(ride_add(&v, "m1", "again"), v);
        assert_eq!(ride_input(&v), "one\n\ntwo"); assert!(ride_block(&v).ends_with("one\n\n---\n\ntwo"));
        let d = ride_done("t", &v, "into the running turn", 5); assert_eq!(d[0]["value"], ""); assert_eq!(d[1]["act"], "ride:m2"); assert_eq!(d[1]["summary"], "Delivered into the running turn: 2 waiting message(s)");
    }
}

#[cfg(test)]
mod laws {
    use super::*;
    #[test] fn ride_kept() { assert_eq!(ride_input(&ride_add("", "m1", "hi")), "hi") }
    #[test] fn the_mailrooms_told_notes_wait_as_one_entry_however_many_come() {
        let note = |i: usize| format!("[Backplane] Your private mailroom just told the person this in your conversation (key k{i}). It is on screen.\n\nLanded on main: PR #{i}.");
        let mut v = ride_add("", "m1", "a person's message");
        for i in 0..41 { v = ride_add(&v, &format!("told:tell:{i}"), &note(i)) }
        let xs = ride_read(&v); assert_eq!(xs.len(), 2, "the person's message and one told entry: {xs:?}");
        assert_eq!(xs[1].0, "told:tell:40", "the newest's id");
        assert!(xs[1].1.starts_with(TOLD_HEAD) && xs[1].1.contains("Landed on main: PR #40.") && xs[1].1.contains("PR #33.") && !xs[1].1.contains("PR #32."), "{}", xs[1].1);
        assert!(xs[1].1.contains("(and 33 earlier, in your conversation)"), "{}", xs[1].1);
        let one = ride_add("", "told:tell:x", &note(7)); assert_eq!(ride_read(&one)[0].1, note(7), "a single note is kept as it came");
        assert_eq!(ride_add(&v, "told:tell:40", "again"), v, "once per id");
    }
    #[test] fn ride_taken_empties() { let d = ride_done("t1", &ride_add("", "m1", "hi"), "into the running turn", 6); assert_eq!(d[0]["key"], "ride.t1"); assert_eq!(d[0]["value"], "") }
    #[test] fn ride_carries_block() { assert!(ride_block(&ride_add("", "m1", "hi")).contains("hi")) }
    #[test] fn codex_error_notice_kind() { let mut c = Cx::new("t"); let fx = c.codex_line(&json!({"type":"error","message":"x"}), "codex", "", 1); assert_eq!(fx.events[0]["kind"], "provider"); assert_eq!(fx.events[0]["tone"], "info"); assert!(fx.events.iter().all(|e| e["$"] == "ActivityLogged"), "codex_error_notice_no_effects") }
}

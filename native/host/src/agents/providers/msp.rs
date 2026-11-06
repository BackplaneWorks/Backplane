//! MSP, the Muse Session Protocol (JSON-RPC over stdio, `muse serve`): a port of src/core/msp.bend. One process per turn:
//! initialize, the initialized note, session/start (or session/resume, then session/setApprovalMode), turn/start.
//! What Muse says becomes Codex shapes (`{"type":"item.delta",...}`), as in acp.bend. Pure: no IO here.
use serde_json::{json, Value};

/// MspTurn: what a turn needs. `nonce` is the turn's gen: Muse refuses a command id it has seen, in any session.
#[derive(Clone, Debug, Default)]
pub struct MspTurn { pub cwd: String, pub mcp: String, pub prompt: String, pub text: String, pub session: String, pub mode: String, pub effort: String, pub nonce: String }

/// AcpOut: lines to write to Muse, shapes for the log, approval requests for the person (the raw request line each).
#[derive(Debug, Default)]
pub struct Out { pub send: Vec<String>, pub emit: Vec<Value>, pub asks: Vec<String> }

/// The turn's state (Acp{sid, done}); `buf` is unused by MSP, which streams and posts whole messages.
#[derive(Debug, Default, Clone)]
pub struct Msp { pub sid: String, pub done: bool }

fn gs<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn or<'a>(a: &'a str, b: &'a str) -> &'a str { if a.is_empty() { b } else { a } }
pub fn event(ty: &str, kvs: Value) -> Value { let mut m = serde_json::Map::new(); m.insert("type".into(), json!(ty)); if let Value::Object(o) = kvs { for (k, v) in o { m.insert(k, v); } } Value::Object(m) }
pub fn failed(msg: &str) -> Value { event("turn.failed", json!({"message": msg})) }
pub fn rpc(id: u64, method: &str, params: Value) -> String { json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string() }
pub fn note(method: &str, params: Value) -> String { json!({"jsonrpc":"2.0","method":method,"params":params}).to_string() }

// ---- Command ids ----
fn digits(s: &str) -> String { s.chars().filter(|c| c.is_ascii_digit()).collect() }
/// Msp.pad: exactly n characters: zeros before it, or its last n.
fn pad(s: &str, n: usize) -> String { let l = s.chars().count(); if l >= n { s.chars().skip(l - n).collect() } else { "0".repeat(n - l) + s } }
fn cut(x: &str, a: usize, n: usize) -> String { x.chars().skip(a).take(n).collect() }
/// Msp.fmt: 32 hex digits as a UUID.
fn fmt(x: &str) -> String { format!("{}-{}-{}-{}-{}", cut(x, 0, 8), cut(x, 8, 4), cut(x, 12, 4), cut(x, 16, 4), cut(x, 20, 12)) }
/// Msp.uuid: 32 digits from the nonce's digits (padded to 28), the last but one naming the command (1 open, 2 mode, 3 turn).
pub fn uuid(nonce: &str, k: &str) -> String { let x = pad(&digits(nonce), 28); fmt(&format!("{}7{}8{}{}0", cut(&x, 0, 12), cut(&x, 12, 3), cut(&x, 15, 13), k)) }

// ---- Requests ----
/// Msp.init
pub fn init() -> String {
    rpc(1, "initialize", json!({"clientInfo":{"name":"backplane","title":"Backplane","version":"1"},"capabilities":{"requestedCapabilities":["sessionMcp"],"userInputDialogs":false}}))
}
fn config(url: &str) -> Value { json!({"mcpServers":{"backplane":{"transport":"streamableHttp","url":url,"mode":"optional"}}}) }
/// Msp.open: a new session or the thread's own.
pub fn open(t: &MspTurn) -> String {
    if t.session.is_empty() { rpc(2, "session/start", json!({"commandId":uuid(&t.nonce, "1"),"workspaceRoot":t.cwd,"approvalMode":t.mode,"config":config(&t.mcp)})) }
    else { rpc(2, "session/resume", json!({"commandId":uuid(&t.nonce, "1"),"sessionId":t.session,"excludeItems":true,"config":config(&t.mcp)})) }
}
fn say_rpc(sid: &str, t: &MspTurn, text: &str) -> String {
    let mut p = json!({"commandId":uuid(&t.nonce, "3"),"sessionId":sid,"input":[{"type":"text","text":text}],"displayText":t.text});
    if !t.effort.is_empty() { p["reasoningEffort"] = json!(t.effort) }
    rpc(4, "turn/start", p)
}
fn mode_rpc(sid: &str, t: &MspTurn) -> String { rpc(3, "session/setApprovalMode", json!({"commandId":uuid(&t.nonce, "2"),"sessionId":sid,"mode":t.mode})) }
/// Msp.say.k: a new session gets Backplane's instructions first; a resumed one is set to the thread's approval mode (it may have changed) first.
fn say(t: &MspTurn, sid: &str) -> Vec<String> {
    if t.session.is_empty() { vec![say_rpc(sid, t, &format!("<backplane_instructions>\n{}\n</backplane_instructions>\n\n{}", t.prompt, t.text))] }
    else { vec![mode_rpc(sid, t), say_rpc(sid, t, &t.text)] }
}

// ---- Replies to our requests ----
fn error(o: &Value) -> String { o.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).unwrap_or("muse failed").to_string() }
/// Msp.hint: a sign-in problem says how to fix it.
pub fn hint(msg: &str) -> String { let l = msg.to_lowercase(); if l.contains("auth") || l.contains("login") { format!("{msg} (sign in once with `muse login`)") } else { msg.to_string() } }

impl Msp {
    pub fn new() -> Msp { Msp::default() }
    fn fail(&mut self, msg: &str) -> Out { self.done = true; Out { emit: vec![failed(&hint(msg))], ..Default::default() } }
    fn reply(&mut self, t: &MspTurn, o: &Value) -> Out {
        let id = o.get("id").and_then(|v| match v { Value::Number(n) => n.as_u64(), Value::String(s) => s.parse().ok(), _ => None }).unwrap_or(0);
        let ok = o.get("error").map_or(true, Value::is_null);
        match (id, ok) {
            (1, true) => Out { send: vec![note("initialized", json!({})), open(t)], ..Default::default() },
            (2, true) => { let sid = o.get("result").and_then(|r| r.get("session")).and_then(|s| s.get("sessionId")).and_then(Value::as_str).unwrap_or(&t.session).to_string();
                self.sid = sid.clone(); Out { send: say(t, &sid), emit: vec![event("thread.started", json!({"thread_id": sid}))], ..Default::default() } }
            (1, false) | (2, false) | (4, false) => self.fail(&error(o)),
            _ => Out::default(),
        }
    }

    // ---- Notifications: the transcript as it happens ----
    fn note(&mut self, method: &str, p: &Value) -> Out {
        let item = p.get("item").unwrap_or(&Value::Null); let kind = gs(item, "kind");
        let mut out = Out::default();
        match method {
            "item/delta" => if p.get("field").and_then(Value::as_str).unwrap_or("text") == "text" { out.emit.push(event("item.delta", json!({"text": gs(p, "delta")}))) },
            "item/started" => match kind {
                "toolCall" => out.emit.push(row(gs(item, "itemId"), &cmd(item))),
                "subagent" => out.emit.push(row(gs(item, "itemId"), &format!("agent: {}", or(gs(item, "objective"), or(gs(item, "role"), "subagent"))))),
                _ => {} },
            "item/completed" => match kind {
                "agentMessage" => { let t = gs(item, "text"); if !t.trim().is_empty() { out.emit.push(event("item.completed", json!({"item":{"type":"agent_message","text":t}}))) } }
                "toolCall" => out.emit.push(event("tool.done", json!({"id": gs(item, "itemId")}))),
                _ => {} },
            "turn/completed" => { self.done = true;
                if gs(p, "terminal") == "completed" { out.emit.push(event("turn.completed", json!({}))) }
                else { let why = p.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).map(str::to_string).or_else(|| p.get("reason").and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| format!("muse turn {}", { let x = gs(p, "terminal"); if x.is_empty() { "failed" } else { x } }));
                    out.emit.push(failed(&hint(&why))) } }
            "session/todoListChanged" => { let xs: Vec<Value> = p.get("items").and_then(Value::as_array).map(|a| a.iter().map(|x| { let s = x.get("status").and_then(Value::as_str).unwrap_or("pending"); let s = if s == "inProgress" { "in_progress" } else { s }; json!({"text":gs(x, "text"),"status":s}) }).collect()).unwrap_or_default();
                out.emit.push(event("item.updated", json!({"item":{"type":"todo_list","items":xs}}))) }
            _ => {}
        }
        out
    }

    /// Msp.line: one line from Muse. `o` is the parsed JSON (a line that is not JSON says nothing: the caller passes a String).
    pub fn line(&mut self, t: &MspTurn, o: &Value) -> Out {
        if !o.is_object() { return Out::default() }
        let method = gs(o, "method");
        if method.is_empty() { return self.reply(t, o) }
        if let Some(id) = o.get("id") {
            // a server request: an approval is received at once and goes to the person; anything else is not supported
            let id = id.to_string();
            if method == "approval/request" { return Out { send: vec![format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{{}}}}")], asks: vec![o.to_string()], ..Default::default() } }
            return Out { send: vec![format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":-32601,\"message\":\"not supported\"}}}}")], ..Default::default() };
        }
        let p = o.get("params").unwrap_or(&Value::Null);
        // only this session's events, once it is open (a subagent's session and a resume's history are not this turn's)
        if !self.sid.is_empty() && gs(p, "sessionId") == self.sid { self.note(method, p) } else { Out::default() }
    }
}

fn row(id: &str, text: &str) -> Value { event("item.completed", json!({"item":{"type":"command_execution","id":id,"command":text}})) }
fn first(ks: &[&str], o: &Value) -> String { ks.iter().map(|k| gs(o, k)).find(|s| !s.is_empty()).unwrap_or("").to_string() }
/// Msp.cmd: what a tool call ran, for its row: a shell's command, else the tool and what it was pointed at.
pub fn cmd(it: &Value) -> String {
    let tool = it.get("tool").and_then(Value::as_str).unwrap_or("tool");
    let args: Value = serde_json::from_str(gs(it, "args")).unwrap_or(Value::Null);
    let c = gs(&args, "command"); if !c.is_empty() { return c.to_string() }
    let f = first(&["path", "file_path", "pattern", "query", "url"], &args);
    if f.is_empty() { tool.to_string() } else { format!("{tool} {f}") }
}

// ---- Approval answers ----
// "accept" picks the approve-once choice, "session" the approve-for-session one, anything else a denial; each falls back to its neighbour.
fn choice(cs: &[Value], d: &str) -> String { cs.iter().find(|c| gs(c, "decision") == d).map(|c| gs(c, "choiceId").to_string()).unwrap_or_default() }
fn choose(answer: &str, cs: &[Value]) -> String {
    let o = |a: &str, b: &str| { let x = choice(cs, a); if x.is_empty() { choice(cs, b) } else { x } };
    match answer { "accept" => o("approved", "approvedForSession"), "session" => o("approvedForSession", "approved"), _ => o("denied", "abort") }
}
/// Msp.decide: the decision for a parked approval/request line (its raw text), once the person answered.
pub fn decide(body: &str, answer: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null); let p = v.get("params").unwrap_or(&Value::Null);
    let cs: Vec<Value> = p.get("availableChoices").and_then(Value::as_array).cloned().unwrap_or_default();
    let ap = gs(p, "approvalId");
    let seq = p.get("sourceRange").and_then(|r| r.get("last")).and_then(|l| l.get("sequence")).map(|s| match s { Value::String(x) => x.clone(), x => x.to_string() }).unwrap_or_else(|| "0".into());
    let hex: String = ap.chars().filter(|c| *c != '-').collect();
    let cid = fmt(&format!("{}{}", cut(&pad(&hex, 32), 0, 20), pad(&digits(&seq), 12)));
    json!({"jsonrpc":"2.0","id":format!("decide-{ap}"),"method":"approval/decide","params":{"approvalId":ap,"choiceId":choose(answer, &cs),"commandId":cid,
        "requirementId":p.get("currentRequirementId").cloned().unwrap_or(Value::Null),"sessionId":gs(p, "sessionId")}}).to_string()
}
/// Msp.tool: what an approval asks to run (what "Allow for session" remembers).
pub fn tool(p: &Value) -> String {
    let s = p.get("subject").unwrap_or(&Value::Null);
    or(gs(p, "toolName"), or(gs(s, "toolName"), { let k = gs(s, "kind"); if k.is_empty() { "tool" } else { k } })).to_string()
}
/// Msp.detail: a line for the approval card.
pub fn detail(p: &Value) -> String { let d = first(&["command", "path", "target", "host"], p.get("subject").unwrap_or(&Value::Null)); if d.is_empty() { gs(p, "rawArgs").chars().take(400).collect() } else { d } }

/// Msp.mode.k: supervised asks for anything Muse's policy does not already allow; accept-edits and auto let Muse decide; full access allows everything.
pub fn mode_k(i: u8) -> &'static str { match i { 0 => "promptUnmatched", 3 => "allowAll", _ => "onRequest" } }
/// Muse.mode: the thread's runtime mode ("approval-required", "auto-accept-edits", "auto", "full-access"; anything else is full access, Runtime.of).
pub fn mode(runtime: &str) -> &'static str { mode_k(match runtime { "approval-required" => 0, "auto-accept-edits" => 1, "auto" => 2, _ => 3 }) }

#[cfg(test)]
mod tests {
    use super::*;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    fn turn(sid: &str) -> MspTurn { MspTurn { cwd: "/tmp/p".into(), mcp: "http://127.0.0.1:1/mcp/t?token=x".into(), prompt: "RULES".into(), text: "hello".into(), session: sid.into(), mode: "promptUnmatched".into(), effort: "".into(), nonce: "1791825557-42".into() } }
    fn turn_hi(sid: &str) -> MspTurn { MspTurn { mode: "allowAll".into(), effort: "high".into(), nonce: "1791825557-43".into(), ..turn(sid) } }
    fn m(l: &str) -> String { j(l)["method"].as_str().unwrap_or("").to_string() }
    fn p(l: &str) -> Value { j(l)["params"].clone() }
    fn emits(o: &Out) -> String { o.emit.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" | ") }
    const INIT_OK: &str = r#"{"id":1,"jsonrpc":"2.0","result":{"experimentalApi":false,"grantedCapabilities":["sessionMcp"],"serverInfo":{"name":"muse","version":"1.4.6"}}}"#;
    fn opened(sid: &str, t: &MspTurn) -> (Msp, Out) { let mut a = Msp::new(); a.line(t, &j(INIT_OK)); let o = a.line(t, &j(&format!(r#"{{"id":2,"jsonrpc":"2.0","result":{{"session":{{"sessionId":"{sid}","status":"idle"}},"viewCursor":"v:{sid}:0"}}}}"#))); (a, o) }

    // The scripted exchange of test/msp_test.bend, line by line (frames as `muse serve` 1.4.6 sends them, trimmed).
    #[test] fn msp_request_lines() {
        assert_eq!(m(&init()), "initialize"); assert!(init().contains("\"sessionMcp\""));
        let t = turn(""); let mut a = Msp::new(); let o1 = a.line(&t, &j(INIT_OK));
        assert_eq!(m(&o1.send[0]), "initialized"); assert_eq!(m(&o1.send[1]), "session/start");
        assert_eq!(p(&o1.send[1])["approvalMode"], "promptUnmatched");
        assert_eq!(p(&o1.send[1])["config"]["mcpServers"]["backplane"]["url"], "http://127.0.0.1:1/mcp/t?token=x");
        assert_eq!(p(&o1.send[1])["commandId"], "00000000-0000-7000-8017-918255574210");
        let ob = Msp::new().line(&turn("OLD"), &j(r#"{"id":1,"jsonrpc":"2.0","result":{}}"#)); assert_eq!(m(&ob.send[1]), "session/resume");
        let (_, ob2) = opened("OLD", &turn("OLD")); assert_eq!(m(&ob2.send[0]), "session/setApprovalMode"); assert_eq!(m(&ob2.send[1]), "turn/start");
        assert!(ob2.send[1].contains("\"text\":\"hello\"") && !ob2.send[1].contains("backplane_instructions"));
        let (_, o2) = opened("S1", &t); assert_eq!(m(&o2.send[0]), "turn/start"); assert!(o2.send[0].contains("<backplane_instructions>\\nRULES"));
        assert_ne!(p(&o2.send[0])["commandId"], p(&o1.send[1])["commandId"]); assert!(!o2.send[0].contains("reasoningEffort"));
        assert_eq!(emits(&o2), r#"{"type":"thread.started","thread_id":"S1"}"#);
        let (_, oh) = opened("S2", &turn_hi("")); assert_eq!(p(&oh.send[0])["reasoningEffort"], "high");
    }
    #[test] fn msp_notifications() {
        let t = turn(""); let (mut a, _) = opened("S1", &t);
        let n = |a: &mut Msp, l: &str| emits(&a.line(&t, &j(l)));
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/delta","params":{"delta":"Hello —","field":"text","itemId":"m1","sessionId":"S1","viewCursor":"v"}}"#), r#"{"type":"item.delta","text":"Hello —"}"#);
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/delta","params":{"delta":"thinking","field":"summary.0","itemId":"r1","sessionId":"S1"}}"#), "");
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/delta","params":{"delta":"x","itemId":"m1","sessionId":"OTHER"}}"#), "");
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/delta","params":{"delta":"y","itemId":"m1","sessionId":"S1"}}"#), r#"{"type":"item.delta","text":"y"}"#); // no field means text
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/started","params":{"item":{"args":"{\"command\":\"ls\",\"description\":\"List\"}","itemId":"T1","kind":"toolCall","status":"inProgress","tool":"bash"},"sessionId":"S1"}}"#),
            r#"{"type":"item.completed","item":{"type":"command_execution","id":"T1","command":"ls"}}"#);
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/started","params":{"item":{"args":"{\"path\":\"src/a.ts\"}","itemId":"T2","kind":"toolCall","tool":"read_file"},"sessionId":"S1"}}"#),
            r#"{"type":"item.completed","item":{"type":"command_execution","id":"T2","command":"read_file src/a.ts"}}"#);
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/started","params":{"item":{"itemId":"R1","kind":"reminderChild"},"sessionId":"S1"}}"#), "");
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/started","params":{"item":{"itemId":"A1","kind":"subagent","objective":"Find it"},"sessionId":"S1"}}"#), r#"{"type":"item.completed","item":{"type":"command_execution","id":"A1","command":"agent: Find it"}}"#);
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/completed","params":{"item":{"itemId":"T1","kind":"toolCall","status":"completed","tool":"bash"},"sessionId":"S1"}}"#), r#"{"type":"tool.done","id":"T1"}"#);
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/completed","params":{"item":{"itemId":"m1","kind":"agentMessage","status":"completed","text":"Hello — done"},"sessionId":"S1"}}"#), r#"{"type":"item.completed","item":{"type":"agent_message","text":"Hello — done"}}"#);
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"item/completed","params":{"item":{"itemId":"m2","kind":"agentMessage","text":"  "},"sessionId":"S1"}}"#), "");
        assert_eq!(n(&mut a, r#"{"jsonrpc":"2.0","method":"session/todoListChanged","params":{"items":[{"status":"completed","text":"Read"},{"status":"inProgress","text":"Fix"}],"sessionId":"S1"}}"#),
            r#"{"type":"item.updated","item":{"type":"todo_list","items":[{"text":"Read","status":"completed"},{"text":"Fix","status":"in_progress"}]}}"#);
        let mut c = a.clone(); assert_eq!(n(&mut c, r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"sessionId":"CHILD","terminal":"completed"}}"#), ""); assert!(!c.done, "a child's turn is not this turn's");
        let mut d = a.clone(); assert_eq!(n(&mut d, r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"durationMs":7363,"sessionId":"S1","terminal":"completed"}}"#), r#"{"type":"turn.completed"}"#); assert!(d.done);
        let mut f = a.clone(); assert_eq!(n(&mut f, r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"error":{"kind":"authRequired","message":"Not authenticated"},"sessionId":"S1","terminal":"failed"}}"#),
            r#"{"type":"turn.failed","message":"Not authenticated (sign in once with `muse login`)"}"#); assert!(f.done);
        let mut g = a.clone(); assert_eq!(n(&mut g, r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"sessionId":"S1","terminal":"cancelled"}}"#), r#"{"type":"turn.failed","message":"muse turn cancelled"}"#);
    }
    #[test] fn msp_errors_and_approvals() {
        let t = turn(""); let mut a = Msp::new();
        let oe = a.line(&t, &j(r#"{"error":{"code":-32600,"message":"Not initialized"},"id":2,"jsonrpc":"2.0"}"#)); assert_eq!(emits(&oe), r#"{"type":"turn.failed","message":"Not initialized"}"#); assert!(a.done);
        let mut b = Msp::new(); assert_eq!(emits(&b.line(&t, &j(r#"{"error":{"code":-32030,"message":"noop"},"id":3,"jsonrpc":"2.0"}"#))), ""); assert!(!b.done, "a mode error says nothing that matters");
        let (mut s, _) = opened("S1", &t);
        let body = r#"{"id":0,"jsonrpc":"2.0","method":"approval/request","params":{"approvalId":"01a13123-bdf5-7d51-876d-5ba7d639f788","availableChoices":[{"choiceId":"allow_once","decision":"approved","label":"Allow once","scope":"once"},{"choiceId":"allow_session","decision":"approvedForSession","label":"Allow for session","scope":"session"},{"choiceId":"deny","decision":"denied","label":"Deny","scope":"once"}],"currentRequirementId":{"approvalId":"01a13123-bdf5-7d51-876d-5ba7d639f788","sourceIndex":0},"itemId":"I","rawArgs":"{\"command\":\"rm -rf junk\"}","sessionId":"S1","sourceRange":{"first":{"id":"x","sequence":55},"last":{"id":"y","sequence":57}},"subject":{"kind":"shell","command":"rm -rf junk"},"toolName":"bash"}}"#;
        let oq = s.line(&t, &j(body)); assert_eq!(oq.send, vec![r#"{"jsonrpc":"2.0","id":0,"result":{}}"#]); assert_eq!(m(&oq.asks[0]), "approval/request");
        let dec = decide(&oq.asks[0], "accept"); assert_eq!(m(&dec), "approval/decide"); assert_eq!(p(&dec)["choiceId"], "allow_once");
        assert_eq!(p(&decide(body, "session"))["choiceId"], "allow_session"); assert_eq!(p(&decide(body, "decline"))["choiceId"], "deny");
        assert_eq!(p(&dec)["commandId"], "01a13123-bdf5-7d51-876d-000000000057");
        assert_eq!(p(&dec)["requirementId"].to_string(), r#"{"approvalId":"01a13123-bdf5-7d51-876d-5ba7d639f788","sourceIndex":0}"#);
        assert_eq!(tool(&p(body)), "bash"); assert_eq!(detail(&p(body)), "rm -rf junk");
        let other = s.line(&t, &j(r#"{"jsonrpc":"2.0","id":9,"method":"userInput/request","params":{}}"#)); assert!(other.asks.is_empty() && other.send[0].contains("-32601"));
    }
    #[test] fn msp_modes_law() { assert_eq!(mode_k(0), "promptUnmatched"); assert_eq!(mode_k(3), "allowAll"); assert_eq!(mode("approval-required"), "promptUnmatched"); assert_eq!(mode("auto"), "onRequest"); assert_eq!(mode("full-access"), "allowAll"); }
}

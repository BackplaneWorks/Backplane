//! Grok's ACP (Agent Client Protocol: JSON-RPC over stdio), one process per turn: initialize, session/new (or session/load), session/prompt.
//! A port of src/core/acp.bend. What the agent says is reported as Codex shapes (thread.started, item.completed, item.delta, tool.done,
//! turn.completed, turn.failed), which `Cx::codex_line` turns into events. Shared with opencode.rs and, for `update` and the permission
//! pieces, agy.rs. A permission request that goes to the person is the shape `{"type":"bp.ask","request":"<raw request line>"}`.
use serde_json::{json, Map, Value};

/// AcpTurn: what a turn needs.
#[derive(Clone, Debug, Default)]
pub struct AcpTurn { pub cwd: String, pub mcp: String, pub prompt: String, pub text: String, pub session: String, pub ask: bool }

/// Acp: the session id once known, the reply text so far, whether the turn ended, and the tool calls already logged ("\n<id>\n" each).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Acp { pub sid: String, pub buf: String, pub done: bool, pub seen: String }
impl Acp { pub fn new() -> Acp { Acp::default() } pub fn open_now(&self) -> bool { !self.sid.is_empty() } }

/// AcpOut: lines to write, shapes to report (permission asks are `bp.ask` shapes).
#[derive(Debug, Default, PartialEq)]
pub struct Out { pub send: Vec<String>, pub shapes: Vec<Value> }

pub fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
/// Acp.or / get_str: a present string field wins even when empty; the default only when the field is missing or not a string.
pub fn gs<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or(d) }
pub fn or(a: &str, b: &str) -> String { if a.is_empty() { b.into() } else { a.into() } }

/// Acp.rpc
pub fn rpc(id: u64, method: &str, params: Value) -> String { json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string() }
/// Acp.event: `{"type":ty, ...kvs}`
pub fn event(ty: &str, kvs: Value) -> Value { let mut m = Map::new(); m.insert("type".into(), json!(ty)); if let Value::Object(o) = kvs { for (k, v) in o { m.insert(k, v); } } Value::Object(m) }
/// Acp.init: the first line of every turn.
pub fn init() -> String { rpc(1, "initialize", json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false}})) }
/// Acp.mcp
pub fn mcp(url: &str) -> Value { json!([{"type":"http","name":"backplane","url":url,"headers":[]}]) }
/// Acp.open: session/new, or session/load for the thread's own session.
pub fn open(t: &AcpTurn) -> String {
    if t.session.is_empty() { rpc(2, "session/new", json!({"cwd":t.cwd,"mcpServers":mcp(&t.mcp)})) }
    else { rpc(2, "session/load", json!({"sessionId":t.session,"cwd":t.cwd,"mcpServers":mcp(&t.mcp)})) }
}
fn say_rpc(sid: &str, text: &str) -> String { rpc(3, "session/prompt", json!({"sessionId":sid,"prompt":[{"type":"text","text":text}]})) }
/// Acp.say.k: a new session gets Backplane's instructions first.
pub fn say(fresh: bool, t: &AcpTurn, sid: &str) -> String {
    if fresh { say_rpc(sid, &format!("<backplane_instructions>\n{}\n</backplane_instructions>\n\n{}", t.prompt, t.text)) } else { say_rpc(sid, &t.text) }
}
pub fn failed(msg: &str) -> Value { event("turn.failed", json!({"message":msg})) }
/// Acp.hint: a sign-in problem says how to fix it.
pub fn hint(msg: &str) -> String { let l = msg.to_lowercase(); if l.contains("auth") || l.contains("login") { format!("{msg} (sign in once with `grok login`)") } else { msg.into() } }
/// Acp.error
pub fn error(o: &Value, default: &str) -> String { o.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).unwrap_or(default).to_string() }
pub fn fail(a: &mut Acp, msg: &str) -> Out { a.done = true; Out { send: vec![], shapes: vec![failed(&hint(msg))] } }

/// Acp.finished: the reply, then the turn's end.
pub fn finished(a: &mut Acp) -> Vec<Value> {
    a.done = true;
    let mut v = vec![]; if !a.buf.is_empty() { v.push(event("item.completed", json!({"item":{"type":"agent_message","text":a.buf}}))) }
    v.push(event("turn.completed", json!({}))); v
}
/// Acp.opened: the session is open: the prompt, and the thread's session known.
pub fn opened(a: &mut Acp, t: &AcpTurn, sid: &str) -> Out {
    a.sid = sid.into();
    Out { send: vec![say(t.session.is_empty(), t, sid)], shapes: vec![event("thread.started", json!({"thread_id":sid}))] }
}
/// Acp.plan: an ACP plan's entries as todo items.
pub fn plan(es: &Value) -> Value { Value::Array(es.as_array().map(|a| a.iter().map(|e| json!({"text":s(e, "content"), "status": e.get("status").and_then(Value::as_str).unwrap_or("pending")})).collect()).unwrap_or_default()) }

/// Acp.update.k: the four session updates (the caller checks the session is open).
pub fn update(a: &mut Acp, u: &Value) -> Vec<Value> {
    match s(u, "sessionUpdate") {
        "agent_message_chunk" => { let t = u.get("content").map(|c| s(c, "text")).unwrap_or(""); a.buf.push_str(t); vec![event("item.delta", json!({"text":t}))] }
        "tool_call" => vec![event("item.completed", json!({"item":{"type":"command_execution","id":s(u, "toolCallId"),"command":gs(u, "title", gs(u, "kind", "tool"))}}))],
        "plan" => vec![event("item.updated", json!({"item":{"type":"todo_list","items":plan(u.get("entries").unwrap_or(&Value::Null))}}))],
        "tool_call_update" => if matches!(s(u, "status"), "completed" | "failed") { vec![event("tool.done", json!({"id":s(u, "toolCallId")}))] } else { vec![] },
        _ => vec![],
    }
}

fn allow_first(os: &[Value], first: &str) -> String { os.iter().find(|o| s(o, "kind").starts_with("allow")).map(|o| gs(o, "optionId", first).to_string()).unwrap_or_else(|| first.into()) }
fn options(params: &Value) -> Vec<Value> { params.get("options").and_then(Value::as_array).cloned().unwrap_or_default() }
fn resp(id: &Value, result: Value) -> String { json!({"jsonrpc":"2.0","id":id,"result":result}).to_string() }
/// Acp.permit: the agent asks before a tool runs; in full access, allow at once (the first option that allows).
pub fn permit_auto(id: &Value, params: &Value) -> String {
    let os = options(params); let first = os.first().map(|o| s(o, "optionId")).unwrap_or("");
    resp(id, json!({"outcome":{"outcome":"selected","optionId":allow_first(&os, first)}}))
}
fn kind_find(os: &[Value], kind: &str) -> String { os.iter().find(|o| s(o, "kind") == kind).map(|o| s(o, "optionId").to_string()).unwrap_or_default() }
/// Acp.choose: "accept" picks allow-once, "session" allow-always, anything else a reject; none of that kind: "".
pub fn choose(answer: &str, os: &[Value]) -> String {
    let (ao, aa) = (kind_find(os, "allow_once"), kind_find(os, "allow_always"));
    match answer { "accept" => or(&ao, &aa), "session" => or(&aa, &ao), _ => or(&kind_find(os, "reject_once"), &kind_find(os, "reject_always")) }
}
/// Acp.permit.reply: the response line for a permission request, once the person answered (none of the kind: cancelled).
pub fn permit_reply(id: &Value, params: &Value, answer: &str) -> String {
    let opt = choose(answer, &options(params));
    resp(id, json!({"outcome": if opt.is_empty() { json!({"outcome":"cancelled"}) } else { json!({"outcome":"selected","optionId":opt}) }}))
}
/// Acp.permit.answer: the response for a parked request line.
pub fn permit_answer(body: &str, answer: &str) -> String {
    let o: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    permit_reply(o.get("id").unwrap_or(&Value::Null), o.get("params").unwrap_or(&Value::Null), answer)
}
/// Acp.tool: what a request asks to run (its kind, else its title): what "Allow for session" remembers.
pub fn tool(params: &Value) -> String { let tc = params.get("toolCall").unwrap_or(&Value::Null); or(gs(tc, "kind", ""), gs(tc, "title", "tool")) }
/// Acp.detail: a line for the approval card.
pub fn detail(params: &Value) -> String {
    let tc = params.get("toolCall").unwrap_or(&Value::Null); let raw = tc.get("rawInput").unwrap_or(&Value::Null);
    let d = gs(raw, "command", gs(raw, "path", gs(raw, "file_path", gs(raw, "filePath", ""))));
    or(d, &or(gs(tc, "title", ""), &raw.to_string().chars().take(400).collect::<String>()))
}
/// Driver::ask_info for a `bp.ask` shape whose request is the raw line (a string): an approval card for an ACP session/request_permission.
pub fn ask_of(request: &Value) -> Option<super::AskInfo> {
    let o: Value = serde_json::from_str(request.as_str()?).ok()?; let p = o.get("params")?;
    Some(super::AskInfo { kind: "approval".into(), tool: tool(p), detail: detail(p), options: String::new() })
}
/// Driver::answer: the response line for the person's answer (accept | session | anything else declines).
pub fn answer_of(request: &Value, answer: &str) -> Vec<String> { match request.as_str() { Some(l) => vec![permit_answer(l, answer)], None => vec![] } }
/// A JSON-RPC id as Bend's `Ocp.id`/`Nat.read` read it: its digits as a number (0 when none, or a string id with none).
/// Acp.reply.k's id for Grok: Nat.read of the id's raw text (a number, or a string of digits; anything else 0).
pub fn nat_id(o: &Value) -> u64 {
    match o.get("id") { Some(Value::Number(n)) => n.as_u64().unwrap_or(0), Some(Value::String(t)) if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) => t.parse().unwrap_or(0), _ => 0 }
}
/// Ocp.id: the digits of the id's text as a number (opencode).
pub fn num_id(o: &Value) -> u64 {
    let raw = match o.get("id") { Some(Value::Number(n)) => n.to_string(), Some(Value::String(t)) => t.clone(), _ => return 0 };
    raw.chars().filter_map(|c| c.to_digit(10)).fold(0u64, |a, d| a.saturating_mul(10).saturating_add(d as u64))
}
/// Acp.id: a request's id as JSON text, "" for a notification.
pub fn id_json(o: &Value) -> Option<Value> { o.get("id").filter(|v| !v.is_null()).cloned() }

/// Acp.permit.route: to the person (a `bp.ask` shape) or allowed at once.
fn permit_route(ask: bool, o: &Value) -> Out {
    if ask { Out { send: vec![], shapes: vec![json!({"type":"bp.ask","request":o.to_string()})] } }
    else { Out { send: vec![permit_auto(o.get("id").unwrap_or(&Value::Null), o.get("params").unwrap_or(&Value::Null))], shapes: vec![] } }
}
/// Acp.request.k for a request from the agent: its method decides.
pub fn request(a: &mut Acp, t: &AcpTurn, o: &Value) -> Out {
    match s(o, "method") {
        "session/update" => { if !a.open_now() { return Out::default() } // session/load replays the old conversation: only updates after the session is open belong to this turn
            Out { send: vec![], shapes: update(a, o.get("params").and_then(|p| p.get("update")).unwrap_or(&Value::Null)) } }
        "session/request_permission" => permit_route(t.ask, o),
        _ => Out { send: id_json(o).map(|id| vec![json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"not supported"}}).to_string()]).unwrap_or_default(), shapes: vec![] },
    }
}
/// Acp.reply.k: the answer to one of our requests.
fn reply(a: &mut Acp, t: &AcpTurn, o: &Value) -> Out {
    let ok = o.get("error").map_or(true, Value::is_null);
    if !ok { return fail(a, &error(o, "grok failed")) }
    match nat_id(o) {
        1 => Out { send: vec![open(t)], shapes: vec![] },
        2 => { let sid = o.get("result").and_then(|r| r.get("sessionId")).and_then(Value::as_str).unwrap_or(&t.session).to_string(); opened(a, t, &sid) }
        3 => Out { send: vec![], shapes: finished(a) },
        _ => Out::default(),
    }
}
/// Acp.line: one line from the agent.
pub fn line(a: &mut Acp, t: &AcpTurn, o: &Value) -> Out {
    if !o.is_object() { return Out::default() }
    if !s(o, "method").is_empty() { request(a, t, o) } else { reply(a, t, o) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn t(sid: &str) -> AcpTurn { AcpTurn { cwd: "/tmp/p".into(), mcp: "http://127.0.0.1:1/mcp/t?token=x".into(), prompt: "RULES".into(), text: "hello".into(), session: sid.into(), ask: false } }
    fn ta(sid: &str) -> AcpTurn { AcpTurn { ask: true, ..t(sid) } }
    fn ln(a: &mut Acp, t: &AcpTurn, l: &str) -> Out { line(a, t, &serde_json::from_str(l).unwrap()) }
    fn outs(o: &Out) -> String { o.send.iter().cloned().chain(o.shapes.iter().map(|v| v.to_string())).collect::<Vec<_>>().join(" | ") }
    fn method(l: &str) -> String { serde_json::from_str::<Value>(l).ok().map(|v| s(&v, "method").to_string()).unwrap_or_default() }
    const BODY: &str = r#"{"jsonrpc":"2.0","id":"p9","method":"session/request_permission","params":{"toolCall":{"toolCallId":"c1","title":"List files","kind":"execute","rawInput":{"command":"ls -la"}},"options":[{"optionId":"no","kind":"reject_once"},{"optionId":"always","kind":"allow_always"},{"optionId":"yes","kind":"allow_once"}]}}"#;

    #[test] fn a_scripted_exchange_line_by_line() { // test/acp_test.bend
        let tt = t(""); let mut a = Acp::new();
        assert_eq!(method(&init()), "initialize");
        let o1 = ln(&mut a, &tt, r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1}}"#); assert_eq!(method(&o1.send[0]), "session/new");
        let o2 = ln(&mut a, &tt, r#"{"jsonrpc":"2.0","id":2,"result":{"sessionId":"S1"}}"#);
        assert!(o2.send[0].contains("session/prompt") && o2.send[0].contains("<backplane_instructions>\\nRULES"));
        assert_eq!(o2.shapes[0].to_string(), r#"{"type":"thread.started","thread_id":"S1"}"#);
        let u = |x: &str| format!(r#"{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"S1","update":{{"sessionUpdate":"agent_message_chunk","content":{{"type":"text","text":"{x}"}}}}}}}}"#);
        assert_eq!(outs(&ln(&mut a, &tt, &u("Hi "))), r#"{"type":"item.delta","text":"Hi "}"#); ln(&mut a, &tt, &u("there"));
        let o5 = ln(&mut a, &tt, r#"{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}"#);
        assert_eq!(outs(&o5), r#"{"type":"item.completed","item":{"type":"agent_message","text":"Hi there"}} | {"type":"turn.completed"}"#); assert!(a.done);
    }
    #[test] fn resume_loads_and_replay_is_ignored() {
        let mut a = Acp::new(); let old = t("OLD");
        let ob = ln(&mut a, &old, r#"{"jsonrpc":"2.0","id":1,"result":{}}"#); assert_eq!(method(&ob.send[0]), "session/load");
        let orp = ln(&mut a, &old, r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"OLD","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"old"}}}}"#); assert_eq!(outs(&orp), "");
        let o = ln(&mut a, &old, r#"{"jsonrpc":"2.0","id":2,"result":{}}"#); assert!(!o.send[0].contains("<backplane_instructions>") && o.shapes[0]["thread_id"] == "OLD");
    }
    #[test] fn a_sign_in_error_says_how_to_fix_it() {
        let mut a = Acp::new(); let o = ln(&mut a, &t(""), r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"Authentication required"}}"#);
        assert_eq!(outs(&o), r#"{"type":"turn.failed","message":"Authentication required (sign in once with `grok login`)"}"#); assert!(a.done);
        let mut b = Acp::new(); assert_eq!(outs(&ln(&mut b, &t(""), r#"{"jsonrpc":"2.0","id":2,"error":{"message":"boom"}}"#)), r#"{"type":"turn.failed","message":"boom"}"#);
        let mut c = Acp::new(); assert_eq!(outs(&ln(&mut c, &t(""), r#"{"jsonrpc":"2.0","id":2,"error":{}}"#)), r#"{"type":"turn.failed","message":"grok failed"}"#);
    }
    #[test] fn permissions_allow_at_once_or_go_to_the_person() {
        let req = r#"{"jsonrpc":"2.0","id":"p9","method":"session/request_permission","params":{"options":[{"optionId":"no","kind":"reject_once"},{"optionId":"yes","kind":"allow_once"}]}}"#;
        let mut a = Acp::new(); assert_eq!(outs(&ln(&mut a, &t(""), req)), r#"{"jsonrpc":"2.0","id":"p9","result":{"outcome":{"outcome":"selected","optionId":"yes"}}}"#);
        let q = ln(&mut a, &ta(""), BODY); assert!(q.send.is_empty()); assert_eq!(q.shapes[0]["type"], "bp.ask");
        assert_eq!(method(q.shapes[0]["request"].as_str().unwrap()), "session/request_permission");
        assert_eq!(ln(&mut a, &t(""), req).shapes.len(), 0, "full access never asks");
        assert_eq!(permit_answer(BODY, "accept"), r#"{"jsonrpc":"2.0","id":"p9","result":{"outcome":{"outcome":"selected","optionId":"yes"}}}"#);
        assert_eq!(permit_answer(BODY, "session"), r#"{"jsonrpc":"2.0","id":"p9","result":{"outcome":{"outcome":"selected","optionId":"always"}}}"#);
        assert_eq!(permit_answer(BODY, "decline"), r#"{"jsonrpc":"2.0","id":"p9","result":{"outcome":{"outcome":"selected","optionId":"no"}}}"#);
        assert_eq!(permit_answer(r#"{"id":4,"params":{"options":[{"optionId":"y","kind":"allow_once"}]}}"#, "decline"), r#"{"jsonrpc":"2.0","id":4,"result":{"outcome":{"outcome":"cancelled"}}}"#);
        let p: Value = serde_json::from_str(BODY).unwrap(); assert_eq!(tool(&p["params"]), "execute"); assert_eq!(detail(&p["params"]), "ls -la");
    }
    #[test] fn other_requests_are_refused_and_notifications_ignored() {
        let mut a = Acp::new(); assert_eq!(outs(&ln(&mut a, &t(""), r#"{"jsonrpc":"2.0","id":7,"method":"fs/read_text_file","params":{}}"#)), r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32601,"message":"not supported"}}"#);
        assert_eq!(outs(&ln(&mut a, &t(""), r#"{"jsonrpc":"2.0","method":"x/y"}"#)), "");
    }
    #[test] fn tool_calls_plans_and_their_ends() {
        let mut a = Acp::new(); a.sid = "S".into(); let tt = t("");
        let u = |x: &str| format!(r#"{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"S","update":{x}}}}}"#);
        assert_eq!(outs(&ln(&mut a, &tt, &u(r#"{"sessionUpdate":"tool_call","toolCallId":"c1","title":"ls","kind":"execute"}"#))), r#"{"type":"item.completed","item":{"type":"command_execution","id":"c1","command":"ls"}}"#);
        assert_eq!(outs(&ln(&mut a, &tt, &u(r#"{"sessionUpdate":"tool_call","toolCallId":"c2","kind":"read"}"#))), r#"{"type":"item.completed","item":{"type":"command_execution","id":"c2","command":"read"}}"#);
        assert_eq!(outs(&ln(&mut a, &tt, &u(r#"{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"in_progress"}"#))), "");
        assert_eq!(outs(&ln(&mut a, &tt, &u(r#"{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"completed"}"#))), r#"{"type":"tool.done","id":"c1"}"#);
        assert_eq!(outs(&ln(&mut a, &tt, &u(r#"{"sessionUpdate":"plan","entries":[{"content":"x","priority":"high","status":"in_progress"},{"content":"y"}]}"#))),
            r#"{"type":"item.updated","item":{"type":"todo_list","items":[{"text":"x","status":"in_progress"},{"text":"y","status":"pending"}]}}"#);
    }
    #[test] fn ids_and_empty_fields_read_as_bend_reads_them() {
        let o = json!({"id":"x3"}); assert_eq!(nat_id(&o), 0, "Grok: Nat.read of x3"); assert_eq!(num_id(&o), 3, "opencode: the digits of the id's text");
        assert_eq!(nat_id(&json!({"id":3})), 3); assert_eq!(nat_id(&json!({"id":"3"})), 3);
        let mut a = Acp::new(); a.sid = "S".into();
        let u = json!({"sessionUpdate":"tool_call","toolCallId":"c","title":"","kind":"read"});
        assert_eq!(update(&mut a, &u)[0]["item"]["command"], "", "a present empty title is logged as it is (get_str)");
        assert_eq!(tool(&json!({"toolCall":{"kind":"","title":""}})), "", "present but empty title: Bend's get_str default does not apply");
        assert_eq!(tool(&json!({"toolCall":{}})), "tool");
    }
    // law todo_acp_plan (TD.Todo.codex over Acp.plan): the plan becomes todos, status kept
    #[test] fn todo_acp_plan() {
        let shape = plan(&json!([{"content":"x","priority":"high","status":"in_progress"}]));
        let mut c = crate::agents::lines::Cx::new("t1");
        let fx = c.codex_line(&json!({"type":"item.updated","item":{"type":"todo_list","items":shape}}), "grok", "m", 1);
        assert_eq!(fx.events[0]["data"], r#"[{"id":"1","text":"x","form":"","status":"in_progress"}]"#);
    }
}

#[cfg(test)]
mod seam_tests {
    use super::*;
    #[test] fn the_actors_seam_describes_and_answers_a_request_line() {
        let line = r#"{"jsonrpc":"2.0","id":"p9","method":"session/request_permission","params":{"toolCall":{"toolCallId":"c1","title":"List files","kind":"execute","rawInput":{"command":"ls -la"}},"options":[{"optionId":"no","kind":"reject_once"},{"optionId":"yes","kind":"allow_once"}]}}"#;
        let r = Value::String(line.into()); let i = ask_of(&r).unwrap(); assert_eq!((i.kind.as_str(), i.tool.as_str(), i.detail.as_str(), i.options.as_str()), ("approval", "execute", "ls -la", ""));
        assert_eq!(answer_of(&r, "deny"), vec![r#"{"jsonrpc":"2.0","id":"p9","result":{"outcome":{"outcome":"selected","optionId":"no"}}}"#.to_string()]);
        assert!(ask_of(&Value::String("not json".into())).is_none() && answer_of(&json!({}), "accept").is_empty());
    }
}

/// Stand-in turns through the real actor (Agents on a temp Hub) against the events and the stand-in's input the JS hub produced for the same
/// stand-in (test/gpui/agents/standin-acp.sh; recorded by record_acp.mjs, fixtures by make_acp_fixtures.py), ids and clocks normalised.
/// Stand-in agents only: BACKPLANE_AGENTS-style, a script in a temp folder; no real CLI, credentials or model.
#[cfg(test)]
mod e2e {
    use super::*;
    use crate::{agents::{proc::Cli, Agents}, api::Caller, hub::{Cfg, Hub}, presenter::Canned};
    use std::{path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}};

    const STANDIN: &str = include_str!("../../../../../test/gpui/agents/standin-acp.sh");
    const TH: &str = "t1700000009-9";
    macro_rules! fx { ($n:literal) => { include_str!(concat!("../../../../../test/gpui/agents/fixtures/acp-", $n, ".json")) } }

    fn dir(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-acp-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn script(d: &Path, name: &str, body: &str) { use std::os::unix::fs::PermissionsExt; let p = d.join(name); std::fs::write(&p, body).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    fn caller() -> Caller { Caller { gen: "1".into(), client: 1 } }
    async fn rpc(hub: &Arc<Hub>, m: &str, p: Value) -> Value { let h = hub.rpc_for(m).expect("handler"); h.call(hub, &caller(), &json!(7), m, &p).await.remove(0) }
    async fn log_from(hub: &Arc<Hub>, from: usize) -> Vec<Value> { hub.store.range(from, usize::MAX).unwrap().iter().filter_map(|l| serde_json::from_str(l).ok()).collect() }
    async fn until(hub: &Arc<Hub>, from: usize, secs: u64, f: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let end = Instant::now() + Duration::from_secs(secs);
        loop { let ev = log_from(hub, from).await; if f(&ev) || Instant::now() > end { return ev } tokio::time::sleep(Duration::from_millis(50)).await }
    }
    fn ended(ev: &[Value]) -> usize { ev.iter().filter(|e| e["$"] == "TurnChanged" && e["state"] != "running").count() }

    fn digits(s: &str) -> String { let mut o = String::new(); let mut run = false; for c in s.chars() { if c.is_ascii_digit() { if !run { o.push('N'); run = true } } else { run = false; o.push(c) } } o }
    /// Ids and clocks only: `at`, the digits in ids, sessions, keys of per-message settings.
    fn norm(v: &Value) -> String {
        let setting = v["$"] == "SettingSet"; let skey = if setting { v["key"].as_str().unwrap_or("") } else { "" };
        let mut o = Map::new();
        for (k, x) in v.as_object().unwrap() {
            let nx = match (k.as_str(), x) {
                ("at", _) => json!(0),
                ("msg" | "act" | "id" | "session" | "key", Value::String(s)) => json!(digits(s)),
                ("value", Value::String(s)) if skey.starts_with("turn.from.") || skey.starts_with("tok.basis.") || skey.starts_with("failed.") => json!(digits(s)),
                _ => x.clone(),
            };
            o.insert(k.clone(), nx);
        }
        Value::Object(o).to_string()
    }
    fn core(ev: &[Value]) -> Vec<Value> {
        let policy = ["liveness.", "autosettle.", "alert.", "told.", "summary.at.", "star.nudge", "machine.id", "accounts.", "allowance.", "tz."];
        ev.iter().filter(|e| { let key = e["key"].as_str().unwrap_or(""); !policy.iter().any(|p| key.starts_with(p)) && (e.to_string().contains(TH) || key.starts_with("stopped.") || key.starts_with("failed.")) }).cloned().collect()
    }
    /// The stand-in's lines, as make_acp_fixtures.py's norm_in: the folder, the MCP address and the runtime prompt replaced; a pid in a session id.
    fn norm_in(l: &str) -> String {
        let Some(j) = l.strip_prefix("IN ") else { return digits(l) };
        let Ok(mut o) = serde_json::from_str::<Value>(j) else { return l.into() };
        if let Some(p) = o.get_mut("params") {
            if p.get("cwd").is_some() { p["cwd"] = json!("CWD") }
            if let Some(ms) = p.get_mut("mcpServers").and_then(Value::as_array_mut) { for m in ms { m["url"] = json!("MCP") } }
            if let Some(cs) = p.get_mut("prompt").and_then(Value::as_array_mut) { for c in cs { let t = c["text"].as_str().unwrap_or("").to_string(); if t.starts_with("<backplane_instructions>") { c["text"] = json!(format!("<I>{}", t.split("</backplane_instructions>").nth(1).unwrap_or(""))) } } }
        }
        format!("IN {}", digits(&serde_json::to_string(&o).unwrap()))
    }

    struct Run { events: Vec<Value>, fake: Vec<String> }
    /// One scenario: a thread on `prov` with `runtime` (and an old session), `msgs` sent in order (each after the last turn ended); for an ask, the answer.
    async fn run(prov: &str, name: &str, runtime: &str, model: &str, session: &str, env: &str, msgs: &[&str], answer: Option<&str>) -> Run {
        let sd = dir(&format!("{name}-bin")); let work = dir(&format!("{name}-work")); let home = dir(&format!("{name}-home")); let fake = home.join("fake.log");
        script(&sd, prov, &STANDIN.replacen('\n', &format!("\nexport FAKE_LOG={} {env}\n", fake.display()), 1));
        let mut log = format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1700000000-1\",\"title\":\"fixture\",\"root\":{},\"at\":1700000000}}\n", serde_json::to_string(&work.to_string_lossy()).unwrap());
        log += &format!("{{\"$\":\"ThreadCreated\",\"id\":\"{TH}\",\"project\":\"p1700000000-1\",\"title\":\"Acp\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"{prov}\",\"model\":\"{model}\",\"at\":1700000005}}\n");
        if runtime != "full-access" { log += &format!("{{\"$\":\"ModesSet\",\"id\":\"{TH}\",\"runtime\":\"{runtime}\",\"interaction\":\"default\",\"model\":\"{model}\",\"at\":1700000006}}\n") }
        if !session.is_empty() { log += &format!("{{\"$\":\"SessionSet\",\"id\":\"{TH}\",\"session\":\"{session}\",\"at\":1700000007}}\n") }
        std::fs::write(home.join("events.jsonl"), &log).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let ag = Agents::start_with(hub.clone(), Cli::standin(&sd)).await.unwrap();
        let from = hub.store.len();
        for (i, m) in msgs.iter().enumerate() {
            assert_eq!(rpc(&hub, "turn.start", json!({"thread":TH,"text":m})).await["ok"], true);
            if let Some(a) = answer {
                let ev = until(&hub, from, 20, |ev| ev.iter().any(|e| e["$"] == "AskOpened")).await;
                let id = ev.iter().find(|e| e["$"] == "AskOpened").expect("the stand-in asked")["id"].as_str().unwrap().to_string();
                assert_eq!(rpc(&hub, "ask.answer", json!({"id":id,"answer":a})).await["ok"], true);
            }
            until(&hub, from, 30, |ev| ended(ev) > i).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        let events = log_from(&hub, from).await; ag.shutdown();
        tokio::time::sleep(Duration::from_millis(300)).await;
        let fake = std::fs::read_to_string(&fake).unwrap_or_default().lines().map(str::to_string).collect();
        Run { events, fake }
    }
    fn expect(name: &str, r: &Run, fixture: &str) {
        let want: Value = serde_json::from_str(fixture).unwrap();
        let (got, exp): (Vec<String>, Vec<String>) = (core(&r.events).iter().map(norm).collect(), want["events"].as_array().unwrap().iter().map(norm).collect());
        assert_eq!(got.len(), exp.len(), "{name}: event counts rust {} js {}\nrust:\n{}\njs:\n{}", got.len(), exp.len(), got.join("\n"), exp.join("\n"));
        for (i, (a, b)) in got.iter().zip(exp.iter()).enumerate() { assert_eq!(a, b, "{name}: event {i}") }
        let gi: Vec<String> = r.fake.iter().map(|l| norm_in(l)).collect(); let wi: Vec<String> = want["fake"].as_array().unwrap().iter().map(|l| norm_in(l.as_str().unwrap())).collect();
        assert_eq!(gi, wi, "{name}: what the stand-in received");
    }
    const NO_ENV: &str = "";
    #[tokio::test(flavor = "multi_thread")] async fn grok_turn_matches_the_js_hub() { let r = run("grok", "gt", "full-access", "", "", NO_ENV, &["hello"], None).await; expect("grok-turn", &r, fx!("grok-turn")) }
    #[tokio::test(flavor = "multi_thread")] async fn grok_resume_loads_the_session_each_turn() { let r = run("grok", "gr", "full-access", "", "old-session", NO_ENV, &["hello", "second"], None).await; expect("grok-resume", &r, fx!("grok-resume")) }
    #[tokio::test(flavor = "multi_thread")] async fn grok_asks_and_the_person_accepts() { let r = run("grok", "ga", "approval-required", "", "", NO_ENV, &["zqask please"], Some("accept")).await; expect("grok-ask-accept", &r, fx!("grok-ask-accept")) }
    #[tokio::test(flavor = "multi_thread")] async fn grok_asks_and_the_person_allows_for_the_session() { let r = run("grok", "gs", "approval-required", "", "", NO_ENV, &["zqask please"], Some("session")).await; expect("grok-ask-session", &r, fx!("grok-ask-session")) }
    #[tokio::test(flavor = "multi_thread")] async fn grok_asks_and_the_person_declines() { let r = run("grok", "gd", "approval-required", "", "", NO_ENV, &["zqask please"], Some("decline")).await; expect("grok-ask-reject", &r, fx!("grok-ask-reject")) }
    #[tokio::test(flavor = "multi_thread")] async fn opencode_turn_matches_the_js_hub() { let r = run("opencode", "ot", "full-access", "", "", NO_ENV, &["hello"], None).await; expect("oc-turn", &r, fx!("oc-turn")) }
    #[tokio::test(flavor = "multi_thread")] async fn opencode_resumes_without_replay() { let r = run("opencode", "or", "full-access", "", "old-session", NO_ENV, &["hello", "second"], None).await; expect("oc-resume", &r, fx!("oc-resume")) }
    #[tokio::test(flavor = "multi_thread")] async fn opencode_opens_afresh_when_its_session_is_gone_and_says_so_once() { let r = run("opencode", "ol", "full-access", "", "old-session", "FAKE_LOST=1", &["hello"], None).await; expect("oc-lost", &r, fx!("oc-lost")) }
    #[tokio::test(flavor = "multi_thread")] async fn opencode_takes_the_threads_model_on_the_session() { let r = run("opencode", "om", "full-access", "opencode/deepseek-v4.1-flash", "", NO_ENV, &["hello"], None).await; expect("oc-model", &r, fx!("oc-model")) }
    #[tokio::test(flavor = "multi_thread")] async fn opencode_fails_a_turn_whose_model_it_refuses() { let r = run("opencode", "on", "full-access", "opencode/deepseek-v4.1-flash", "", "FAKE_NOMODEL=1", &["hello"], None).await; expect("oc-nomodel", &r, fx!("oc-nomodel")) }
    #[tokio::test(flavor = "multi_thread")] async fn opencode_asks_for_edits_when_supervised() { let r = run("opencode", "oa", "approval-required", "", "", NO_ENV, &["zqask please"], Some("accept")).await; expect("oc-ask", &r, fx!("oc-ask")) }
}

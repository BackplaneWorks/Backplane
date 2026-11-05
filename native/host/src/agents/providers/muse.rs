//! Muse Code (`muse serve`, MSP: msp.rs). One process per turn, resumed by session id, reported in Codex shapes.
//! Ported from server.bend's Muse.* (command line, modes, model/list) and Hub.run.k (laws muse_run_turn, interrupt_muse_stops).
//! Approvals: `approval/request` is received at once (a receipt) and goes to the person as a `{"type":"bp.ask","request":<raw line>}` shape;
//! the answer is `msp::decide(request, answer)`, written back to Muse by whoever holds the ask (the agents lane's permit path).
use super::{msp, AskInfo, Driver, Made, Reaction, TurnIn};
use serde_json::{json, Value};

/// AG.Effort.of("muse", e): the level when Muse takes it, else "" (its own default).
pub fn effort(e: &str) -> &str { if ["minimal", "low", "medium", "high", "xhigh", "max"].contains(&e) { e } else { "" } }

/// Muse.args: `muse serve` with the thread's model; the workspace's own rules and skills load; full access also drops Muse's shell sandbox.
pub fn args(runtime: &str, model: &str) -> Vec<String> {
    let mut v = vec!["serve".to_string()];
    if !model.is_empty() { v.push("--model".into()); v.push(model.into()) }
    v.push("--trust-workspace".into());
    if mode_of(runtime) == 3 { v.push("--disable-sandbox".into()) }
    v
}
/// Runtime.of: "approval-required" 0, "auto-accept-edits" 1, "auto" 2, anything else full access (3).
fn mode_of(runtime: &str) -> u8 { match runtime { "approval-required" => 0, "auto-accept-edits" => 1, "auto" => 2, _ => 3 } }

/// The ask shape the actor puts to the person (shared with the ACP lane: one actor path).
pub fn ask_shape(request_line: &str) -> Value { json!({"type":"bp.ask","request":request_line}) }

pub struct MuseDriver { turn: msp::MspTurn, st: msp::Msp, runtime: String }
impl MuseDriver { pub fn new() -> MuseDriver { MuseDriver { turn: msp::MspTurn::default(), st: msp::Msp::new(), runtime: String::new() } } pub fn done(&self) -> bool { self.st.done } }
impl Driver for MuseDriver {
    fn argv(&self, a: &TurnIn) -> Vec<String> { args(&a.runtime, &a.model) }
    fn start(&mut self, a: &TurnIn) -> Vec<String> {
        self.runtime = a.runtime.clone();
        self.turn = msp::MspTurn { cwd: a.cwd.clone(), mcp: a.mcp_url.clone(), prompt: a.prompt.clone(), text: a.text.clone(), session: a.session.clone(), mode: msp::mode(&a.runtime).into(), effort: effort(&a.effort).into(), nonce: a.gen.clone() };
        self.st = msp::Msp::new();
        vec![msp::init()]
    }
    fn on_line(&mut self, line: &Value) -> Reaction {
        if !line.is_object() { return Reaction::default() }
        let o = self.st.line(&self.turn, line);
        let mut shapes = o.emit; for r in o.asks { shapes.push(ask_shape(&r)) }
        Reaction { send: o.send, shapes }
    }
    /// Muse.permit: an approval/request (the raw line) is an approval card: the tool is what "Allow for session" remembers.
    fn ask_info(&self, request: &Value) -> Option<AskInfo> {
        let v: Value = serde_json::from_str(request.as_str()?).ok()?; if v["method"] != "approval/request" { return None }
        let p = v.get("params").unwrap_or(&Value::Null);
        Some(AskInfo { kind: "approval".into(), tool: msp::tool(p), detail: msp::detail(p), options: String::new() })
    }
    /// Msp.decide: the approval/decide line for the person's answer ("accept", "session", anything else a denial).
    fn answer(&mut self, request: &Value, answer: &str) -> Vec<String> { request.as_str().map(|r| vec![msp::decide(r, answer)]).unwrap_or_default() }
    /// `muse serve` stays up after its turn: the actor ends it once the turn's own events are in (Grok.stop).
    fn ends_on_turn_end(&self) -> bool { true }
    /// Proto.gone
    fn exit_why(&self, _code: i32) -> Option<String> { Some("muse exited before the turn ended (is it signed in? run `muse login`)".into()) }
}
pub fn driver(provider: &str) -> Option<Made> { if provider == "muse" { Some(Made { cli: "muse", driver: Box::new(MuseDriver::new()) }) } else { None } }

// ---- model/list (Muse.probe.sh, Muse.catalog, Models.muse, About.muse): what the Settings job reads ----
pub fn probe_sh() -> String {
    "tmo() { t=$1; shift; if command -v timeout >/dev/null 2>&1; then timeout -k 1 \"$t\" \"$@\"; else gtimeout -k 1 \"$t\" \"$@\"; fi; }; command -v muse >/dev/null && printf '%s\\n' \
'{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"backplane\",\"version\":\"1\"}}}' \
'{\"jsonrpc\":\"2.0\",\"method\":\"initialized\",\"params\":{}}' \
'{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"model/list\",\"params\":{}}' | tmo 15 muse serve --no-session-log 2>/dev/null".into()
}
/// The models in the answer to request 2 among the lines Muse wrote.
pub fn catalog(out: &str) -> Vec<Value> {
    for l in out.lines() { if let Ok(o) = serde_json::from_str::<Value>(l.trim()) { if o.get("id").map(|i| i.to_string()) == Some("2".into()) { return o["result"]["models"].as_array().cloned().unwrap_or_default() } } }
    vec![]
}
pub fn models(xs: &[Value]) -> Vec<String> { xs.iter().map(|o| o["modelId"].as_str().unwrap_or("").to_string()).collect() }
/// About.muse: the picker's labels ("default" first, then each model's label; the default one says so).
pub fn about(xs: &[Value]) -> Value {
    if xs.is_empty() { return json!({}) }
    let mut m = serde_json::Map::new(); m.insert("default".into(), json!("Muse's own default"));
    for o in xs { let id = o["modelId"].as_str().unwrap_or(""); let l = o.get("displayLabel").and_then(Value::as_str).unwrap_or(id);
        { m.insert(id.into(), json!(format!("{l}{}", if o["isDefault"].as_bool().unwrap_or(false) { " · Muse's default" } else { "" }))); } }
    Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ti(runtime: &str, model: &str) -> TurnIn { TurnIn { thread: "t1".into(), cwd: "/tmp/p".into(), model: model.into(), runtime: runtime.into(), effort: "high".into(), text: "hello".into(), prompt: "RULES".into(), mcp_url: "http://127.0.0.1:1/mcp/t?token=x".into(), gen: "1791825557-42".into(), ..Default::default() } }
    #[test] fn muse_args_follow_the_runtime() {
        assert_eq!(args("approval-required", ""), ["serve", "--trust-workspace"]);
        assert_eq!(args("full-access", "m1"), ["serve", "--model", "m1", "--trust-workspace", "--disable-sandbox"]);
        assert_eq!(args("auto", "m1"), ["serve", "--model", "m1", "--trust-workspace"]);
    }
    #[test] fn a_turn_through_the_driver() {
        let mut d = MuseDriver::new(); let a = ti("approval-required", "");
        assert_eq!(d.argv(&a), ["serve", "--trust-workspace"]);
        let s = d.start(&a); assert_eq!(s.len(), 1); assert!(s[0].contains("\"initialize\""));
        let r = d.on_line(&serde_json::from_str(r#"{"id":1,"jsonrpc":"2.0","result":{}}"#).unwrap()); assert_eq!(r.send.len(), 2);
        let r = d.on_line(&serde_json::from_str(r#"{"id":2,"jsonrpc":"2.0","result":{"session":{"sessionId":"S1"}}}"#).unwrap());
        assert!(r.send[0].contains("\"effort\"") == false && r.send[0].contains("\"reasoningEffort\":\"high\"")); assert_eq!(r.shapes[0]["type"], "thread.started");
        let r = d.on_line(&serde_json::from_str(r#"{"jsonrpc":"2.0","id":0,"method":"approval/request","params":{"sessionId":"S1","toolName":"bash","approvalId":"a-b","availableChoices":[{"choiceId":"c1","decision":"approved"}]}}"#).unwrap());
        assert_eq!(r.send, vec![r#"{"jsonrpc":"2.0","id":0,"result":{}}"#]); assert_eq!(r.shapes[0]["type"], "bp.ask");
        assert!(d.on_line(&Value::String("noise".into())).shapes.is_empty());
        let q = r.shapes[0]["request"].clone(); let info = d.ask_info(&q).unwrap(); assert_eq!((info.kind.as_str(), info.tool.as_str()), ("approval", "bash"));
        assert!(d.answer(&q, "accept")[0].contains("approval/decide")); assert!(d.ends_on_turn_end()); assert!(d.ask_info(&Value::String("{\"method\":\"x\"}".into())).is_none());
        assert!(d.on_stop().is_empty(), "interrupt_muse_stops: Stop ends the process, nothing is written first");
        assert!(super::super::driver("muse").is_some());
    }
    #[test] fn effort_is_one_muse_takes() { assert_eq!(effort("max"), "max"); assert_eq!(effort("off"), ""); assert_eq!(effort(""), ""); }
    #[test] fn model_list_reads_request_two() {
        let out = "{\"id\":1,\"result\":{}}\n{\"id\":2,\"result\":{\"models\":[{\"modelId\":\"a\",\"displayLabel\":\"A\",\"isDefault\":true},{\"modelId\":\"b\"}]}}\n";
        let c = catalog(out); assert_eq!(models(&c), ["a", "b"]);
        assert_eq!(about(&c).to_string(), r#"{"default":"Muse's own default","a":"A · Muse's default","b":"b"}"#); assert_eq!(about(&[]).to_string(), "{}"); assert!(catalog("junk").is_empty());
    }
}

//! Pi (`pi --mode rpc`, earendil-works/pi 1.4): src/core/pi. Pi speaks neither ACP nor MSP: its RPC mode reads JSON commands on stdin and
//! writes responses and session events on stdout, one a line. One process per turn:
//!   1 get_state: the session Pi opened (`--session-id`, made when absent);
//!   2 prompt: the person's text; then its events until agent_settled.
//! What it says is reported in the Codex event shapes. Pi has no approval prompts of its own and no MCP flag: Backplane's
//! extension (`pi_ext.js`, written to `<home>/pi/backplane.js` and loaded with `-e`) adds the thread's Backplane MCP server and, before each
//! tool the thread's mode does not allow, asks through Pi's extension UI (a `confirm` titled `backplane:approval`), which goes to the person as
//! an approval (dialog.rs's envelope; the answer an extension_ui_response). Other extensions' dialogs go to the person as questions.
//! Instructions go in with `--append-system-prompt` at every start, never into the conversation.
use super::dialog::{self as dl, event, gs, jshow, obj, or, raw, take};
use super::{AskInfo, Driver, Made, Reaction, TurnIn};
use serde_json::{json, Value};

/// Acp: the session id once known, the reply text so far, whether the turn has ended, and the failure the last assistant message ended with ("" for none).
#[derive(Clone, Debug, Default)]
pub struct Acp { pub sid: String, pub buf: String, pub done: bool, pub err: String }

/// AcpOut: lines to write to the agent, lines to report to the hub, and dialogs for the person (bp/ask envelopes).
#[derive(Debug, Default)]
pub struct Out { pub send: Vec<String>, pub emit: Vec<Value>, pub asks: Vec<Value> }
impl Out {
    pub fn none() -> Out { Out::default() }
    pub fn emit(v: Vec<Value>) -> Out { Out { emit: v, ..Out::default() } }
    pub fn into_reaction(self) -> Reaction { let mut shapes = self.emit; for e in self.asks { shapes.push(json!({"type":"bp.ask","request":e})) } Reaction { send: self.send, shapes } }
}

/// Pi.cmd
pub fn cmd(id: &str, ty: &str, kvs: Vec<(&str, Value)>) -> String { let mut v = vec![("id", json!(id)), ("type", json!(ty))]; v.extend(kvs); jshow(&obj(v)) }
/// Pi.init: the first line of every turn.
pub fn init() -> String { cmd("1", "get_state", vec![]) }

/// Pi.safe: a session id takes letters, digits, ".", "_" and "-".
pub fn safe(s: &str) -> String { s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '-' }).collect() }
/// Pi.session: the session a turn opens: the thread's own, else a new one (from the thread and the turn's gen).
pub fn session(session: &str, thread: &str, gen: &str) -> String { if session.is_empty() { format!("bp-{}-{}-0", safe(thread), safe(gen)) } else { session.to_string() } }
/// Pi.mode.k by the thread's runtime: supervised asks before every tool but reading and Backplane's own; accept edits also lets edit and write
/// run; auto and full access ask nothing.
pub fn mode(runtime: &str) -> &'static str { match runtime { "approval-required" => "ask", "auto-accept-edits" => "edit", _ => "allow" } }
/// Grok.asks: whether tool requests go to the person (every mode but full access; the runtime "" is full access).
pub fn asks(runtime: &str) -> bool { matches!(runtime, "approval-required" | "auto-accept-edits" | "auto") }
/// PiRun.model: "" or "default" is the agent's own default model.
pub fn model(m: &str) -> &str { if m == "default" { "" } else { m } }
/// Effort.levels("pi") and ("omp"): the levels each takes.
pub const PI_LEVELS: &[&str] = &["off", "minimal", "low", "medium", "high", "xhigh", "max"];
pub const OMP_LEVELS: &[&str] = &["off", "auto", "minimal", "low", "medium", "high", "xhigh", "max"];
/// Effort.args("pi", e): `--thinking e` for a level Pi takes, nothing else.
pub fn effort_args(e: &str) -> Vec<String> { if PI_LEVELS.contains(&e) { vec!["--thinking".into(), e.into()] } else { vec![] } }
/// Codex.opt
pub fn opt(flag: &str, v: &str) -> Vec<String> { if v.is_empty() { vec![] } else { vec![flag.into(), v.into()] } }
/// Pi.approval.title
pub const APPROVAL_TITLE: &str = "backplane:approval";
/// Pi.ext: Backplane's extension, as Pi loads it (plain ES module JavaScript), generated from pi by test/agents/prov-pi/gen_ext.py.
pub const EXT: &str = include_str!("pi_ext.js");

/// Written whole and moved into place (turns of several threads may start at once): PiRun.write.
pub fn write_atomic(dir: &std::path::Path, name: &str, text: &str) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{name}.{}.{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&tmp, text)?; crate::plat::fs::rename(&tmp, dir.join(name))
}

// ---- Failures ----
fn signin(low: &str) -> bool { low.contains("no api key") || low.contains("/login") || low.contains("unauthorized") || low.contains("no model") }
/// Pi.hint: the reason, and how to fix it when that is known.
pub fn hint(msg: &str) -> String {
    let low = take(msg, 400).to_lowercase();
    if low.contains("model not found") { format!("{msg} (pick another Pi model in the composer)") }
    else if signin(&low) { format!("{msg} (sign Pi in once: run `pi` and use /login, or set the provider's API key)") } else { msg.to_string() }
}
/// Acp.failed
pub fn failed(msg: &str) -> Value { event("turn.failed", vec![("message", json!(msg))]) }
/// Acp.finished: the reply whole, then the end.
pub fn finished(a: &mut Acp) -> Out {
    a.done = true;
    let mut e = vec![]; if !a.buf.is_empty() { e.push(event("item.completed", vec![("item", obj(vec![("type", json!("agent_message")), ("text", json!(a.buf))]))])) }
    e.push(event("turn.completed", vec![])); Out::emit(e)
}
pub fn notice(msg: &str) -> Value { event("notice", vec![("message", json!(msg))]) }
fn fail(a: &mut Acp, msg: &str) -> Out { a.done = true; Out::emit(vec![failed(&hint(msg))]) }

/// Pi.lost.msg: a resumed session with no messages is one Pi no longer had (it made the id afresh).
fn lost(resumed: bool, count: &str, sid: &str) -> Vec<Value> {
    if resumed && count == "0" { vec![notice(&format!("Pi had no saved conversation for session {sid} in this folder, so this turn starts without the earlier context. This thread's history in Backplane is kept."))] } else { vec![] }
}

/// What a turn needs of the process's state: the person's text, the session the thread had ("" for none) and whether tools ask.
#[derive(Clone, Debug, Default)]
pub struct Turn { pub text: String, pub session: String, pub ask: bool }

/// Pi.prompt
pub fn prompt(text: &str) -> String { cmd("2", "prompt", vec![("message", json!(text))]) }

fn opened(a: &mut Acp, t: &Turn, d: &Value) -> Out {
    let sid = gs(d, "sessionId", &t.session).to_string(); let count = raw(d, "messageCount", "0");
    a.sid = sid.clone();
    let mut e = vec![event("thread.started", vec![("thread_id", json!(sid))])]; e.extend(lost(!t.session.is_empty(), &count, &sid));
    Out { send: vec![prompt(&t.text)], emit: e, asks: vec![] }
}
/// Pi.reply.k: replies to our commands (by id).
fn reply(a: &mut Acp, t: &Turn, o: &Value) -> Out {
    let ok = dl::gb(o, "success", false);
    match (gs(o, "id", ""), ok) {
        ("1", true) => opened(a, t, dl::go(o, "data")),
        // a prompt an extension command handled starts no run: the turn is over
        ("2", true) => if gs(dl::go(o, "data"), "disposition", "") == "handled" { finished(a) } else { Out::none() },
        (_, false) => fail(a, gs(o, "error", "pi refused the command")),
        _ => Out::none(),
    }
}

// ---- Events ----
fn delta(a: &mut Acp, x: &str) -> Out { a.buf.push_str(x); Out::emit(vec![event("item.delta", vec![("text", json!(x))])]) }
/// Pi.update.k: a reply's text is streamed as it comes; a text block after earlier text starts its own paragraph; a thought block's end carries
/// the whole thought, told as a reasoning item (which the hub logs only while thinking is on).
pub fn update(a: &mut Acp, e: &Value) -> Out {
    match gs(e, "type", "") {
        "text_start" => if a.buf.is_empty() { Out::none() } else { delta(a, "\n\n") },
        "text_delta" => delta(a, gs(e, "delta", "")),
        "thinking_end" => Out::emit(vec![event("item.completed", vec![("item", obj(vec![("type", json!("reasoning")), ("text", json!(gs(e, "content", "")))]))])]),
        _ => Out::none(),
    }
}
/// Pi.what: what a call works on, in a line: a file tool's path, a search's pattern.
pub fn what(args: &Value) -> String {
    or(gs(args, "path", gs(args, "file_path", "")), or(gs(args, "pattern", gs(args, "query", "")), gs(args, "url", ""))).to_string()
}
/// Pi.label: a call's line: a shell's is its command; Backplane's own tools by name; others their name and what they work on.
pub fn label(name: &str, args: &Value) -> String {
    if name == "bash" { or(gs(args, "command", ""), "bash").to_string() }
    else if name.starts_with("mcp__backplane__") { format!("backplane {}", dl::drop(name, 16)) }
    else { let w = what(args); if w.is_empty() { name.to_string() } else { format!("{name} {w}") } }
}
/// A call is logged when it starts (Pi gives its input then): the item the hub files as a tool call.
pub fn call_item(o: &Value, command: String) -> Value {
    event("item.completed", vec![("item", obj(vec![("type", json!("command_execution")), ("id", json!(gs(o, "toolCallId", ""))), ("command", json!(command))]))])
}
fn log(o: &Value) -> Value { call_item(o, label(gs(o, "toolName", "tool"), dl::go(o, "args"))) }
/// Pi.done: a call's end ("tool.done": the hub snapshots the design then).
pub fn done(o: &Value) -> Value { event("tool.done", vec![("id", json!(gs(o, "toolCallId", "")))]) }

/// Pi.msg_end: an assistant message ends: one that failed is kept as the turn's failure (Pi may still retry it), one that ended well clears it.
pub fn msg_end(m: &Value, a: &mut Acp) {
    if gs(m, "role", "") == "assistant" {
        let (why, err) = (gs(m, "stopReason", ""), gs(m, "errorMessage", ""));
        a.err = if why == "error" { or(err, "the model's provider failed").to_string() } else if why == "aborted" { or(err, "Pi aborted the turn").to_string() } else { String::new() };
    }
}
/// Pi.settled: Pi will do nothing more: the turn ends, failed when its last assistant message did.
fn settled(a: &mut Acp, aborted: bool) -> Out {
    let e = if aborted && a.err.is_empty() { "Pi aborted the turn".to_string() } else { a.err.clone() };
    if e.is_empty() { finished(a) } else { fail(a, &e) }
}

// ---- Dialogs ----
// Backplane's approval goes to the person as one; any other extension's select, confirm, input or editor goes as a question (a confirm's
// choices Yes and No); a warning or error notification as a notice.
fn str1(x: &Value) -> String { match x { Value::String(s) => s.clone(), o => jshow(o) } }
/// Pi.detail
fn detail(input: &Value) -> String { or(gs(input, "command", ""), &or(&what(input), &take(&jshow(input), 400))).to_string() }
/// Pi.ask: the approval envelope for the extension's confirm.
pub fn ask(o: &Value) -> Value {
    let m = o.get("message").and_then(Value::as_str).and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or(Value::Null);
    dl::line(&dl::id_of(o), "approval", "pi", vec![("tool", json!(gs(&m, "tool", "tool"))), ("detail", json!(detail(dl::go(&m, "input"))))])
}
/// Pi.question.text / Pi.question.opts
pub fn question_text(o: &Value) -> String { if gs(o, "message", "").is_empty() { gs(o, "title", "").to_string() } else { format!("{}\n\n{}", gs(o, "title", ""), gs(o, "message", "")) } }
pub fn question_opts(o: &Value) -> String { if gs(o, "method", "") == "confirm" { "Yes\nNo".into() } else { dl::elems(dl::go(o, "options")).iter().map(str1).collect::<Vec<_>>().join("\n") } }
/// A question envelope: `who` names the agent on the card ("Pi", "Oh My Pi").
pub fn question(o: &Value, who: &str) -> Value {
    dl::line(&dl::id_of(o), "question", "pi", vec![("tool", json!(who)), ("detail", json!(question_text(o))), ("options", json!(question_opts(o))), ("bool", json!(gs(o, "method", "") == "confirm"))])
}
/// An approval when the thread's mode asks, else allowed at once (in full access the extension asks nothing; if it does, allowed).
fn approval(ask_on: bool, o: &Value) -> Out {
    if ask_on { Out { asks: vec![ask(o)], ..Out::none() } } else { Out { send: vec![dl::reply(&ask(o), "accept")], ..Out::none() } }
}
fn ui(t: &Turn, o: &Value) -> Out {
    let (m, title) = (gs(o, "method", ""), gs(o, "title", ""));
    if m == "confirm" && title == APPROVAL_TITLE { approval(t.ask, o) }
    else if m == "notify" { if gs(o, "notifyType", "info") != "info" { Out::emit(vec![notice(&format!("Pi: {}", gs(o, "message", "")))]) } else { Out::none() } }
    else if matches!(m, "select" | "confirm" | "input" | "editor") { Out { asks: vec![question(o, "Pi")], ..Out::none() } }
    else { Out::none() }
}

/// Pi.line: one line from Pi. Lines this turn reads nothing from (the system prompt and the whole conversation come again in some of them) fall
/// to the default arm; skips them by their first bytes before parsing, which changes nothing observable.
pub fn line(a: &mut Acp, t: &Turn, o: &Value) -> Out {
    match gs(o, "type", "") {
        "response" => reply(a, t, o),
        "message_update" => update(a, dl::go(o, "assistantMessageEvent")),
        "tool_execution_start" => Out::emit(vec![log(o)]),
        "tool_execution_end" => Out::emit(vec![done(o)]),
        "message_end" => { msg_end(dl::go(o, "message"), a); Out::none() }
        "agent_settled" => settled(a, dl::gb(o, "aborted", false)),
        "extension_ui_request" => ui(t, o),
        "auto_retry_start" => Out::emit(vec![notice(&format!("Pi is retrying (attempt {} of {}): {}", raw(o, "attempt", "?"), raw(o, "maxAttempts", "?"), gs(o, "errorMessage", "")))]),
        "compaction_start" => Out::emit(vec![notice(&format!("Pi is compacting the conversation ({})", gs(o, "reason", "")))]),
        "compaction_end" => if gs(o, "errorMessage", "").is_empty() { Out::none() } else { Out::emit(vec![notice(&format!("Pi could not compact the conversation: {}", gs(o, "errorMessage", "")))]) },
        "extension_error" => Out::emit(vec![notice(&format!("A Pi extension failed ({}): {}", gs(o, "event", ""), gs(o, "error", "")))]),
        _ => Out::none(),
    }
}

/// The driver of one Pi turn.
pub struct PiDriver { acp: Acp, turn: Turn }
impl PiDriver { pub fn new() -> PiDriver { PiDriver { acp: Acp::default(), turn: Turn::default() } } }
impl Default for PiDriver { fn default() -> Self { Self::new() } }

/// PiRun.args: `pi --mode rpc` with its session, Backplane's extension and the instructions; the model ("provider/id") and thinking level are flags.
pub fn args(sid: &str, ext: &str, prompt: &str, model_: &str, effort: &str) -> Vec<String> {
    let mut v: Vec<String> = ["--mode", "rpc", "--session-id", sid, "-e", ext, "--append-system-prompt", prompt].iter().map(|x| x.to_string()).collect();
    v.extend(opt("--model", model(model_))); v.extend(effort_args(effort)); v
}

impl Driver for PiDriver {
    /// Proto.install
    fn install_hint(&self) -> Option<String> { Some("Install Pi (https://pi.dev), then sign in: run `pi` and use /login.".into()) }
    /// Proto.gone
    fn exit_why(&self, _code: i32) -> Option<String> { Some("pi exited before the turn ended (is it signed in? run `pi` and use /login)".into()) }
    /// The extension is written before each turn (`<home>/pi/backplane.js`), as PiRun.go does.
    fn argv(&self, a: &TurnIn) -> Vec<String> {
        let dir = std::path::Path::new(&a.home).join("pi");
        let _ = write_atomic(&dir, "backplane.js", EXT);
        args(&session(&a.session, &a.thread, &a.gen), &dir.join("backplane.js").to_string_lossy(), &a.prompt, &a.model, &a.effort)
    }
    /// BACKPLANE_PI_MCP and BACKPLANE_PI_MODE: what Backplane's extension reads.
    fn env(&self, a: &TurnIn) -> Vec<(String, String)> { vec![("BACKPLANE_PI_MCP".into(), a.mcp_url.clone()), ("BACKPLANE_PI_MODE".into(), mode(&a.runtime).into())] }
    fn start(&mut self, a: &TurnIn) -> Vec<String> {
        self.turn = Turn { text: a.text.clone(), session: a.session.clone(), ask: asks(&a.runtime) }; self.acp = Acp::default(); vec![init()]
    }
    fn on_line(&mut self, line_: &Value) -> Reaction {
        if !line_.is_object() { return Reaction::default() }
        line(&mut self.acp, &self.turn, line_).into_reaction()
    }
    fn ask_info(&self, request: &Value) -> Option<AskInfo> { dl::ask_info(request) }
    fn answer(&mut self, request: &Value, answer: &str) -> Vec<String> { vec![dl::reply(request, answer)] }
    fn ends_on_turn_end(&self) -> bool { true }
}

pub fn driver(provider: &str) -> Option<Made> { if provider == "pi" { Some(Made { cli: "pi", driver: Box::new(PiDriver::new()) }) } else { None } }

#[cfg(test)]
mod tests {
    use super::*;
    trait Rx { fn asks(&self) -> Vec<Value>; fn plain(&self) -> Vec<Value>; }
    impl Rx for Reaction {
        fn asks(&self) -> Vec<Value> { self.shapes.iter().filter(|s| s["type"] == "bp.ask").map(|s| s["request"].clone()).collect() }
        fn plain(&self) -> Vec<Value> { self.shapes.iter().filter(|s| s["type"] != "bp.ask").cloned().collect() }
    }
    fn ti(runtime: &str) -> TurnIn { TurnIn { thread: "t1".into(), gen: "9".into(), mcp_url: "http://m".into(), home: "/h".into(), runtime: runtime.into(), text: "hi".into(), prompt: "P".into(), ..TurnIn::default() } }
    fn feed(d: &mut PiDriver, lines: &[&str]) -> Vec<Reaction> { lines.iter().map(|l| d.on_line(&serde_json::from_str(l).unwrap())).collect() }

    // laws pi_mode_*: supervised asks, accept edits asks for all but edits, full access asks nothing
    #[test] fn pi_mode_supervised_asks() { assert_eq!(mode("approval-required"), "ask") }
    #[test] fn pi_mode_accept_edits() { assert_eq!(mode("auto-accept-edits"), "edit") }
    #[test] fn pi_mode_full_allows() { assert_eq!(mode("full-access"), "allow"); assert_eq!(mode("auto"), "allow") }
    // law pi_session_kept: a Pi thread's later turns open its own session, never a new one
    #[test] fn pi_session_kept() { assert_eq!(session("bp-t1-9-0", "anything", "g"), "bp-t1-9-0"); assert_eq!(session("", "t1.x y", "17-3"), "bp-t1.x-y-17-3-0") }
    // laws pi_effort_flag / pi_effort_unknown_default
    #[test] fn pi_effort_flag() { assert_eq!(effort_args("high"), vec!["--thinking", "high"]) }
    #[test] fn pi_effort_unknown_default() { assert!(effort_args("turbo").is_empty() && effort_args("").is_empty()) }
    // laws pi_run_turn / pi_acp_like: a Pi thread's turn is one run of `pi --mode rpc`, its tool calls logged before they run, design steps at "tool.done"
    // law pi_run_turn: a thread's turn is one run of `pi`, carrying its model, session, runtime and text. (cwd goes through the actor, which spawns in it.)
    #[test] fn pi_run_turn() {
        let m = driver("pi").unwrap(); assert_eq!(m.cli, "pi"); assert!(driver("omp").is_none() && driver("claude").is_none());
        let mut d = m.driver; let dir = crate::plat::tmp::uncreated("pi-run");
        let a = TurnIn { thread: "th".into(), home: dir.to_string_lossy().into(), model: "openai/gpt".into(), session: "S9".into(), runtime: "approval-required".into(), text: "say this".into(), prompt: "P".into(), mcp_url: "http://mcp".into(), ..TurnIn::default() };
        let argv = d.argv(&a);
        assert!(argv.windows(2).any(|w| w == ["--model", "openai/gpt"]) && argv.windows(2).any(|w| w == ["--session-id", "S9"]), "model and session reach argv");
        assert_eq!(d.env(&a), vec![("BACKPLANE_PI_MCP".to_string(), "http://mcp".to_string()), ("BACKPLANE_PI_MODE".into(), "ask".into())]);
        d.start(&a);
        let r = d.on_line(&json!({"type":"response","id":"1","success":true,"data":{"sessionId":"S9","messageCount":4}}));
        assert_eq!(r.send, vec![r#"{"id":"2","type":"prompt","message":"say this"}"#], "the text reaches the prompt");
    }
    #[test] fn pi_exits_with_its_own_words() { assert_eq!(PiDriver::new().exit_why(1).unwrap(), "pi exited before the turn ended (is it signed in? run `pi` and use /login)") }
    #[test] fn pi_acp_like() {
        let mut d = PiDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"tool_execution_start","toolCallId":"c1","toolName":"bash","args":{"command":"ls -la"}}"#, r#"{"type":"tool_execution_end","toolCallId":"c1","toolName":"bash"}"#]);
        assert_eq!(r[0].plain(), vec![json!({"type":"item.completed","item":{"type":"command_execution","id":"c1","command":"ls -la"}})]);
        assert_eq!(r[1].plain(), vec![json!({"type":"tool.done","id":"c1"})]);
    }

    #[test] fn the_command_line_is_the_hubs() {
        let d = PiDriver::new(); let mut a = ti("approval-required"); a.model = "anthropic/claude-x".into(); a.effort = "high".into();
        let dir = crate::plat::tmp::uncreated("pi-argv"); a.home = dir.to_string_lossy().into();
        let v = d.argv(&a);
        assert_eq!(v, vec!["--mode", "rpc", "--session-id", "bp-t1-9-0", "-e", &format!("{}/pi/backplane.js", a.home), "--append-system-prompt", "P", "--model", "anthropic/claude-x", "--thinking", "high"]);
        assert_eq!(std::fs::read_to_string(dir.join("pi/backplane.js")).unwrap(), EXT);
        assert_eq!(d.env(&a), vec![("BACKPLANE_PI_MCP".to_string(), a.mcp_url.clone()), ("BACKPLANE_PI_MODE".into(), "ask".into())]);
        a.model = "default".into(); a.effort = "".into(); a.session = "sess-1".into();
        assert_eq!(d.argv(&a)[3], "sess-1"); assert_eq!(d.argv(&a).len(), 8);
    }
    // the file is generated from pi by test/agents/prov-pi/gen_ext.py; its bytes are pinned
    #[test] fn the_extension_is_pi_byte_for_byte() { use sha2::{Digest, Sha256}; assert_eq!(hex::encode(Sha256::digest(EXT.as_bytes())), "97e272d8ae5d6a2bbb06895fa13c4b03053ef17772fe5d7bb813cc858fa6af5f") }

    #[test] fn a_turn_streams_a_reply_and_ends() {
        let mut d = PiDriver::new(); assert_eq!(d.start(&ti("full-access")), vec![r#"{"id":"1","type":"get_state"}"#]);
        let r = feed(&mut d, &[
            r#"{"type":"response","id":"1","command":"get_state","success":true,"data":{"sessionId":"S1","messageCount":0}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_start"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hello"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_end","content":"hmm"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_start"}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"again"}}"#,
            r#"{"type":"message_end","message":{"role":"assistant","stopReason":"stop"}}"#,
            r#"{"type":"agent_settled"}"#]);
        assert_eq!(r[0].send, vec![r#"{"id":"2","type":"prompt","message":"hi"}"#]);
        assert_eq!(r[0].plain(), vec![json!({"type":"thread.started","thread_id":"S1"})]);
        assert!(r[1].plain().is_empty());
        assert_eq!(r[2].plain(), vec![json!({"type":"item.delta","text":"Hello"})]);
        assert_eq!(r[3].plain(), vec![json!({"type":"item.completed","item":{"type":"reasoning","text":"hmm"}})]);
        assert_eq!(r[4].plain(), vec![json!({"type":"item.delta","text":"\n\n"})]);
        assert_eq!(r[7].plain(), vec![json!({"type":"item.completed","item":{"type":"agent_message","text":"Hello\n\nagain"}}), json!({"type":"turn.completed"})]);
    }
    #[test] fn a_failed_assistant_message_fails_the_turn_with_the_hint() {
        let mut d = PiDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"message_end","message":{"role":"assistant","stopReason":"error","errorMessage":"No API key for provider"}}"#, r#"{"type":"agent_settled"}"#]);
        assert_eq!(r[1].plain(), vec![json!({"type":"turn.failed","message":"No API key for provider (sign Pi in once: run `pi` and use /login, or set the provider's API key)"})]);
        let mut d = PiDriver::new(); d.start(&ti("full-access"));
        assert_eq!(feed(&mut d, &[r#"{"type":"agent_settled","aborted":true}"#])[0].plain(), vec![json!({"type":"turn.failed","message":"Pi aborted the turn"})]);
        let mut d = PiDriver::new(); d.start(&ti("full-access"));
        assert_eq!(feed(&mut d, &[r#"{"type":"response","id":"1","success":false,"error":"Model not found: x"}"#])[0].plain(), vec![json!({"type":"turn.failed","message":"Model not found: x (pick another Pi model in the composer)"})]);
    }
    #[test] fn a_resumed_session_pi_no_longer_has_is_said_once() {
        let mut d = PiDriver::new(); let mut a = ti("full-access"); a.session = "old".into(); d.start(&a);
        let r = feed(&mut d, &[r#"{"type":"response","id":"1","success":true,"data":{"sessionId":"old","messageCount":0}}"#]);
        assert_eq!(r[0].plain().len(), 2); assert_eq!(r[0].plain()[1]["type"], "notice");
        assert!(r[0].plain()[1]["message"].as_str().unwrap().starts_with("Pi had no saved conversation for session old in this folder"));
    }
    #[test] fn a_handled_prompt_ends_the_turn_and_other_replies_are_quiet() {
        let mut d = PiDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"response","id":"2","success":true,"data":{"disposition":"started"}}"#, r#"{"type":"response","id":"2","success":true,"data":{"disposition":"handled"}}"#]);
        assert!(r[0].plain().is_empty()); assert_eq!(r[1].plain(), vec![json!({"type":"turn.completed"})]);
    }
    #[test] fn backplanes_approval_asks_in_a_supervised_thread_and_is_allowed_in_full_access() {
        let o = r#"{"type":"extension_ui_request","id":"u1","method":"confirm","title":"backplane:approval","message":"{\"tool\":\"bash\",\"input\":{\"command\":\"rm x\"}}"}"#;
        let mut d = PiDriver::new(); d.start(&ti("approval-required"));
        let r = feed(&mut d, &[o]);
        assert!(r[0].send.is_empty() && r[0].plain().is_empty());
        assert_eq!(jshow(&r[0].asks()[0]), r#"{"jsonrpc":"2.0","id":"u1","method":"bp/ask","params":{"kind":"approval","proto":"pi","tool":"bash","detail":"rm x"}}"#);
        assert_eq!(d.answer(&r[0].asks()[0], "accept"), vec![r#"{"type":"extension_ui_response","id":"u1","confirmed":true}"#]);
        assert_eq!(d.answer(&r[0].asks()[0], "decline"), vec![r#"{"type":"extension_ui_response","id":"u1","confirmed":false}"#]);
        let mut d = PiDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[o]);
        assert!(r[0].asks().is_empty()); assert_eq!(r[0].send, vec![r#"{"type":"extension_ui_response","id":"u1","confirmed":true}"#]);
    }
    #[test] fn another_extensions_dialogs_are_questions_and_warnings_notices() {
        let mut d = PiDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[
            r#"{"type":"extension_ui_request","id":"s1","method":"select","title":"Pick","options":["a",{"x":1}]}"#,
            r#"{"type":"extension_ui_request","id":"c1","method":"confirm","title":"Sure?","message":"really"}"#,
            r#"{"type":"extension_ui_request","id":"n1","method":"notify","message":"fyi","notifyType":"info"}"#,
            r#"{"type":"extension_ui_request","id":"n2","method":"notify","message":"careful","notifyType":"warning"}"#,
            r#"{"type":"extension_ui_request","id":"x","method":"setStatus"}"#]);
        assert_eq!(jshow(&r[0].asks()[0]), r#"{"jsonrpc":"2.0","id":"s1","method":"bp/ask","params":{"kind":"question","proto":"pi","tool":"Pi","detail":"Pick","options":"a\n{\"x\":1}","bool":false}}"#);
        assert_eq!(jshow(&r[1].asks()[0]), r#"{"jsonrpc":"2.0","id":"c1","method":"bp/ask","params":{"kind":"question","proto":"pi","tool":"Pi","detail":"Sure?\n\nreally","options":"Yes\nNo","bool":true}}"#);
        assert!(r[2].plain().is_empty() && r[2].asks().is_empty());
        assert_eq!(r[3].plain(), vec![json!({"type":"notice","message":"Pi: careful"})]);
        assert!(r[4].plain().is_empty() && r[4].asks().is_empty() && r[4].send.is_empty());
    }
    #[test] fn labels_say_what_a_call_works_on() {
        assert_eq!(label("bash", &json!({})), "bash"); assert_eq!(label("bash", &json!({"command":"make"})), "make");
        assert_eq!(label("mcp__backplane__thread_list", &json!({})), "backplane thread_list");
        assert_eq!(label("read", &json!({"path":"a.rs"})), "read a.rs"); assert_eq!(label("grep", &json!({"pattern":"x"})), "grep x"); assert_eq!(label("web", &json!({"url":"u"})), "web u"); assert_eq!(label("ls", &json!({})), "ls");
    }
    #[test] fn retries_compaction_and_extension_failures_are_notices() {
        let mut d = PiDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"auto_retry_start","attempt":2,"maxAttempts":3,"errorMessage":"overloaded"}"#, r#"{"type":"compaction_start","reason":"threshold"}"#, r#"{"type":"compaction_end"}"#,
            r#"{"type":"compaction_end","errorMessage":"too big"}"#, r#"{"type":"extension_error","event":"tool_call","error":"boom"}"#, r#"{"type":"message_start"}"#, r#"{"type":"agent_end"}"#]);
        let m = |i: usize| r[i].plain().get(0).map(|s| s["message"].as_str().unwrap().to_string());
        assert_eq!(m(0).unwrap(), "Pi is retrying (attempt 2 of 3): overloaded"); assert_eq!(m(1).unwrap(), "Pi is compacting the conversation (threshold)");
        assert!(m(2).is_none()); assert_eq!(m(3).unwrap(), "Pi could not compact the conversation: too big"); assert_eq!(m(4).unwrap(), "A Pi extension failed (tool_call): boom");
        assert!(r[5].plain().is_empty() && r[6].plain().is_empty());
    }
}

//! Providers whose turns are a process each, reported to the hub in Codex's shapes.
//! Codex is the reference: providers/codex.rs. Grok/OpenCode/Muse/Antigravity/Pi/Oh My Pi are one file each, owned by their lanes
//! (docs/LANES.md); each exports `driver(provider) -> Option<Made>`, and the single chain in `driver()` below already lists all of them:
//! a lane edits ONLY its own file(s). This file is the agents lane's.
//!
//! What the actor (agents/mod.rs) does for any driver, and a driver never does:
//!  - delivery: an idle thread starts a turn; a running one posts the message at once and keeps it as the ride (setting `ride.<thread>`), taken by the
//!    next Backplane tool result or at the turn's end (laws ride_*, deliver_never_waits); interrupt ends the process; an exit while running fails the turn;
//!  - the process: spawn (setsid, PDEATHSIG, reaped, group killed), bounded stdin/stdout queues, the stand-in-only rule in test mode, the owner marker;
//!  - events: every *shape* a driver returns goes through `Cx::codex_line` (agents/codex.rs), the one place Codex-shaped lines become log events,
//!    usage (`Turn.codex`), design-history steps (`tool.done`), todos, notices and failures.
//! A driver is a small state machine for ONE turn: it names the command line, writes its handshake, and turns the process's own protocol
//! (ACP / MSP / RPC JSON lines) into shapes and replies. It is built fresh per turn (`driver()`), so it keeps whatever it needs in itself.
use serde_json::Value;
pub mod acp; pub mod agy; pub mod codex; pub mod dialog; pub mod grok; pub mod msp; pub mod muse; pub mod omp; pub mod opencode; pub mod pi;

/// Everything about the turn a driver may read. `text` is the message with `$skill` mentions made real; `images` its image attachments' paths;
/// `prompt` the runtime prompt (Hub.prompt ++ skills); `mcp_url` this process's Backplane MCP endpoint; `home` the hub's home;
/// `session` the thread's provider session to resume ("" for a fresh one).
#[derive(Clone, Debug, Default)]
pub struct TurnIn {
    pub thread: String, pub cwd: String, pub home: String, pub model: String, pub session: String,
    pub runtime: String, pub interaction: String, pub effort: String, pub think: bool,
    pub text: String, pub images: Vec<String>, pub prompt: String, pub mcp_url: String, pub pcbparts_url: String,
    /// the turn's generation (Hub.gen, also in `mcp_url`): Muse's command ids are made from it
    pub gen: String,
    /// world settings a driver reads (`agy.auth`, `agy.account`, `agy.gcp.project`, `agy.gcp.location`); see `SETTINGS`
    pub settings: Vec<(String, String)>,
}
/// The settings the actor copies into `TurnIn.settings` (add a key here when a driver needs one).
pub const SETTINGS: &[&str] = &["agy.auth", "agy.account", "agy.gcp.project", "agy.gcp.location"];
impl TurnIn { pub fn setting(&self, k: &str) -> &str { self.settings.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str()).unwrap_or("") } }

/// What a driver answers to one line from its process: lines to write back (a reply, the next request) and Codex-shaped lines for the log.
/// Shapes are the JSON objects `Cx::codex_line` reads: thread.started, item.completed {agent_message, command_execution, file_change, todo_list, reasoning},
/// item.started/updated (todo_list), item.delta, tool.done, turn.completed {usage}, turn.failed, notice, error, context.usage; plus `bp.ask` (below).
#[derive(Debug, Default)]
pub struct Reaction { pub send: Vec<String>, pub shapes: Vec<Value> }

/// What a driver tells the actor about a permission/question its process asked (ACP session/request_permission, MSP approval/request, Pi/Oh My Pi bp/ask).
/// `kind` "approval" | "input"; `tool` is what "allow for this session" is keyed on (Acp.tool); `detail` the card; `options` the choices ("" for approval).
#[derive(Debug, Clone, Default)]
pub struct AskInfo { pub kind: String, pub tool: String, pub detail: String, pub options: String }

/// PERMISSIONS (one seam for all providers): a driver puts `{"type":"bp.ask","request": <the process's request: its raw JSON-RPC line as a string, or an envelope object>}`
/// in `Reaction.shapes`. The actor takes that shape before `codex_line`: `ask_info(request)` describes it; if `kind == "approval"` and the thread already allowed that
/// tool for the session (World.sessions) the actor calls `answer(request, "accept")` at once, otherwise it commits AskOpened{id,thread,kind,tool,detail,options}, keeps
/// `request`, and when the person answers (`accept` | `session` | `deny`/other | an input answer) calls `answer(request, answer)` and writes the lines it returns.
pub trait Driver: Send {
    /// The command line (after the binary).
    fn argv(&self, a: &TurnIn) -> Vec<String>;
    /// Environment on top of the actor's (OPENCODE_CONFIG_CONTENT, GEMINI_HOME, ...).
    fn env(&self, _a: &TurnIn) -> Vec<(String, String)> { vec![] }
    /// Lines to write to the process's stdin once it runs (initialize, session/new, prompt, ...). Empty for Codex: the text is an argument.
    fn start(&mut self, _a: &TurnIn) -> Vec<String> { vec![] }
    /// One stdout line. A line that is not JSON is passed as `Value::String`.
    fn on_line(&mut self, line: &Value) -> Reaction;
    /// The person pressed Stop: lines to write before the process is ended (ACP session/cancel). The actor ends the process either way.
    fn on_stop(&mut self) -> Vec<String> { vec![] }
    /// Before `argv`: Err(e) fails the turn as "could not start <cli>: <e>" without starting anything (OmpRun.prepared).
    fn prepare(&mut self, _a: &TurnIn) -> Result<(), String> { Ok(()) }
    /// The install sentence added after a failure to start ("Install Pi (https://pi.dev), then sign in: ..."; Proto.install).
    fn install_hint(&self) -> Option<String> { None }
    /// Proto.gone: the words for a process that exits while its turn runs, WITHOUT the exit code or stderr. The actor words the row as Exit.say:
    /// `<these words, or "agent was killed (signal 9, likely out of memory)" for 137> (exit N | signal N)` and the scrubbed tail of stderr.
    /// None: the actor's "agent exited with code N". (There is no on_exit: a turn that ends by itself says so in a turn.completed/turn.failed shape.)
    fn exit_why(&self, _code: i32) -> Option<String> { None }
    /// Per-turn processes the actor must end once the turn's own events are in (Grok: SIGTERM after turn.completed/turn.failed). Codex exits by itself.
    fn ends_on_turn_end(&self) -> bool { false }
    /// A permission/question in `request` (see above): None if it is not one.
    fn ask_info(&self, _request: &Value) -> Option<AskInfo> { None }
    /// Lines to write to stdin for the person's answer to `request`.
    fn answer(&mut self, _request: &Value, _answer: &str) -> Vec<String> { vec![] }
}

/// A driver and the CLI it runs (resolved through `Cli`: in test mode only the stand-in folder).
pub struct Made { pub cli: &'static str, pub driver: Box<dyn Driver> }

/// The driver for a thread's provider, if the Rust hub runs it.
pub fn driver(provider: &str) -> Option<Made> {
    #[cfg(test)] { if provider == "testprov" { return Some(Made { cli: "testcli", driver: Box::new(test_driver::Td) }) } }
    codex::driver(provider).or_else(|| grok::driver(provider)).or_else(|| opencode::driver(provider)).or_else(|| muse::driver(provider))
        .or_else(|| agy::driver(provider)).or_else(|| pi::driver(provider)).or_else(|| omp::driver(provider))
}

/// A test-only provider: asks once, writes the person's answer back, ends with its turn (the seams a lane's driver uses).
#[cfg(test)] pub mod test_driver {
    use super::*;
    pub struct Td;
    impl Driver for Td {
        fn argv(&self, _a: &TurnIn) -> Vec<String> { vec![] }
        fn start(&mut self, a: &TurnIn) -> Vec<String> { vec![format!("{{\"hello\":\"{}\"}}", a.session)] }
        fn on_line(&mut self, l: &Value) -> Reaction {
            if l.get("q").is_some() { return Reaction { send: vec![], shapes: vec![serde_json::json!({"type":"thread.started","thread_id":"tp-1"}), serde_json::json!({"type":"bp.ask","request":"REQ"})] } }
            if let Some(a) = l.get("ans").and_then(Value::as_str) { return Reaction { send: vec![], shapes: vec![serde_json::json!({"type":"item.completed","item":{"id":"i","type":"agent_message","text":format!("answered {a}")}}), serde_json::json!({"type":"turn.completed"})] } }
            Reaction::default()
        }
        fn ask_info(&self, r: &Value) -> Option<AskInfo> { if r == "REQ" { Some(AskInfo { kind: "approval".into(), tool: "Bash".into(), detail: "ls".into(), options: String::new() }) } else { None } }
        fn answer(&mut self, _r: &Value, a: &str) -> Vec<String> { vec![format!("{{\"ans\":\"{a}\"}}")] }
        fn ends_on_turn_end(&self) -> bool { true }
    }
}

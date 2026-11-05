//! Oh My Pi (`omp --mode rpc-ui`, can1357/oh-my-pi 18.12): src/core/omp.bend. A fork of Pi with its own RPC dialect: JSON commands on stdin, a
//! `ready` frame, then responses and session events on stdout, one a line, the events and the extension dialogs Pi's (pi.rs), so most lines are
//! read the way Pi's are. One process per turn:
//!   0 set_event_filter: message updates as deltas only;
//!   1 get_state: the session omp opened (the newest in the thread's own session folder, `--session-dir` with `--continue`);
//!   2 prompt: the person's text; then its events until prompt_result;
//!   3 get_session_stats: the session's running totals, reported with turn.completed (Turn.codex takes the turn's share) and the context fill.
//! Approvals are omp's own (`--approval-mode`, from the thread's mode): a tool it asks for comes as an extension_ui_request `select` titled
//! "Allow tool: <name>" with the details under it and the choices Approve, Approve <name> for this session and Deny; it goes to the person as an
//! approval and the choice made goes back (dialog.rs). Other selects (omp's `ask` questions), confirms, inputs and editors go as questions.
//! Backplane's own tools come from an extension Backplane writes (`omp_ext.js`, loaded with `-e`). Instructions go in with `--append-system-prompt`
//! as a private file path (one launch folder a turn, removed after the process exits), never into argv or the conversation.
use super::dialog::{self as dl, event, gs, jshow, num, obj, or, raw};
use super::pi::{self, cmd, finished, notice, opt, Acp, Out};
use super::{AskInfo, Driver, Made, Reaction, TurnIn};
use serde_json::{json, Value};

/// Omp.mode.k: supervised asks before writes and commands (always-ask: only reading runs unasked), accept edits lets writes and edits run
/// (write), auto and full access ask nothing (yolo).
pub fn mode(runtime: &str) -> &'static str { match runtime { "approval-required" => "always-ask", "auto-accept-edits" => "write", _ => "yolo" } }
/// Omp.ext
pub const EXT: &str = include_str!("omp_ext.js");
/// Omp.approval.title
pub const APPROVAL_TITLE: &str = "Allow tool: ";

/// Omp.init: deltas only, then the session (two lines).
pub fn init() -> Vec<String> { vec![cmd("0", "set_event_filter", vec![("events", Value::Null), ("messageUpdates", json!("delta"))]), pi::init()] }
pub fn prompt(text: &str) -> String { cmd("2", "prompt", vec![("message", json!(text))]) }
pub fn stats() -> String { cmd("3", "get_session_stats", vec![]) }
/// Omp.dir: a thread's session folder (under Backplane's home), from its id.
pub fn dir(home: &str, thread: &str) -> String { format!("{home}/omp/sessions/{}", pi::safe(thread)) }
/// Effort.of("omp", e): the level omp takes, else its default.
pub fn effort(e: &str) -> &str { if pi::OMP_LEVELS.contains(&e) { e } else { "" } }
/// OmpLaunch.args (after the program): omp's approval mode, its thread's session folder, Backplane's extension and the instructions file.
pub fn args(mode_: &str, sessions: &str, files: &str, model: &str, effort_: &str) -> Vec<String> {
    let mut v: Vec<String> = ["--mode", "rpc-ui", "--approval-mode", mode_, "--session-dir", sessions, "--continue", "-e", &format!("{files}/backplane.js"), "--append-system-prompt", &format!("{files}/system-prompt.txt")].iter().map(|x| x.to_string()).collect();
    v.extend(opt("--model", model)); v.extend(opt("--thinking", effort_)); v
}

// ---- Failures ----
fn signin(low: &str) -> bool { low.contains("no api key") || low.contains("no model selected") || low.contains("/login") || low.contains("unauthorized") }
/// Omp.hint
pub fn hint(msg: &str) -> String {
    let low = dl::take(msg, 400).to_lowercase();
    if low.contains("model not found") { format!("{msg} (pick another Oh My Pi model in the composer)") }
    else if signin(&low) { format!("{msg} (sign Oh My Pi in once: run `omp` and use /login, or set the provider's API key)") } else { msg.to_string() }
}
fn fail(a: &mut Acp, msg: &str) -> Out { a.done = true; Out::emit(vec![pi::failed(&hint(msg))]) }

// ---- Replies to our commands (by id) ----
fn lost(resumed: bool, count: &str) -> Vec<Value> {
    if resumed && count == "0" { vec![notice("Oh My Pi had no saved conversation for this thread, so this turn starts without the earlier context. This thread's history in Backplane is kept.")] } else { vec![] }
}
fn opened(a: &mut Acp, t: &pi::Turn, d: &Value) -> Out {
    let sid = gs(d, "sessionId", &t.session).to_string(); let count = raw(d, "messageCount", "0");
    a.sid = sid.clone();
    let mut e = vec![event("thread.started", vec![("thread_id", json!(sid))])]; e.extend(lost(!t.session.is_empty(), &count));
    Out { send: vec![prompt(&t.text)], emit: e, asks: vec![] }
}
/// Omp.usage: the session's running totals as Codex's (input counts what was read from the cache, which Turn.codex takes out again), its cost.
fn usage(d: &Value) -> Value {
    let tk = dl::go(d, "tokens"); let (input, cache_read) = (dl::gn(tk, "input", 0), dl::gn(tk, "cacheRead", 0));
    obj(vec![("input_tokens", json!(input + cache_read)), ("cached_input_tokens", json!(cache_read)), ("cache_write_input_tokens", json!(dl::gn(tk, "cacheWrite", 0))),
        ("output_tokens", json!(dl::gn(tk, "output", 0))), ("reasoning_output_tokens", json!(dl::gn(tk, "reasoning", 0))), ("cost_usd", num(&raw(d, "cost", "0")))])
}
/// Omp.context: the context it holds now.
fn context(d: &Value) -> Value {
    let c = dl::go(d, "contextUsage"); event("context.usage", vec![("used", json!(dl::gn(c, "tokens", 0))), ("window", json!(dl::gn(c, "contextWindow", 0)))])
}
/// Omp.counted: the turn is over: the reply (it streamed as deltas), its usage, then the end.
fn counted(a: &mut Acp, d: &Value) -> Out {
    a.done = true;
    let mut e = vec![]; if !a.buf.is_empty() { e.push(event("item.completed", vec![("item", obj(vec![("type", json!("agent_message")), ("text", json!(a.buf))]))])) }
    e.push(context(d)); e.push(event("turn.completed", vec![("usage", usage(d))])); Out::emit(e)
}
fn reply(a: &mut Acp, t: &pi::Turn, o: &Value) -> Out {
    let ok = dl::gb(o, "success", false);
    match (gs(o, "id", ""), ok) {
        ("1", true) => opened(a, t, dl::go(o, "data")),
        // a prompt omp finished itself (a builtin slash command) starts no run
        ("2", true) => if dl::gb(dl::go(o, "data"), "agentInvoked", true) { Out::none() } else { finished(a) },
        ("3", true) => counted(a, dl::go(o, "data")),
        ("3", false) => finished(a),
        ("0", false) => Out::none(),
        (_, false) => fail(a, gs(o, "error", "omp refused the command")),
        _ => Out::none(),
    }
}
/// Omp.result: the prompt's run is over: counted when it completed, else failed.
fn result(a: &mut Acp, o: &Value) -> Out {
    let status = gs(o, "status", "completed");
    if status == "completed" { Out { send: vec![stats()], ..Out::none() } }
    else if status == "aborted" { fail(a, "Oh My Pi aborted the turn") }
    else { let err = a.err.clone(); fail(a, or(gs(dl::go(o, "error"), "message", ""), or(&err, "Oh My Pi's turn failed"))) }
}

// ---- Tool calls ----
/// Omp.label: a call's line, as Pi's; a Backplane tool omp reached through its xd:// device (a write to xd://mcp__backplane__<tool>) by its own name.
pub fn label(name: &str, args: &Value) -> String {
    let p = gs(args, "path", "");
    if p.starts_with("xd://mcp__backplane__") { format!("backplane {}", dl::drop(p, 21)) } else { pi::label(name, args) }
}

// ---- Dialogs ----
fn first(xs: &[String]) -> String { xs.first().cloned().unwrap_or_default() }
fn label_of(xs: &[String], part: &str) -> String { xs.iter().find(|x| x.contains(part)).cloned().unwrap_or_default() }
fn opts(o: &Value) -> Vec<String> { dl::elems(dl::go(o, "options")).iter().map(|x| match x { Value::String(s) => s.clone(), o => jshow(o) }).collect() }
/// Omp.ask: an approval: the tool (the title's first line), what it would do (the rest), and the labels each answer picks.
pub fn ask(o: &Value, opts: &[String]) -> Value {
    let rest = dl::drop(gs(o, "title", ""), APPROVAL_TITLE.chars().count()); let lines: Vec<String> = rest.split('\n').map(str::to_string).collect();
    dl::line(&dl::id_of(o), "approval", "pi", vec![("tool", json!(first(&lines))), ("detail", json!(lines.iter().skip(1).cloned().collect::<Vec<_>>().join("\n"))),
        ("once", json!(first(opts))), ("session", json!(label_of(opts, "for this session"))), ("deny", json!(label_of(opts, "Deny")))])
}
fn ui(t: &pi::Turn, o: &Value) -> Out {
    let (m, title) = (gs(o, "method", ""), gs(o, "title", ""));
    if m == "select" && title.starts_with(APPROVAL_TITLE) {
        let e = ask(o, &opts(o));
        // asked in a mode that asks nothing (omp should not): allowed once
        if t.ask { Out { asks: vec![e], ..Out::none() } } else { Out { send: vec![dl::reply(&e, "accept")], ..Out::none() } }
    }
    else if m == "notify" { if gs(o, "notifyType", "info") != "info" { Out::emit(vec![notice(&format!("Oh My Pi: {}", gs(o, "message", "")))]) } else { Out::none() } }
    else if matches!(m, "select" | "confirm" | "input" | "editor") { Out { asks: vec![pi::question(o, "Oh My Pi")], ..Out::none() } }
    else { Out::none() }
}

/// Omp.line: one line from omp. Lines this turn reads nothing from (the whole conversation, the system prompt and the command list come in some
/// of them) fall to the default arm; Bend skips them by their first bytes before parsing, which changes nothing observable.
pub fn line(a: &mut Acp, t: &pi::Turn, o: &Value) -> Out {
    match gs(o, "type", "") {
        "response" => reply(a, t, o),
        "message_update" => pi::update(a, dl::go(o, "assistantMessageEvent")),
        "tool_execution_start" => Out::emit(vec![pi::call_item(o, label(gs(o, "toolName", "tool"), dl::go(o, "args")))]),
        "tool_execution_end" => Out::emit(vec![pi::done(o)]),
        "message_end" => { pi::msg_end(dl::go(o, "message"), a); Out::none() }
        "prompt_result" => result(a, o),
        "extension_ui_request" => ui(t, o),
        "auto_retry_start" => Out::emit(vec![notice(&format!("Oh My Pi is retrying (attempt {} of {}): {}", raw(o, "attempt", "?"), raw(o, "maxAttempts", "?"), gs(o, "errorMessage", "")))]),
        "compaction_start" => Out::emit(vec![notice("Oh My Pi is compacting the conversation")]),
        "extension_error" => Out::emit(vec![notice(&format!("An Oh My Pi extension failed ({}): {}", gs(o, "event", ""), gs(o, "error", "")))]),
        _ => Out::none(),
    }
}

static LAUNCHES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// OmpLaunch.prepare: one private folder a turn under `<home>/omp` (0700) holding the instructions and the extension (0600, created exclusively),
/// so the prompt never enters argv. Removed by `remove` after the process exits.
/// NOTE: Bend masks the prompt (Vx.Vx.mask, the secret vault) before it is written; the Rust hub has no vault yet, so the actor must pass `TurnIn.prompt` masked once it does.
pub fn prepare(home: &str, prompt: &str, extension: &str) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write; use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let root = std::path::Path::new(home).join("omp"); std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&root)?;
    let n = LAUNCHES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = root.join(format!("launch-{}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0), n));
    std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
    let put = |name: &str, text: &str| -> std::io::Result<()> { std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(dir.join(name))?.write_all(text.as_bytes()) };
    if let Err(e) = put("system-prompt.txt", prompt).and_then(|_| put("backplane.js", extension)) { let _ = std::fs::remove_dir_all(&dir); return Err(e) }
    Ok(dir)
}
/// OmpLaunch.remove: only the folder `prepare` returned.
pub fn remove(dir: &std::path::Path) { let _ = std::fs::remove_dir_all(dir); }

/// The driver of one Oh My Pi turn.
pub struct OmpDriver { acp: Acp, turn: pi::Turn, launch: std::cell::RefCell<Option<std::path::PathBuf>> }
impl OmpDriver { pub fn new() -> OmpDriver { OmpDriver { acp: Acp::default(), turn: pi::Turn::default(), launch: std::cell::RefCell::new(None) } } }
impl Default for OmpDriver { fn default() -> Self { Self::new() } }

impl OmpDriver {
    /// OmpLaunch.prepare + Grok.failed: the turn's private launch folder, or the words of a turn that could not start (the actor says "could not start omp: " first).
    pub fn prepare_turn(&self, a: &TurnIn) -> Result<(), String> {
        match prepare(&a.home, &a.prompt, EXT) { Ok(d) => { *self.launch.borrow_mut() = Some(d); Ok(()) } Err(e) => Err(launch_why(&e)) }
    }
}
/// Proto.install
pub const INSTALL: &str = "Install Oh My Pi (https://github.com/can1357/oh-my-pi), then sign in: run `omp` and use /login.";
/// OmpLaunch.why: ENOENT and EIO have words of their own.
pub fn launch_why(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(2) => "Oh My Pi CLI or a required launch path was not found (ENOENT); check omp on the hub's PATH and its working directory".into(),
        Some(5) => "Oh My Pi launch failed (EIO); check its executable and permissions on the working directory and private launch files".into(),
        _ => e.to_string(),
    }
}

impl Driver for OmpDriver {
    /// OmpRun.prepared: omp is never started without its private launch folder; the actor adds "could not start omp: " before and the hint after.
    fn prepare(&mut self, a: &TurnIn) -> Result<(), String> { self.prepare_turn(a) }
    fn install_hint(&self) -> Option<String> { Some(INSTALL.into()) }
    /// Proto.gone
    fn exit_why(&self, _code: i32) -> Option<String> { Some("omp exited before the turn ended (is it signed in? run `omp` and use /login)".into()) }
    /// The launch folder is made by `prepare_turn` (OmpRun.prepared: omp is never started without it). The agents lane calls that first (see the
    /// request for a `Driver::prepare` hook); until it does, the first `argv` makes it, and only a failure there is left to omp.
    fn argv(&self, a: &TurnIn) -> Vec<String> {
        if self.launch.borrow().is_none() { let _ = self.prepare_turn(a); }
        let files = self.launch.borrow().as_ref().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        args(mode(&a.runtime), &dir(&a.home, &a.thread), &files, pi::model(&a.model), effort(&a.effort))
    }
    fn env(&self, a: &TurnIn) -> Vec<(String, String)> { vec![("BACKPLANE_OMP_MCP".into(), a.mcp_url.clone())] }
    fn start(&mut self, a: &TurnIn) -> Vec<String> {
        self.turn = pi::Turn { text: a.text.clone(), session: a.session.clone(), ask: pi::asks(&a.runtime) }; self.acp = Acp::default(); init()
    }
    fn on_line(&mut self, line_: &Value) -> Reaction { if !line_.is_object() { return Reaction::default() } line(&mut self.acp, &self.turn, line_).into_reaction() }
    fn ask_info(&self, request: &Value) -> Option<AskInfo> { dl::ask_info(request) }
    fn answer(&mut self, request: &Value, answer: &str) -> Vec<String> { vec![dl::reply(request, answer)] }
    fn ends_on_turn_end(&self) -> bool { true }
}
impl Drop for OmpDriver { fn drop(&mut self) { if let Some(d) = self.launch.borrow_mut().take() { remove(&d) } } }

pub fn driver(provider: &str) -> Option<Made> { if provider == "omp" { Some(Made { cli: "omp", driver: Box::new(OmpDriver::new()) }) } else { None } }

#[cfg(test)]
mod tests {
    use super::*;
    trait Rx { fn asks(&self) -> Vec<Value>; fn plain(&self) -> Vec<Value>; }
    impl Rx for Reaction {
        fn asks(&self) -> Vec<Value> { self.shapes.iter().filter(|s| s["type"] == "bp.ask").map(|s| s["request"].clone()).collect() }
        fn plain(&self) -> Vec<Value> { self.shapes.iter().filter(|s| s["type"] != "bp.ask").cloned().collect() }
    }
    fn ti(runtime: &str) -> TurnIn { TurnIn { thread: "t1".into(), gen: "9".into(), home: "/h".into(), runtime: runtime.into(), text: "hi".into(), prompt: "P".into(), mcp_url: "http://m".into(), ..TurnIn::default() } }
    fn feed(d: &mut OmpDriver, lines: &[&str]) -> Vec<Reaction> { lines.iter().map(|l| d.on_line(&serde_json::from_str(l).unwrap())).collect() }

    // laws omp_mode_*
    #[test] fn omp_mode_supervised_asks() { assert_eq!(mode("approval-required"), "always-ask") }
    #[test] fn omp_mode_accept_edits() { assert_eq!(mode("auto-accept-edits"), "write") }
    #[test] fn omp_mode_full_allows() { assert_eq!(mode("full-access"), "yolo"); assert_eq!(mode("auto"), "yolo") }
    // laws omp_run_turn / omp_acp_like
    // law omp_run_turn: a thread's turn is one run of `omp`, carrying its model, session, runtime and text. (cwd goes through the actor, which spawns in it.)
    #[test] fn omp_run_turn() {
        let m = driver("omp").unwrap(); assert_eq!(m.cli, "omp"); assert!(driver("pi").is_none() && driver("claude").is_none());
        let mut d = m.driver; let dir_ = std::env::temp_dir().join(format!("bp-omp-run-{}", std::process::id()));
        let a = TurnIn { thread: "th".into(), home: dir_.to_string_lossy().into(), model: "openai/gpt".into(), session: "S9".into(), runtime: "approval-required".into(), text: "say this".into(), prompt: "P".into(), mcp_url: "http://mcp".into(), ..TurnIn::default() };
        let argv = d.argv(&a);
        assert!(argv.windows(2).any(|w| w == ["--model", "openai/gpt"]) && argv.windows(2).any(|w| w == ["--approval-mode", "always-ask"]), "model and runtime reach argv");
        assert_eq!(d.env(&a), vec![("BACKPLANE_OMP_MCP".to_string(), "http://mcp".to_string())]);
        d.start(&a);
        let r = d.on_line(&json!({"type":"response","id":"1","success":true,"data":{"sessionId":"S9","messageCount":4}}));
        assert_eq!(r.send, vec![r#"{"id":"2","type":"prompt","message":"say this"}"#], "the text reaches the prompt; the session is the one resumed");
        assert_eq!(r.shapes, vec![json!({"type":"thread.started","thread_id":"S9"})]);
        drop(d); let _ = std::fs::remove_dir_all(dir_);
    }
    #[test] fn omp_exits_with_its_own_words() { assert_eq!(OmpDriver::new().exit_why(1).unwrap(), "omp exited before the turn ended (is it signed in? run `omp` and use /login)") }
    #[test] fn a_launch_folder_that_cannot_be_made_fails_the_turn_with_the_install_hint() {
        let blocker = std::env::temp_dir().join(format!("bp-omp-block-{}", std::process::id())); std::fs::write(&blocker, "x").unwrap();
        let d = OmpDriver::new(); let a = TurnIn { home: blocker.to_string_lossy().into(), ..TurnIn::default() };
        let mut d = d; let e = d.prepare(&a).unwrap_err(); assert!(!e.is_empty(), "{e}"); assert_eq!(d.install_hint().unwrap(), "Install Oh My Pi (https://github.com/can1357/oh-my-pi), then sign in: run `omp` and use /login.");
        assert_eq!(launch_why(&std::io::Error::from_raw_os_error(2)), "Oh My Pi CLI or a required launch path was not found (ENOENT); check omp on the hub's PATH and its working directory");
        let _ = std::fs::remove_file(blocker);
    }
    #[test] fn omp_acp_like() {
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"tool_execution_start","toolCallId":"c1","toolName":"write","args":{"path":"xd://mcp__backplane__thread_list"}}"#, r#"{"type":"tool_execution_start","toolCallId":"c2","toolName":"bash","args":{"command":"ls"}}"#, r#"{"type":"tool_execution_end","toolCallId":"c1"}"#]);
        assert_eq!(r[0].plain(), vec![json!({"type":"item.completed","item":{"type":"command_execution","id":"c1","command":"backplane thread_list"}})]);
        assert_eq!(r[1].plain()[0]["item"]["command"], "ls"); assert_eq!(r[2].plain(), vec![json!({"type":"tool.done","id":"c1"})]);
    }

    #[test] fn the_first_lines_filter_events_then_open_the_session() {
        let mut d = OmpDriver::new(); assert_eq!(d.start(&ti("full-access")), vec![r#"{"id":"0","type":"set_event_filter","events":null,"messageUpdates":"delta"}"#, r#"{"id":"1","type":"get_state"}"#]);
    }
    #[test] fn the_command_line_is_the_bend_hubs() {
        let d = OmpDriver::new(); let mut a = ti("auto-accept-edits"); a.model = "default".into(); a.effort = "auto".into();
        let dir_ = std::env::temp_dir().join(format!("bp-omp-argv-{}", std::process::id())); a.home = dir_.to_string_lossy().into(); a.prompt = "THE PROMPT".into();
        let v = d.argv(&a); let launch = d.launch.borrow().clone().unwrap();
        assert_eq!(v, vec!["--mode", "rpc-ui", "--approval-mode", "write", "--session-dir", &format!("{}/omp/sessions/t1", a.home), "--continue", "-e", &format!("{}/backplane.js", launch.display()),
            "--append-system-prompt", &format!("{}/system-prompt.txt", launch.display()), "--thinking", "auto"]);
        assert!(!v.iter().any(|x| x == "THE PROMPT")); // never in argv
        assert_eq!(std::fs::read_to_string(launch.join("system-prompt.txt")).unwrap(), "THE PROMPT"); assert_eq!(std::fs::read_to_string(launch.join("backplane.js")).unwrap(), EXT);
        { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(launch.join("system-prompt.txt")).unwrap().permissions().mode() & 0o777, 0o600); assert_eq!(std::fs::metadata(&launch).unwrap().permissions().mode() & 0o777, 0o700); }
        assert_eq!(d.env(&a), vec![("BACKPLANE_OMP_MCP".to_string(), "http://m".to_string())]);
        drop(d); assert!(!launch.exists());
        a.model = "openai/gpt".into(); a.effort = "turbo".into(); let d2 = OmpDriver::new(); let v2 = d2.argv(&a); assert_eq!(&v2[v2.len() - 2..], ["--model", "openai/gpt"]); drop(d2);
        let _ = std::fs::remove_dir_all(dir_);
    }
    #[test] fn a_completed_turn_counts_the_sessions_totals() {
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[
            r#"{"type":"response","id":"1","success":true,"data":{"sessionId":"S2","messageCount":3}}"#,
            r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Done."}}"#,
            r#"{"type":"prompt_result","status":"completed"}"#,
            r#"{"type":"response","id":"3","success":true,"data":{"tokens":{"input":100,"cacheRead":40,"cacheWrite":5,"output":20,"reasoning":7},"cost":0.0123,"contextUsage":{"tokens":900,"contextWindow":200000}}}"#]);
        assert_eq!(r[0].send, vec![r#"{"id":"2","type":"prompt","message":"hi"}"#]);
        assert_eq!(r[2].send, vec![r#"{"id":"3","type":"get_session_stats"}"#]); assert!(r[2].plain().is_empty());
        assert_eq!(r[3].plain(), vec![json!({"type":"item.completed","item":{"type":"agent_message","text":"Done."}}), json!({"type":"context.usage","used":900,"window":200000}),
            json!({"type":"turn.completed","usage":{"input_tokens":140,"cached_input_tokens":40,"cache_write_input_tokens":5,"output_tokens":20,"reasoning_output_tokens":7,"cost_usd":0.0123}})]);
    }
    #[test] fn an_aborted_or_failed_run_fails_the_turn() {
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        assert_eq!(feed(&mut d, &[r#"{"type":"prompt_result","status":"aborted"}"#])[0].plain(), vec![json!({"type":"turn.failed","message":"Oh My Pi aborted the turn"})]);
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        assert_eq!(feed(&mut d, &[r#"{"type":"prompt_result","status":"error","error":{"message":"No API key for x"}}"#])[0].plain(),
            vec![json!({"type":"turn.failed","message":"No API key for x (sign Oh My Pi in once: run `omp` and use /login, or set the provider's API key)"})]);
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"message_end","message":{"role":"assistant","stopReason":"error","errorMessage":"provider down"}}"#, r#"{"type":"prompt_result","status":"error"}"#]);
        assert_eq!(r[1].plain(), vec![json!({"type":"turn.failed","message":"provider down"})]);
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        assert_eq!(feed(&mut d, &[r#"{"type":"prompt_result","status":"error"}"#])[0].plain(), vec![json!({"type":"turn.failed","message":"Oh My Pi's turn failed"})]);
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"response","id":"0","success":false,"error":"x"}"#, r#"{"type":"response","id":"2","success":false,"error":"Model not found: m"}"#]);
        assert!(r[0].plain().is_empty()); assert_eq!(r[1].plain(), vec![json!({"type":"turn.failed","message":"Model not found: m (pick another Oh My Pi model in the composer)"})]);
    }
    #[test] fn a_failed_stats_call_still_ends_the_turn_and_a_builtin_command_runs_nothing() {
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"response","id":"2","success":true,"data":{"agentInvoked":true}}"#, r#"{"type":"response","id":"3","success":false,"error":"no stats"}"#]);
        assert!(r[0].plain().is_empty()); assert_eq!(r[1].plain(), vec![json!({"type":"turn.completed"})]);
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        assert_eq!(feed(&mut d, &[r#"{"type":"response","id":"2","success":true,"data":{"agentInvoked":false}}"#])[0].plain(), vec![json!({"type":"turn.completed"})]);
    }
    const SELECT: &str = r#"{"type":"extension_ui_request","id":"a1","method":"select","title":"Allow tool: bash\nrm -rf x\nin /tmp","options":["Approve","Approve bash for this session","Deny"]}"#;
    #[test] fn omps_approval_select_goes_to_the_person_and_the_label_comes_back() {
        let mut d = OmpDriver::new(); d.start(&ti("approval-required"));
        let r = feed(&mut d, &[SELECT]);
        assert_eq!(jshow(&r[0].asks()[0]), r#"{"jsonrpc":"2.0","id":"a1","method":"bp/ask","params":{"kind":"approval","proto":"pi","tool":"bash","detail":"rm -rf x\nin /tmp","once":"Approve","session":"Approve bash for this session","deny":"Deny"}}"#.replace("\\n", "\\n"));
        assert_eq!(d.answer(&r[0].asks()[0], "accept"), vec![r#"{"type":"extension_ui_response","id":"a1","value":"Approve"}"#]);
        assert_eq!(d.answer(&r[0].asks()[0], "session"), vec![r#"{"type":"extension_ui_response","id":"a1","value":"Approve bash for this session"}"#]);
        assert_eq!(d.answer(&r[0].asks()[0], "decline"), vec![r#"{"type":"extension_ui_response","id":"a1","value":"Deny"}"#]);
        assert_eq!(d.answer(&r[0].asks()[0], ""), vec![r#"{"type":"extension_ui_response","id":"a1","value":"Deny"}"#]);
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[SELECT]);
        assert!(r[0].asks().is_empty()); assert_eq!(r[0].send, vec![r#"{"type":"extension_ui_response","id":"a1","value":"Approve"}"#]);
    }
    #[test] fn omps_other_dialogs_are_questions_and_notifications_notices() {
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"extension_ui_request","id":"q1","method":"select","title":"Which?","options":["x","y"]}"#, r#"{"type":"extension_ui_request","id":"q2","method":"input","title":"Name?"}"#,
            r#"{"type":"extension_ui_request","id":"n","method":"notify","message":"oops","notifyType":"error"}"#, r#"{"type":"extension_ui_request","id":"n","method":"notify","message":"fine"}"#]);
        assert_eq!(jshow(&r[0].asks()[0]), r#"{"jsonrpc":"2.0","id":"q1","method":"bp/ask","params":{"kind":"question","proto":"pi","tool":"Oh My Pi","detail":"Which?","options":"x\ny","bool":false}}"#);
        assert_eq!(r[1].asks()[0]["params"]["options"], ""); assert_eq!(r[2].plain(), vec![json!({"type":"notice","message":"Oh My Pi: oops"})]); assert!(r[3].plain().is_empty());
    }
    #[test] fn notices_for_retries_compaction_and_extensions() {
        let mut d = OmpDriver::new(); d.start(&ti("full-access"));
        let r = feed(&mut d, &[r#"{"type":"auto_retry_start","attempt":1,"maxAttempts":2,"errorMessage":"429"}"#, r#"{"type":"compaction_start"}"#, r#"{"type":"extension_error","event":"x","error":"y"}"#, r#"{"type":"available_commands_update"}"#]);
        assert_eq!(r[0].plain()[0]["message"], "Oh My Pi is retrying (attempt 1 of 2): 429"); assert_eq!(r[1].plain()[0]["message"], "Oh My Pi is compacting the conversation");
        assert_eq!(r[2].plain()[0]["message"], "An Oh My Pi extension failed (x): y"); assert!(r[3].plain().is_empty());
    }
    #[test] fn a_lost_session_is_said_once() {
        let mut d = OmpDriver::new(); let mut a = ti("full-access"); a.session = "old".into(); d.start(&a);
        let r = feed(&mut d, &[r#"{"type":"response","id":"1","success":true,"data":{"sessionId":"new","messageCount":0}}"#]);
        assert_eq!(r[0].plain()[1], json!({"type":"notice","message":"Oh My Pi had no saved conversation for this thread, so this turn starts without the earlier context. This thread's history in Backplane is kept."}));
    }
    // the file is generated from omp.bend by test/gpui/agents/prov-pi/gen_ext.py; its bytes are pinned
    #[test] fn the_extension_is_omp_bends_byte_for_byte() { use sha2::{Digest, Sha256}; assert_eq!(hex::encode(Sha256::digest(EXT.as_bytes())), "a5ebd02e761b170ef6daca3d07c6bc168f978cf6062a9237740d79d723c3e442") }
}

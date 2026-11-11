//! Google Antigravity through the person's own `agy` CLI (Antigravity CLI 1.4+), when Backplane's pinned `agy_acp_server` is not installed.
//! One process a turn, in print mode with JSON lines both ways:
//!   `agy --print= --input-format stream-json --output-format stream-json [--conversation ID] [--model M] [--effort E] [mode flags]`
//! stdin takes `{"event":"user","message":{"role":"user","content":TEXT}}`; stdout says `init` (the conversation), `step_update`s (the reply's
//! text_delta, tools ACTIVE then DONE) and `result` (SUCCESS or ERROR, usage, and the actions headless mode denied).
//! Sign-in is the CLI's own (its keyring): Backplane never reads a Google token, and the agent runs signed in as the person is in a terminal.
//!
//! The Backplane MCP server: agy reads MCP servers only from `~/.gemini/config/mcp_config.json` (no flag, no per-session file), and starts a stdio
//! server with its own environment. So `prepare` keeps one entry there, `backplane`, a stdio command that runs this hub's `--mcp-stdio` bridge
//! (agents/mcp_stdio.rs) on `BACKPLANE_MCP_URL`, the thread's endpoint for this process. Outside Backplane the variables are unset and it exits at once.
//!
//! Approvals: headless agy cannot ask, it denies what its mode does not allow and names it in `denied_actions`. Backplane's modes map to agy's:
//! supervised is agy's own review mode (with the person's allow rules in ~/.gemini/settings.json), accept edits `--mode accept-edits`, auto
//! `--mode auto`, full access `--dangerously-skip-permissions`; a denial is said on the thread with what to change.
use super::{TurnIn};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const MCP_NAME: &str = "backplane";
/// The stdio command agy starts for the `backplane` server: the hub's bridge when a Backplane turn started it, nothing otherwise.
pub const MCP_SH: &str = "[ -n \"$BACKPLANE_MCP_URL\" ] && [ -x \"$BACKPLANE_HUB_EXE\" ] && exec \"$BACKPLANE_HUB_EXE\" --mcp-stdio; exit 0";
pub fn mcp_entry() -> Value { json!({"command": "sh", "args": ["-c", MCP_SH]}) }

/// How agy is started: `sh -c RUN agy ARGS...` (the hub resolves `sh`; agy is the person's own binary, run as it is).
pub const RUN: &str = "exec \"$0\" \"$@\"";

/// The person's agy, found where the hub finds provider CLIs (its PATH, the login shell's, ~/.local/bin, ...). In tests only `BACKPLANE_TEST_AGY`.
pub fn find() -> Option<PathBuf> {
    if cfg!(test) { return std::env::var_os("BACKPLANE_TEST_AGY").map(PathBuf::from).filter(|p| p.is_file()) }
    crate::agents::proc::search_dirs().iter().map(|d| d.join("agy")).find(|p| p.is_file())
}
/// Where agy reads its MCP servers: `$HOME/.gemini/config/mcp_config.json` (the HOME the agent runs with).
pub fn mcp_config(home: &Path) -> PathBuf { home.join(".gemini/config/mcp_config.json") }

/// What `ensure_mcp` did: the entry was there, was written, or the file is not one Backplane can safely edit (said on the thread).
#[derive(Debug, PartialEq)]
pub enum Mcp { Had, Wrote, Skipped(String) }
/// Keeps `backplane` in agy's mcp_config.json, leaving every other server and key as it is. A file that is not plain JSON (agy also takes
/// comments and trailing commas) or a `backplane` server that is the person's own is left alone.
pub fn ensure_mcp(path: &Path) -> Mcp {
    let mut o: Value = match std::fs::read_to_string(path) {
        Ok(t) if t.trim().is_empty() => json!({}),
        Ok(t) => match serde_json::from_str::<Value>(&t) { Ok(v) if v.is_object() => v, _ => return Mcp::Skipped(format!("{} is not plain JSON, so Backplane left it alone", path.display())) },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Mcp::Skipped(format!("cannot read {}: {e}", path.display())),
    };
    if !o.get("mcpServers").map_or(true, Value::is_object) { return Mcp::Skipped(format!("{} has an mcpServers that is not an object", path.display())) }
    let servers = o.as_object_mut().unwrap().entry("mcpServers").or_insert_with(|| json!({}));
    let ours = mcp_entry();
    match servers.get(MCP_NAME) {
        Some(v) if *v == ours => return Mcp::Had,
        Some(v) if !v.to_string().contains("BACKPLANE_MCP_URL") => return Mcp::Skipped(format!("{} already has a server named {MCP_NAME} of its own", path.display())),
        _ => {}
    }
    servers.as_object_mut().unwrap().insert(MCP_NAME.into(), ours);
    let text = serde_json::to_string_pretty(&o).unwrap_or_default() + "\n";
    let dir = path.parent().unwrap_or(Path::new("."));
    let r = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tmp = dir.join(format!(".mcp_config.json.bp.{}.{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::write(&tmp, &text)?; crate::plat::fs::rename(&tmp, path)
    })();
    match r { Ok(()) => Mcp::Wrote, Err(e) => Mcp::Skipped(format!("cannot write {}: {e}", path.display())) }
}

/// agy's flags for Backplane's permission mode.
pub fn mode_args(runtime: &str) -> Vec<String> {
    match runtime { "approval-required" => vec![], "auto-accept-edits" => vec!["--mode".into(), "accept-edits".into()], "auto" => vec!["--mode".into(), "auto".into()], _ => vec!["--dangerously-skip-permissions".into()] }
}
pub const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
/// The nearest level `bs` offers to `want` on Backplane's scale (a tie goes to the lower).
pub fn nearest(want: &str, levels: &[String]) -> Option<String> {
    let at = |l: &str| super::agy::SCALE.iter().position(|x| *x == l).map(|i| i as i64);
    let w = at(want)?; levels.iter().filter_map(|l| at(l).map(|i| ((i - w).abs(), i, l.clone()))).min().map(|t| t.2)
}
/// The thread's model and effort as agy takes them: `--model <base>-<level>` when agy has the model as variants (effort "" is the medium one, or the
/// nearest it has; a level it lacks maps to the nearest), the model as given otherwise, and `--effort` only for agy's own default model or one it did not list.
pub fn resolve(model: &str, effort: &str, bases: &[super::agy::Base]) -> (Option<String>, Option<String>) {
    let eff = if EFFORTS.contains(&effort) { Some(effort.to_string()) } else { None };
    if model.is_empty() || model == "default" { return (None, eff) }
    let (b, own) = super::agy::split_variant(model);
    let found = bases.iter().find(|x| x.id == model).or_else(|| bases.iter().find(|x| x.id == b && own.is_some()));
    match found {
        Some(base) if !base.levels.is_empty() => {
            let want = if !effort.is_empty() && super::agy::SCALE.contains(&effort) { effort } else { own.unwrap_or("medium") };
            let pick = nearest(want, &base.levels).unwrap_or_else(|| base.levels[0].clone());
            (Some(format!("{}-{pick}", base.id)), None)
        }
        Some(base) => (Some(base.id.clone()), None),
        None => (Some(model.to_string()), if own.is_some() { None } else { eff }),
    }
}
/// The command line after `agy`.
pub fn args(a: &TurnIn, bases: &[super::agy::Base]) -> Vec<String> {
    let mut v: Vec<String> = ["--print=", "--input-format", "stream-json", "--output-format", "stream-json"].iter().map(|s| s.to_string()).collect();
    if !a.session.is_empty() { v.push("--conversation".into()); v.push(a.session.clone()) }
    let (m, e) = resolve(&a.model, &a.effort, bases);
    if let Some(m) = m { v.push("--model".into()); v.push(m) }
    if let Some(e) = e { v.push("--effort".into()); v.push(e) }
    v.extend(mode_args(&a.runtime)); v
}
/// The one input line: the person's text (Backplane's instructions first in a new conversation; image attachments named by path).
pub fn input(a: &TurnIn) -> String {
    let mut text = if a.session.is_empty() && !a.prompt.is_empty() { format!("<backplane_instructions>\n{}\n</backplane_instructions>\n\n{}", a.prompt, a.text) } else { a.text.clone() };
    for p in &a.images { text.push_str(&format!("\n[attached image: {p}]")) }
    json!({"event":"user","message":{"role":"user","content":text}}).to_string()
}

fn gs<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn n(v: &Value, k: &str) -> u64 { v.get(k).and_then(Value::as_u64).unwrap_or(0) }
fn ev(ty: &str, kvs: Value) -> Value { let mut m = serde_json::Map::new(); m.insert("type".into(), json!(ty)); if let Value::Object(o) = kvs { m.extend(o) } Value::Object(m) }

/// A tool step's one line: the tool and what it was given (its command, path or query), as the activity row shows it.
pub fn tool_line(info: &Value, name: &str) -> String {
    let p = info.get("parameters").unwrap_or(&Value::Null);
    let first = ["CommandLine", "AbsolutePath", "TargetFile", "DirectoryPath", "SearchPath", "Query", "Url", "Pattern", "ServerName"].iter().map(|k| gs(p, k)).find(|s| !s.is_empty());
    let mcp = { let (s, t) = (gs(p, "ServerName"), gs(p, "ToolName")); if t.is_empty() { String::new() } else if s.is_empty() { t.to_string() } else { format!("{s}.{t}") } };
    match (name, first) {
        ("run_command", Some(c)) => c.to_string(),
        (_, _) if !mcp.is_empty() => format!("{name} {mcp}"),
        (_, Some(x)) => format!("{name} {x}"),
        _ => name.to_string(),
    }
}
pub const QUOTA_MSG: &str = "Antigravity's quota for this Google account is used up. Backplane does not retry or move to another account: send again once it resets (agy -p /quota shows it).";
pub fn hint(msg: &str) -> String {
    let l = msg.to_lowercase();
    if ["429", "quota", "resource_exhausted", "rate limit", "rate-limit", "too many requests"].iter().any(|k| l.contains(k)) { return format!("{msg} ({QUOTA_MSG})") }
    if ["auth", "sign in", "signin", "login", "credential", "token"].iter().any(|k| l.contains(k)) { format!("{msg} (sign in once by running `agy` in a terminal)") } else { msg.into() }
}
/// Said when headless agy denied something the thread's mode does not allow.
pub fn denied_note(xs: &[Value]) -> String {
    let names: Vec<String> = xs.iter().map(|x| { let d = gs(x, "display_name"); if d.is_empty() { gs(x, "action").to_string() } else { d.to_string() } }).filter(|s| !s.is_empty()).collect();
    format!("Antigravity was not allowed to: {}. agy cannot ask for approval when Backplane runs it, so switch the thread to Auto or Full access, or add an allow rule under permissions.allow in ~/.gemini/settings.json.", names.join(", "))
}

/// One turn's state: the conversation, the reply text of the step being written, and whether the turn has ended.
#[derive(Debug, Default)]
pub struct St { pub session: String, pub step: i64, pub buf: String, pub done: bool, pub note: String, pub last_err: String }

/// One stdout line from agy, as Codex shapes.
pub fn line(s: &mut St, o: &Value) -> Vec<Value> {
    let mut out = vec![];
    if let Value::String(l) = o { let l = l.trim(); if l.starts_with("error:") { s.last_err = l.trim_start_matches("error:").trim().to_string() } return out }
    match gs(o, "event") {
        "init" => {
            let c = gs(o, "conversation_id");
            if !c.is_empty() && c != s.session { s.session = c.into(); out.push(ev("thread.started", json!({"thread_id": c}))) }
            if !s.note.is_empty() { out.push(ev("notice", json!({"message": std::mem::take(&mut s.note)}))) }
        }
        "step_update" => {
            let u = o.get("step_update").unwrap_or(&Value::Null); let idx = u.get("step_index").and_then(Value::as_i64).unwrap_or(-1);
            let (state, ty) = (gs(u, "state"), gs(u, "step_type"));
            match ty {
                "agent_response" => {
                    if idx != s.step { flush(s, &mut out); s.step = idx }
                    let t = gs(u, "text_delta"); if !t.is_empty() { s.buf.push_str(t); out.push(ev("item.delta", json!({"text": t}))) }
                    if state == "DONE" { flush(s, &mut out) }
                }
                "tool" => {
                    flush(s, &mut out);
                    let id = format!("agy-{}-{idx}", if s.session.is_empty() { "s" } else { &s.session });
                    let info = u.get("tool_info").unwrap_or(&Value::Null); let name = { let x = gs(u, "tool_name"); if x.is_empty() { gs(info, "name") } else { x } };
                    match state {
                        "ACTIVE" => out.push(ev("item.completed", json!({"item":{"type":"command_execution","id":id,"command":tool_line(info, name)}}))),
                        "DONE" | "ERROR" | "FAILED" | "CANCELED" | "CANCELLED" => out.push(ev("tool.done", json!({"id": id}))),
                        _ => {}
                    }
                }
                "thinking" | "planner_response" => { let t = gs(u, "text_delta"); if !t.is_empty() { out.push(ev("item.completed", json!({"item":{"type":"reasoning","text":t}}))) } }
                _ => {}
            }
        }
        "result" => {
            let r = o.get("result").unwrap_or(&Value::Null); flush(s, &mut out); s.done = true;
            let c = gs(r, "conversation_id"); if !c.is_empty() && c != s.session { s.session = c.into(); out.push(ev("thread.started", json!({"thread_id": c}))) }
            if let Some(xs) = r.get("denied_actions").and_then(Value::as_array).filter(|x| !x.is_empty()) { out.push(ev("notice", json!({"message": denied_note(xs)}))) }
            if gs(r, "status") == "SUCCESS" {
                let u = r.get("usage").unwrap_or(&Value::Null);
                out.push(ev("turn.completed", json!({"usage":{"input_tokens":n(u, "input_tokens"),"cached_input_tokens":n(u, "cache_read_tokens"),"output_tokens":n(u, "output_tokens"),"reasoning_output_tokens":n(u, "thinking_tokens")}})));
            } else {
                let e = { let e = gs(r, "error"); if !e.is_empty() { e.to_string() } else if !s.last_err.is_empty() { s.last_err.clone() } else { format!("Antigravity ended the turn: {}", { let st = gs(r, "status"); if st.is_empty() { "no result" } else { st } }) } };
                out.push(ev("turn.failed", json!({"message": hint(&e)})));
            }
        }
        _ => {}
    }
    out
}
fn flush(s: &mut St, out: &mut Vec<Value>) { if !s.buf.trim().is_empty() { out.push(ev("item.completed", json!({"item":{"type":"agent_message","text":std::mem::take(&mut s.buf)}}))) } s.buf.clear() }

#[cfg(test)]
mod tests {
    use super::*;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    fn feed(s: &mut St, text: &str) -> Vec<String> { text.lines().filter(|l| !l.trim().is_empty()).flat_map(|l| line(s, &serde_json::from_str(l).unwrap_or(Value::String(l.into())))).map(|v| v.to_string()).collect() }
    fn ti(session: &str, runtime: &str) -> TurnIn { TurnIn { session: session.into(), runtime: runtime.into(), text: "hi".into(), prompt: "RULES".into(), model: "gemini-4-argon".into(), effort: "high".into(), ..Default::default() } }

    fn bases() -> Vec<super::super::agy::Base> { super::super::agy::fold(&super::super::agy::cli_models(include_str!("../../../../../test/fixtures/antigravity/cli-models.txt"))) }
    #[test] fn effort_picks_the_variant() {
        let b = bases(); let r = |m: &str, e: &str| resolve(m, e, &b);
        assert_eq!(r("gemini-4-carbon", "low"), (Some("gemini-4-carbon-low".into()), None));
        assert_eq!(r("gemini-4-carbon", "high"), (Some("gemini-4-carbon-high".into()), None));
        assert_eq!(r("gemini-4-carbon", ""), (Some("gemini-4-carbon-medium".into()), None), "auto is the medium variant");
        assert_eq!(r("gemini-4-carbon", "max"), (Some("gemini-4-carbon-high".into()), None), "a level agy lacks maps to the nearest");
        assert_eq!(r("gemini-4-carbon", "minimal"), (Some("gemini-4-carbon-low".into()), None));
        assert_eq!(r("gpt-oss-120b", "high"), (Some("gpt-oss-120b-medium".into()), None), "its one variant");
        assert_eq!(r("gemini-4-carbon-high", ""), (Some("gemini-4-carbon-high".into()), None), "a thread that kept a variant id keeps it");
        assert_eq!(r("gemini-4-carbon-high", "low"), (Some("gemini-4-carbon-low".into()), None), "and the effort picker moves it");
        assert_eq!(r("", "high"), (None, Some("high".into()))); assert_eq!(r("default", "auto"), (None, None));
        assert_eq!(r("new-model", "low"), (Some("new-model".into()), Some("low".into())), "a model agy did not list is passed as given");
        assert_eq!(resolve("gemini-4-carbon", "low", &[]), (Some("gemini-4-carbon".into()), Some("low".into())), "no list yet: as given");
    }
    #[test] fn args_mode_model_effort_resume() {
        assert_eq!(args(&ti("", "full-access"), &bases()), ["--print=", "--input-format", "stream-json", "--output-format", "stream-json", "--model", "gemini-4-argon-high", "--dangerously-skip-permissions"]);
        let v = args(&TurnIn { session: "C1".into(), runtime: "approval-required".into(), model: "default".into(), effort: "auto".into(), ..Default::default() }, &[]);
        assert_eq!(v[5..], ["--conversation", "C1"], "supervised adds no flag, the default model and an effort agy does not take are left out");
        assert_eq!(mode_args("auto-accept-edits"), ["--mode", "accept-edits"]); assert_eq!(mode_args("auto"), ["--mode", "auto"]); assert_eq!(mode_args(""), ["--dangerously-skip-permissions"]);
    }
    #[test] fn input_has_the_instructions_only_in_a_new_conversation() {
        let i = j(&input(&ti("", "auto"))); assert_eq!(i["event"], "user"); assert_eq!(i["message"]["role"], "user");
        assert!(i["message"]["content"].as_str().unwrap().starts_with("<backplane_instructions>\nRULES\n</backplane_instructions>\n\nhi"));
        assert_eq!(j(&input(&ti("C1", "auto")))["message"]["content"], "hi");
        let mut a = ti("C1", "auto"); a.images = vec!["/p/x.png".into()]; assert_eq!(j(&input(&a))["message"]["content"], "hi\n[attached image: /p/x.png]");
    }
    // test/fixtures/antigravity/cli-turn.jsonl: a real agy 1.4.2 turn (a command, then the reply)
    #[test] fn a_real_turn() {
        let mut s = St::default(); let got = feed(&mut s, include_str!("../../../../../test/fixtures/antigravity/cli-turn.jsonl"));
        assert_eq!(got[0], r#"{"type":"thread.started","thread_id":"e6a565ab-b096-4285-a77b-0e43cf2c73ed"}"#);
        assert!(got.contains(&r#"{"type":"item.completed","item":{"type":"command_execution","id":"agy-e6a565ab-b096-4285-a77b-0e43cf2c73ed-2","command":"ls"}}"#.to_string()), "{got:#?}");
        assert!(got.contains(&r#"{"type":"tool.done","id":"agy-e6a565ab-b096-4285-a77b-0e43cf2c73ed-2"}"#.to_string()));
        assert!(got.iter().filter(|l| l.contains("item.delta")).count() >= 3, "the reply streams");
        let msg: Vec<&String> = got.iter().filter(|l| l.contains("agent_message")).collect(); assert_eq!(msg.len(), 1); assert!(msg[0].contains("The files in the current directory"));
        assert_eq!(got.last().unwrap(), r#"{"type":"turn.completed","usage":{"input_tokens":10434,"cached_input_tokens":6166,"output_tokens":168,"reasoning_output_tokens":52}}"#);
        assert!(s.done);
    }
    // test/fixtures/antigravity/cli-turn-1.5.5.jsonl: agy 1.5.5 (Nov 10), same flags and events as 1.4: a command, then the reply
    #[test] fn a_real_turn_on_1_5_5() {
        let mut s = St::default(); let got = feed(&mut s, include_str!("../../../../../test/fixtures/antigravity/cli-turn-1.5.5.jsonl"));
        assert!(got[0].starts_with(r#"{"type":"thread.started","thread_id":"#));
        assert!(got.iter().any(|l| l.contains("command_execution") && l.contains("ls")) && got.iter().any(|l| l.contains("tool.done")), "{got:#?}");
        assert!(got.iter().any(|l| l.contains("agent_message") && l.contains("DONE"))); assert!(got.last().unwrap().contains("turn.completed")); assert!(s.done);
    }
    #[test] fn a_resumed_turn_keeps_its_conversation_and_denials_are_said() {
        let mut s = St { session: "C1".into(), ..Default::default() };
        let got = feed(&mut s, r#"{"event":"init","conversation_id":"C1","init":{}}
{"event":"result","result":{"conversation_id":"C1","status":"SUCCESS","response":"","usage":{"input_tokens":5,"output_tokens":1},"denied_actions":[{"action":"command","display_name":"RunCommand"}]}}"#);
        assert!(!got.iter().any(|l| l.contains("thread.started")), "the thread's own conversation is kept");
        assert!(got[0].contains("not allowed to: RunCommand") && got[0].contains("\"notice\"")); assert!(got[1].contains("turn.completed"));
    }
    #[test] fn errors_fail_the_turn_with_a_way_on() {
        let mut s = St::default();
        let got = feed(&mut s, "error: not authenticated\n{\"event\":\"result\",\"result\":{\"status\":\"ERROR\",\"error\":\"\"}}");
        assert_eq!(got, [r#"{"type":"turn.failed","message":"not authenticated (sign in once by running `agy` in a terminal)"}"#]);
        let mut s = St::default(); let got = feed(&mut s, r#"{"event":"result","result":{"status":"ERROR","error":"Unknown model ID: x"}}"#);
        assert_eq!(got, [r#"{"type":"turn.failed","message":"Unknown model ID: x"}"#]);
    }
    #[test] fn quota_is_said_never_retried() {
        let mut s = St::default(); let got = feed(&mut s, r#"{"event":"result","result":{"status":"ERROR","error":"429 RESOURCE_EXHAUSTED"}}"#);
        assert_eq!(got.len(), 1); assert!(got[0].contains("turn.failed") && got[0].contains("does not retry or move to another account"));
    }
    #[test] fn tool_lines() {
        assert_eq!(tool_line(&j(r#"{"parameters":{"CommandLine":"ls -l"}}"#), "run_command"), "ls -l");
        assert_eq!(tool_line(&j(r#"{"parameters":{"ServerName":"backplane","ToolName":"thread_list"}}"#), "call_mcp_tool"), "call_mcp_tool backplane.thread_list");
        assert_eq!(tool_line(&j(r#"{"parameters":{"AbsolutePath":"/a/b"}}"#), "view_file"), "view_file /a/b"); assert_eq!(tool_line(&Value::Null, "wait"), "wait");
    }
    #[test] fn the_mcp_entry_is_kept_and_nothing_else_is_touched() {
        let d = super::super::muse::e2e::dir("agy-cli-mcp"); let p = mcp_config(&d);
        assert_eq!(ensure_mcp(&p), Mcp::Wrote); let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap(); assert_eq!(v["mcpServers"]["backplane"], mcp_entry());
        assert_eq!(ensure_mcp(&p), Mcp::Had);
        std::fs::write(&p, r#"{"other":1,"mcpServers":{"aws":{"command":"uvx"},"backplane":{"command":"sh","args":["-c","old BACKPLANE_MCP_URL"]}}}"#).unwrap();
        assert_eq!(ensure_mcp(&p), Mcp::Wrote); let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["other"], 1); assert_eq!(v["mcpServers"]["aws"]["command"], "uvx"); assert_eq!(v["mcpServers"]["backplane"], mcp_entry());
        let keys: Vec<&String> = v["mcpServers"].as_object().unwrap().keys().collect(); assert_eq!(keys, ["aws", "backplane"], "the person's order is kept");
        std::fs::write(&p, r#"{"mcpServers":{"backplane":{"url":"https://mine"}}}"#).unwrap(); assert!(matches!(ensure_mcp(&p), Mcp::Skipped(w) if w.contains("of its own")));
        std::fs::write(&p, "{ // a comment\n}").unwrap(); assert!(matches!(ensure_mcp(&p), Mcp::Skipped(w) if w.contains("not plain JSON"))); assert_eq!(std::fs::read_to_string(&p).unwrap(), "{ // a comment\n}");
        assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1, "no temporary file is left");
    }
    #[test] fn the_mcp_command_does_nothing_outside_backplane() {
        let o = std::process::Command::new("sh").args(["-c", MCP_SH]).env_remove("BACKPLANE_MCP_URL").output().unwrap(); assert!(o.status.success() && o.stdout.is_empty());
        let d = super::super::muse::e2e::dir("agy-cli-mcpsh"); let exe = d.join("hub"); std::fs::write(&exe, "#!/bin/sh\necho \"bridge $1 $BACKPLANE_MCP_URL\"\n").unwrap();
        { use crate::plat::fs::PermissionsExt; std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        let o = std::process::Command::new("sh").args(["-c", MCP_SH]).env("BACKPLANE_MCP_URL", "http://u").env("BACKPLANE_HUB_EXE", &exe).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout), "bridge --mcp-stdio http://u\n");
    }
}

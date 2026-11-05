//! Google Antigravity (src/core/agy.bend, src/server/agyio.bend, server.bend's Agt.go): Backplane runs Google's own agent, `agy_acp_server`, pinned by
//! version, size and SHA-256 under `<home>/antigravity/<version>`, and speaks ACP to it like Grok, one process per turn, started through the runner
//! script `<home>/antigravity/agy.sh` (the agent's profile is its GEMINI_HOME; only the method's own API key from the API keys file reaches it).
//! Backplane never reads a Google token: the agent keeps its own in the profile, and nothing here opens a profile file.
//!
//! Ported: the turn (initialize, authenticate, session new/resume, mode, model, prompt), the runner script, the pinned releases and install script,
//! methods/accounts/settings, models cache, the probe's info, the sign-in state machine (`AgySign`) and the callback check.
//! Not ported here (IO the agents lane's Settings job host owns, no agent process): running the install script, the probe, the sign-in process,
//! sign-out and forwarding Google's way back; each is a shell script this file builds, as in agyio.bend, and the Rust job host only has to run it.
use super::{AskInfo, Driver, Made, Reaction, TurnIn};
use serde_json::{json, Value};

pub const VERSION: &str = "1.3.0";
/// Agy.rels: (uname, url, size, sha256)
pub const RELS: [(&str, &str, &str, &str); 5] = [
    ("Linux-x86_64", "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.3.0-linux-x86_64.zip", "333727150", "9fb60956af0a9d76220a4db91ca9ac88e2a2372ad68f985ab5fceace6b825b96"),
    ("Linux-aarch64", "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.3.0-linux-arm64.zip", "321690363", "500b0bc0fb858e88f4df404d4cedf80bf9298c178291e39e383d6c50b111cbdf"),
    ("Linux-arm64", "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-1.3.0-linux-arm64.zip", "321690363", "500b0bc0fb858e88f4df404d4cedf80bf9298c178291e39e383d6c50b111cbdf"),
    ("Darwin-arm64", "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-1.3.0-darwin-arm64.zip", "111456962", "7cd97045f7b4fe81175a107cdf16f9c51484e3c78a5162cae415338bb6aa5b88"),
    ("Darwin-x86_64", "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-1.3.0-darwin-x86_64.zip", "117245544", "bb23956b89984bf5d354af2c3725e6c57f0cc1b7228e77a0e91c9c2bc1d47646"),
];
pub fn root(home: &str) -> String { format!("{home}/antigravity") }
pub fn dir(root: &str) -> String { format!("{root}/{VERSION}") }

/// Agy.install.sh: `sh -c SCRIPT sh ROOT [URL SIZE SHA]`; prints "ok" last, or a line saying why not. Size and hash are checked before anything is unpacked.
pub fn install_sh() -> String {
    let arms: String = RELS.iter().map(|(n, u, s, h)| format!("  {n}) u='{u}' n={s} s={h};;\n")).collect();
    format!("d=$1; v={VERSION}\n\
fail() {{ echo \"$*\"; rm -rf \"$d/dl\" \"$d/$v.new\"; exit 1; }}\n\
if [ -n \"$2\" ]; then u=$2 n=$3 s=$4; else\n\
case \"$(uname -s)-$(uname -m)\" in\n{arms}  *) fail \"Antigravity has no build for $(uname -s) $(uname -m)\";;\n\
esac; fi\n\
mkdir -p \"$d/dl\" || fail \"cannot write $d\"\n\
z=\"$d/dl/agy.zip\"\n\
curl -fsSL --retry 3 --connect-timeout 20 -o \"$z\" \"$u\" || fail \"the download failed\"\n\
[ \"$(wc -c < \"$z\" | tr -d ' ')\" = \"$n\" ] || fail \"the download is not the size Backplane pins\"\n\
if command -v sha256sum >/dev/null 2>&1; then h=$(sha256sum \"$z\" | cut -d' ' -f1); else h=$(shasum -a 256 \"$z\" | cut -d' ' -f1); fi\n\
[ \"$h\" = \"$s\" ] || fail \"the download's SHA-256 is not the one Backplane pins\"\n\
rm -rf \"$d/$v.new\"; mkdir -p \"$d/$v.new\"\n\
if command -v unzip >/dev/null 2>&1; then unzip -q -o \"$z\" -d \"$d/$v.new\"; elif command -v bsdtar >/dev/null 2>&1; then bsdtar -xf \"$z\" -C \"$d/$v.new\"; else python3 -m zipfile -e \"$z\" \"$d/$v.new\"; fi || fail \"the archive did not unpack\"\n\
chmod 755 \"$d/$v.new/agy_acp_server.par\" \"$d/$v.new/localharness_external\" || fail \"the archive has no agy_acp_server.par\"\n\
rm -rf \"$d/$v\" && mv \"$d/$v.new\" \"$d/$v\" && touch \"$d/$v/.ok\" || fail \"cannot move the agent into place\"\n\
rm -rf \"$d/dl\"\n\
for o in \"$d\"/*/.ok; do o=${{o%/.ok}}; [ -f \"$o/.ok\" ] && [ \"$o\" != \"$d/$v\" ] && rm -rf \"$o\"; done\n\
echo ok\n")
}
/// Agy.install.err: "" when done, else why not (its last line).
pub fn install_err(out: &str) -> String {
    let last = out.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if last == "ok" { String::new() } else if last.is_empty() { "the install failed".into() } else { last.into() }
}

/// Agy.run.sh: `agy.sh DIR PROFILE METHOD KEYS SETTINGS` runs the agent with its profile as GEMINI_HOME, the method's settings.json, only the method's own
/// key from the API keys file (none from the hub's environment), a temporary folder of its own, and BROWSER pointing back at itself so a browser never
/// opens on the hub's machine (`agy.sh --open URL` prints BACKPLANE-SIGNIN <url>). A missing agent says BACKPLANE-MISSING.
pub fn run_sh() -> String {
    "#!/bin/sh\n\
# Backplane's runner for Google's Antigravity agent (src/core/agy.bend)\n\
if [ \"$1\" = --open ]; then printf 'BACKPLANE-SIGNIN %s\\n' \"$2\"; exit 0; fi\n\
d=$1 p=$2 m=$3 k=$4\n\
if [ ! -x \"$d/agy_acp_server.par\" ]; then echo BACKPLANE-MISSING; exit 1; fi\n\
umask 077\n\
mkdir -p \"$p/antigravity-acp\" && printf '%s\\n' \"$5\" > \"$p/antigravity-acp/settings.json\" || exit 1\n\
t=\"$p/tmp/$$\"; mkdir -p \"$t\"\n\
case \"$m\" in gemini-api-key) n=GEMINI_API_KEY;; agent-platform) n=GOOGLE_API_KEY;; *) n=;; esac\n\
key=; if [ -n \"$n\" ] && [ -f \"$k\" ]; then key=$(unset \"$n\"; . \"$k\" >/dev/null 2>&1; eval \"printf %s \\\"\\${$n:-}\\\"\"); fi\n\
unset GEMINI_API_KEY GOOGLE_API_KEY GOOGLE_CLOUD_PROJECT GOOGLE_CLOUD_LOCATION GOOGLE_GENAI_USE_VERTEXAI AGY_ACP_ENABLE_OAUTH AGY_ACP_CCPA_PROJECT ELECTRON_RUN_AS_NODE\n\
[ -n \"$key\" ] && export \"$n=$key\"\n\
export GEMINI_HOME=\"$p\" AGY_ACP_FORCE_FILE_STORAGE=1 PYTHONUNBUFFERED=1 TMPDIR=\"$t\" ANTIGRAVITY_HARNESS_PATH=\"$d/localharness_external\" BROWSER=\"'$0' --open %s\"\n\
set -- \"$d/agy_acp_server.par\"; [ \"$(uname -s)\" = Linux ] && set -- \"$@\" --uid=\n\
exec 3<&0\n\
\"$@\" <&3 3<&- 2>\"$p/agy.log\" &\n\
c=$!\n\
trap 'kill $c 2>/dev/null' TERM INT HUP\n\
wait $c; r=$?\n\
while kill -0 $c 2>/dev/null; do wait $c; r=$?; done\n\
rm -rf \"$t\"\n\
exit $r\n".into()
}
/// Agx.write: the runner script, written whole and renamed into place (turns may be starting from it).
pub fn write_runner(root: &str) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(root)?;
    // a name of its own per call: two turns starting together never write the same file (pid + a counter, as pi.rs's write_atomic)
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = format!("{root}/agy.sh.{}.{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)); std::fs::write(&tmp, run_sh())?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?; std::fs::rename(&tmp, format!("{root}/agy.sh"))
}

// ---- Methods and accounts ----
pub const METHODS: [&str; 4] = ["gemini-api-key", "agent-platform", "oauth-business", "oauth-personal"];
fn pos(m: &str) -> usize { METHODS.iter().position(|x| *x == m).unwrap_or(0) }
/// Agy.method: a setting's method, the API key for anything not known (law agy_default_api_key).
pub fn method(m: &str) -> &'static str { METHODS[pos(m)] }
pub fn label(m: &str) -> &'static str { ["API key", "Vertex AI", "Gemini Enterprise", "Google account"][pos(m)] }
pub fn browser(m: &str) -> bool { method(m).starts_with("oauth-") }
pub fn key_name(m: &str) -> &'static str { ["GEMINI_API_KEY", "GOOGLE_API_KEY", "", ""][pos(m)] }
pub fn gcp(m: &str) -> bool { matches!(method(m), "oauth-business" | "agent-platform") }
pub fn warn() -> &'static str { "Google's Antigravity terms restrict third-party clients: signing in with your Google account here could get it suspended. An API key is safer." }
/// Agy.note.k (law agy_google_warns: note.k(3) == warn)
pub fn note_k(i: usize) -> &'static str {
    match i { 0 => "Uses GEMINI_API_KEY from API keys. Google suggests a key for agents outside its own apps.", 1 => "Uses GOOGLE_API_KEY from API keys, or Application Default Credentials with a Google Cloud project and location.",
        2 => "Signs in with Google for Gemini Enterprise; needs a Google Cloud project and location.", _ => warn() }
}
pub fn note(m: &str) -> &'static str { note_k(pos(m)) }
/// Agy.acct: an account's name as a folder: lower case letters, digits, - and _, at most 32 ("" is "default").
pub fn acct(raw: &str) -> String {
    let s: String = raw.trim().chars().filter_map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' { Some(c) } else if c.is_ascii_uppercase() { Some(c.to_ascii_lowercase()) } else { None }).take(32).collect();
    if s.is_empty() { "default".into() } else { s }
}
pub fn profile(root: &str, a: &str) -> String { format!("{root}/profiles/{}", acct(a)) }
/// Agy.settings: the agent's settings.json for a method: auth.type and, when given, the Google Cloud project and location (never a credential).
pub fn settings(m: &str, project: &str, location: &str) -> String {
    let mut g = serde_json::Map::new();
    if !project.trim().is_empty() { { g.insert("project".into(), json!(project.trim())); } }
    if !location.trim().is_empty() { { g.insert("location".into(), json!(location.trim())); } }
    let mut o = serde_json::Map::new(); o.insert("auth".into(), json!({"type": method(m)}));
    if !g.is_empty() { { o.insert("gcp".into(), Value::Object(g)); } }
    Value::Object(o).to_string()
}
/// Agy.args: the script, the agent's folder, the profile, the method, the API keys file and the settings.
pub fn args(root: &str, keys: &str, m: &str, a: &str, project: &str, location: &str) -> Vec<String> {
    vec![format!("{root}/agy.sh"), dir(root), profile(root, a), method(m).into(), keys.into(), settings(m, project, location)]
}
/// Agy.mode.k: default asks, auto_edit lets edits through, yolo allows everything.
pub fn mode_k(i: u8) -> &'static str { match i { 1 => "auto_edit", 3 => "yolo", _ => "default" } }
pub fn mode(runtime: &str) -> &'static str { mode_k(match runtime { "approval-required" => 0, "auto-accept-edits" => 1, "auto" => 2, _ => 3 }) }

// ---- ACP, as acp.bend handles it for Grok (a private copy; the ACP lane's acp.rs is the shared one) ----
fn gs<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn event(ty: &str, kvs: Value) -> Value { let mut m = serde_json::Map::new(); m.insert("type".into(), json!(ty)); if let Value::Object(o) = kvs { for (k, v) in o { m.insert(k, v); } } Value::Object(m) }
fn failed(msg: &str) -> Value { event("turn.failed", json!({"message": msg})) }
fn rpc(id: u64, method: &str, params: Value) -> String { json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string() }
fn mcp(url: &str) -> Value { json!([{"type":"http","name":"backplane","url":url,"headers":[]}]) }

/// AcpTurn
#[derive(Clone, Debug, Default)]
pub struct AcpTurn { pub cwd: String, pub mcp: String, pub prompt: String, pub text: String, pub session: String, pub ask: bool }
/// AgyTurn
#[derive(Clone, Debug, Default)]
pub struct AgyTurn { pub acp: AcpTurn, pub method: String, pub mode: String, pub model: String }
/// Acp{sid, buf, done}
#[derive(Clone, Debug, Default)]
pub struct Acp { pub sid: String, pub buf: String, pub done: bool }
#[derive(Debug, Default)]
pub struct Out { pub send: Vec<String>, pub emit: Vec<Value>, pub asks: Vec<String> }
fn out_emit(e: Vec<Value>) -> Out { Out { emit: e, ..Default::default() } }
fn out_send(s: Vec<String>) -> Out { Out { send: s, ..Default::default() } }

pub fn init() -> String { rpc(1, "initialize", json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false}})) }
pub fn auth(m: &str) -> String { rpc(4, "authenticate", json!({"methodId": method(m)})) }
fn new_rpc(id: u64, cwd: &str, mcp: Value) -> String { rpc(id, "session/new", json!({"cwd":cwd,"mcpServers":mcp})) }
fn open(t: &AcpTurn) -> String {
    if t.session.is_empty() { new_rpc(2, &t.cwd, mcp(&t.mcp)) } else { rpc(2, "session/resume", json!({"sessionId":t.session,"cwd":t.cwd,"mcpServers":mcp(&t.mcp)})) }
}
fn set_mode(id: u64, sid: &str, mode: &str) -> String { rpc(id, "session/set_mode", json!({"sessionId":sid,"modeId":mode})) }
fn set_model(id: u64, sid: &str, model: &str) -> String { rpc(id, "session/set_config_option", json!({"sessionId":sid,"configId":"model","value":model})) }
/// Acp.say.k: the prompt; a new session gets Backplane's instructions first.
fn say_rpc(fresh: bool, t: &AcpTurn, sid: &str) -> String {
    let text = if fresh { format!("<backplane_instructions>\n{}\n</backplane_instructions>\n\n{}", t.prompt, t.text) } else { t.text.clone() };
    rpc(3, "session/prompt", json!({"sessionId":sid,"prompt":[{"type":"text","text":text}]}))
}
pub fn hint(msg: &str) -> String {
    if ["auth", "API key", "credential", "sign"].iter().any(|k| msg.contains(k)) { format!("{msg} (Settings › Agents › Antigravity: add a key or sign in)") } else { msg.into() }
}
/// Acp.hint: ACP's own failure on the prompt (id 3) says how Grok signs in, as the JS hub's shared reply does.
fn acp_hint(msg: &str) -> String { let l = msg.to_lowercase(); if l.contains("auth") || l.contains("login") { format!("{msg} (sign in once with `grok login`)") } else { msg.into() } }
fn err_msg(o: &Value) -> String { o.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).unwrap_or("grok failed").to_string() }
pub const SIGNIN_MSG: &str = "Antigravity is not signed in: sign in under Settings › Agents › Antigravity first.";
pub const MISSING_MSG: &str = "Antigravity is not installed: install it under Settings › Agents › Antigravity.";

fn update(a: &mut Acp, u: &Value) -> Vec<Value> {
    match gs(u, "sessionUpdate") {
        "agent_message_chunk" => { let t = u.get("content").map(|c| gs(c, "text")).unwrap_or(""); a.buf.push_str(t); vec![event("item.delta", json!({"text": t}))] }
        "tool_call" => { let (t, k) = (gs(u, "title"), gs(u, "kind")); let c = if !t.is_empty() { t } else if !k.is_empty() { k } else { "tool" };
            vec![event("item.completed", json!({"item":{"type":"command_execution","id":gs(u, "toolCallId"),"command":c}}))] }
        "plan" => { let xs: Vec<Value> = u.get("entries").and_then(Value::as_array).map(|e| e.iter().map(|x| { let s = gs(x, "status"); json!({"text":gs(x, "content"),"status":if s.is_empty() { "pending" } else { s }}) }).collect()).unwrap_or_default();
            vec![event("item.updated", json!({"item":{"type":"todo_list","items":xs}}))] }
        "tool_call_update" => if matches!(gs(u, "status"), "completed" | "failed") { vec![event("tool.done", json!({"id": gs(u, "toolCallId")}))] } else { vec![] },
        _ => vec![],
    }
}
fn finished(a: &mut Acp) -> Vec<Value> {
    a.done = true; let mut v = vec![];
    if !a.buf.is_empty() { v.push(event("item.completed", json!({"item":{"type":"agent_message","text":a.buf}}))) }
    v.push(event("turn.completed", json!({}))); v
}
/// Acp.permit: full access allows at once: the first option whose kind starts with "allow", else the first option.
pub fn permit_auto(id: &Value, params: &Value) -> String {
    let os = params.get("options").and_then(Value::as_array).cloned().unwrap_or_default();
    let first = os.first().map(|o| gs(o, "optionId")).unwrap_or("").to_string();
    let pick = os.iter().find(|o| gs(o, "kind").starts_with("allow")).map(|o| gs(o, "optionId").to_string()).unwrap_or(first);
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{{\"outcome\":{{\"outcome\":\"selected\",\"optionId\":{}}}}}}}", Value::String(pick))
}
/// Acp.permit.reply: "accept" picks the allow-once option, "session" the allow-always one, anything else a reject option; with none, the request is cancelled.
pub fn permit_reply(id: &Value, params: &Value, answer: &str) -> String {
    let os = params.get("options").and_then(Value::as_array).cloned().unwrap_or_default();
    let find = |k: &str| os.iter().find(|o| gs(o, "kind") == k).map(|o| gs(o, "optionId").to_string()).unwrap_or_default();
    let or = |a: String, b: String| if a.is_empty() { b } else { a };
    let opt = match answer { "accept" => or(find("allow_once"), find("allow_always")), "session" => or(find("allow_always"), find("allow_once")), _ => or(find("reject_once"), find("reject_always")) };
    let outcome = if opt.is_empty() { json!({"outcome":"cancelled"}) } else { json!({"outcome":"selected","optionId":opt}) };
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{}}}", json!({"outcome": outcome}))
}
/// The response for a parked request line (its raw text), once the person answered.
pub fn permit_answer(body: &str, answer: &str) -> String { let v: Value = serde_json::from_str(body).unwrap_or(Value::Null); permit_reply(v.get("id").unwrap_or(&Value::Null), v.get("params").unwrap_or(&Value::Null), answer) }
pub fn tool(params: &Value) -> String { let tc = params.get("toolCall").unwrap_or(&Value::Null); let k = gs(tc, "kind"); if k.is_empty() { let t = gs(tc, "title"); if t.is_empty() { "tool".into() } else { t.into() } } else { k.into() } }
pub fn detail(params: &Value) -> String {
    let tc = params.get("toolCall").unwrap_or(&Value::Null); let raw = tc.get("rawInput").unwrap_or(&Value::Null);
    for k in ["command", "path", "file_path", "filePath"] { let s = gs(raw, k); if !s.is_empty() { return s.into() } }
    let t = gs(tc, "title"); if !t.is_empty() { t.into() } else { raw.to_string().chars().take(400).collect() }
}

/// Acp.line for a method line: session/update (only once the session is open), session/request_permission, else not supported.
fn acp_request(a: &mut Acp, t: &AcpTurn, o: &Value) -> Out {
    let id = o.get("id");
    match gs(o, "method") {
        "session/update" => if a.sid.is_empty() { Out::default() } else { out_emit(update(a, o.get("params").and_then(|p| p.get("update")).unwrap_or(&Value::Null))) },
        "session/request_permission" => {
            if t.ask { Out { asks: vec![o.to_string()], ..Default::default() } }
            else { out_send(vec![permit_auto(id.unwrap_or(&Value::Null), o.get("params").unwrap_or(&Value::Null))]) } }
        _ => match id { Some(id) => out_send(vec![format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":-32601,\"message\":\"not supported\"}}}}")]), None => Out::default() },
    }
}

fn fail(a: &mut Acp, msg: &str) -> Out { a.done = true; out_emit(vec![failed(msg)]) }
fn opened(fresh: bool, a: &mut Acp, t: &AgyTurn, sid: &str) -> Out { a.sid = sid.into(); Out { send: vec![set_mode(if fresh { 5 } else { 15 }, sid, &t.mode)], emit: vec![event("thread.started", json!({"thread_id": sid}))], ..Default::default() } }
fn say(fresh: bool, a: &Acp, t: &AgyTurn) -> Out { out_send(vec![say_rpc(fresh, &t.acp, &a.sid)]) }
fn moded(fresh: bool, a: &Acp, t: &AgyTurn) -> Out { if t.model.is_empty() { say(fresh, a, t) } else { out_send(vec![set_model(if fresh { 6 } else { 16 }, &a.sid, &t.model)]) } }
fn unmodel(t: &AgyTurn) -> String { format!("Antigravity has no model {} for this sign-in: pick another in the composer.", t.model) }

/// Agy.reply.k: replies to our requests, by id.
fn reply(a: &mut Acp, t: &AgyTurn, o: &Value) -> Out {
    let id = o.get("id").map(|v| match v { Value::Number(n) => n.to_string(), v => v.to_string() }).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let ok = o.get("error").map_or(true, Value::is_null); let fresh_at_open = t.acp.session.is_empty();
    let res_sid = |d: &str| o.get("result").map(|r| gs(r, "sessionId")).filter(|s| !s.is_empty()).unwrap_or(d).to_string();
    match (id, ok) {
        (1, true) => out_send(vec![auth(&t.method)]),
        (4, true) => out_send(vec![open(&t.acp)]),
        (2, true) => { let sid = res_sid(&t.acp.session); opened(fresh_at_open, a, t, &sid) }
        (2, false) => if fresh_at_open { fail(a, &hint(&err_msg(o))) } else { out_send(vec![new_rpc(7, &t.acp.cwd, mcp(&t.acp.mcp))]) },
        (7, true) => { let sid = o.get("result").map(|r| gs(r, "sessionId")).unwrap_or("").to_string(); opened(true, a, t, &sid) }
        (5, _) => moded(true, a, t),
        (15, _) => moded(false, a, t),
        (6, true) => say(true, a, t),
        (16, true) => say(false, a, t),
        (6, false) | (16, false) => fail(a, &unmodel(t)),
        (3, true) => out_emit(finished(a)),
        (3, false) => fail(a, &acp_hint(&err_msg(o))),
        (_, false) => fail(a, &hint(&err_msg(o))),
        (_, true) => Out::default(),
    }
}
/// Agy.plain: a line that is not JSON: the runner's own words, or the agent's noise.
fn plain(a: &mut Acp, line: &str) -> Out {
    let l = line.trim();
    if l.starts_with("BACKPLANE-SIGNIN ") { fail(a, SIGNIN_MSG) } else if l.starts_with("BACKPLANE-MISSING") { fail(a, MISSING_MSG) } else { Out::default() }
}
/// Agy.line: one line from the agent (`None` json: the raw text of a line that did not parse).
pub fn line(a: &mut Acp, t: &AgyTurn, o: &Value) -> Out {
    match o {
        Value::Object(_) => if gs(o, "method").is_empty() { reply(a, t, o) } else { acp_request(a, &t.acp, o) },
        Value::String(s) => plain(a, s),
        _ => Out::default(),
    }
}

// ---- Models ----
// session/new answers the models this sign-in offers: configOptions' "model" select (its options, or groups of them), else models' availableModels.
fn vals(xs: &[Value]) -> Vec<(String, String)> { xs.iter().filter(|o| !gs(o, "value").is_empty()).map(|o| (gs(o, "value").to_string(), { let n = gs(o, "name"); if n.is_empty() { gs(o, "value").into() } else { n.into() } })).collect() }
pub fn models(r: &Value) -> Vec<(String, String)> {
    let cfg = r.get("configOptions").and_then(Value::as_array).and_then(|a| a.iter().find(|o| gs(o, "id") == "model"));
    match cfg {
        Some(c) => c.get("options").and_then(Value::as_array).map(|os| os.iter().flat_map(|o| if gs(o, "value").is_empty() { vals(o.get("options").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])) } else { vals(std::slice::from_ref(o)) }).collect()).unwrap_or_default(),
        None => r.get("models").and_then(|m| m.get("availableModels")).and_then(Value::as_array).map(|a| a.iter().map(|o| (gs(o, "modelId").to_string(), { let n = gs(o, "name"); if n.is_empty() { gs(o, "modelId").into() } else { n.into() } })).collect()).unwrap_or_default(),
    }
}
pub fn current(r: &Value) -> String {
    match r.get("configOptions").and_then(Value::as_array).and_then(|a| a.iter().find(|o| gs(o, "id") == "model")) { Some(c) => gs(c, "currentValue").into(), None => r.get("models").map(|m| gs(m, "currentModelId")).unwrap_or("").into() }
}
/// Agy.cache: the cache's text for a session/new result.
pub fn cache(r: &Value) -> String { json!({"models": models(r).into_iter().map(|(i, n)| json!({"id":i,"name":n})).collect::<Vec<_>>(), "default": current(r)}).to_string() }

// ---- The sign-in ----
// One process: initialize (1), authenticate (4), and once that answers, a session (2, in the hub's home, no MCP) whose answer lists the models.
// What it says: "url\t<address>" (a Google sign-in to open), "ok\t<cache>" or "fail\t<why>"; done once ok or fail.
#[derive(Clone, Debug, Default)]
pub struct AgySign { pub url: String, pub done: bool }
#[derive(Debug, Default)]
pub struct SignOut { pub send: Vec<String>, pub say: Vec<String> }
fn sign_why(msg: &str) -> String { if msg.contains("SUBSCRIPTION_REQUIRED") { "Google needs an eligible Antigravity plan for this account.".into() } else if msg.contains("access_denied") || msg.contains("cancelled") { "Google's sign-in was not approved.".into() } else { msg.into() } }
impl AgySign {
    fn fail(&mut self, why: &str) -> SignOut { self.done = true; SignOut { say: vec![format!("fail\t{why}")], ..Default::default() } }
    /// AgySign.line: one line from the agent while signing in with method `m` (home: where its probe session opens).
    pub fn line(&mut self, m: &str, home: &str, o: &Value) -> SignOut {
        match o {
            Value::Object(_) => {
                if !gs(o, "method").is_empty() { return SignOut::default() }
                let id = o.get("id").and_then(Value::as_u64).unwrap_or(0); let ok = o.get("error").map_or(true, Value::is_null);
                match (id, ok) {
                    (1, true) => SignOut { send: vec![auth(m)], ..Default::default() },
                    (4, true) => SignOut { send: vec![new_rpc(2, home, json!([]))], ..Default::default() },
                    (2, true) => { self.done = true; SignOut { say: vec![format!("ok\t{}", cache(o.get("result").unwrap_or(&Value::Null)))], ..Default::default() } }
                    (_, false) => { let m = o.get("error").and_then(|e| e.get("message")).and_then(Value::as_str).unwrap_or("Antigravity could not sign in"); self.fail(&sign_why(m)) }
                    _ => SignOut::default(),
                } }
            Value::String(s) => { let l = s.trim();
                if let Some(u) = l.strip_prefix("BACKPLANE-SIGNIN ") { let u = u.trim(); if url_ok(u) { self.url = u.into(); SignOut { say: vec![format!("url\t{u}")], ..Default::default() } } else { self.fail("Antigravity gave a sign-in address that is not Google's") } }
                else if l.starts_with("BACKPLANE-MISSING") { self.fail(MISSING_MSG) } else { SignOut::default() } }
            _ => SignOut::default(),
        }
    }
}

// ---- The way back from Google ----
// Google sends the browser to the agent's loopback address. From another device the page fails to load and the person pastes its address: it is taken
// only if it is the redirect this sign-in asked for (same port and path), carries this sign-in's state once and one code or error.
fn unpct(s: &str) -> String {
    let hex = |c: char| c.to_digit(16).unwrap_or(0); let cs: Vec<char> = s.chars().collect(); let mut o = String::new(); let mut i = 0;
    while i < cs.len() { match cs[i] { '%' => { let hi = cs.get(i + 1).map_or(0, |c| hex(*c)); let lo = cs.get(i + 2).map_or(0, |c| hex(*c)); o.push(char::from_u32(hi * 16 + lo).unwrap_or('\u{fffd}')); i += 3 } '+' => { o.push(' '); i += 1 } c => { o.push(c); i += 1 } } }
    o
}
fn query(u: &str) -> Vec<&str> { match u.split_once('?') { Some((_, q)) => q.split('&').collect(), None => vec![""] } }
fn params(u: &str, name: &str) -> Vec<String> { query(u).into_iter().filter_map(|p| p.strip_prefix(&format!("{name}=")).map(unpct)).collect() }
fn param(u: &str, name: &str) -> String { let mut v = params(u, name); if v.len() == 1 { v.remove(0) } else { String::new() } }
pub fn redirect(u: &str) -> String { param(u, "redirect_uri") }
/// Agy.url.ok: a sign-in address the agent prints, checked: Google's OAuth page, one state, a loopback redirect on a port of its own.
pub fn url_ok(u: &str) -> bool {
    u.starts_with("https://accounts.google.com/o/oauth2/v2/auth?") && u.len() < 16384 && !u.contains(' ') && redirect(u).starts_with("http://127.0.0.1:") && !param(u, "state").is_empty()
}
fn trailing(r: &str) -> String { if r.ends_with('/') { r.into() } else { format!("{r}/") } }
/// Agy.callback.ok: whether a pasted address answers the sign-in that asked at `url` (law agy_callback_needs_signin: no address is fetched for none).
pub fn callback_ok(url: &str, back: &str) -> bool {
    url_ok(url) && trailing(back.trim().split('?').next().unwrap_or("")) == trailing(&redirect(url))
        && params(back, "state").len() == 1 && param(back, "state") == param(url, "state")
        && params(back, "code").len() + params(back, "error").len() == 1 && !back.contains('#')
}

// ---- What the hub knows (Agy.probe.sh, Agy.info) ----
pub fn probe_sh() -> String {
    format!("d=$1; [ -f \"$d/{VERSION}/.ok\" ] && echo have {VERSION}\n\
for o in \"$d\"/*/.ok; do [ -f \"$o\" ] && echo old \"$(basename \"$(dirname \"$o\")\")\"; done\n\
echo plat \"$(uname -s)-$(uname -m)\"\n\
for p in \"$d\"/profiles/*/; do [ -d \"$p\" ] || continue; s=0; [ -f \"$p/antigravity-acp/acp_token.json\" ] && s=1; echo acct $s \"$(basename \"$p\")\"; done\n\
[ -f \"$d/models.json\" ] && printf 'models %s\\n' \"$(tr -d '\\n' < \"$d/models.json\")\"\n\
exit 0\n")
}
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Info { pub have: String, pub old: String, pub plat: String, pub accts: Vec<String>, pub signed: Vec<String>, pub cache: String }
pub fn info(out: &str) -> Info {
    let mut a = Info::default();
    for l in out.lines().map(str::trim) {
        match l.split(' ').next().unwrap_or("") {
            "have" => a.have = l[5.min(l.len())..].into(),
            "old" => { let v = &l[4.min(l.len())..]; a.old = if v == VERSION { a.old.clone() } else { v.into() } }
            "plat" => a.plat = l[5.min(l.len())..].into(),
            "acct" => { let v = &l[7.min(l.len())..]; a.accts.push(v.into()); if l.starts_with("acct 1 ") { a.signed.push(v.into()) } }
            "models" => a.cache = l[7.min(l.len())..].into(),
            _ => {}
        }
    }
    a
}
/// Agy.info.json: the "agy" object in the models info.
pub fn info_json(a: &Info) -> Value { json!({"pin":VERSION,"have":a.have,"old":a.old,"plat":RELS.iter().any(|r| r.0 == a.plat),"accts":a.accts,"signed":a.signed}) }
/// Agy.info.models: the models a client lists (none until the agent is installed).
pub fn info_models(a: &Info) -> Vec<String> {
    if a.have.is_empty() { return vec![] }
    serde_json::from_str::<Value>(&a.cache).ok().and_then(|c| c["models"].as_array().map(|m| m.iter().map(|x| gs(x, "id").to_string()).collect())).unwrap_or_default()
}
/// Agy.info.about: "about" for the picker.
pub fn info_about(a: &Info) -> Value {
    let c: Value = serde_json::from_str(&a.cache).unwrap_or(Value::Null); let dm = gs(&c, "default").to_string();
    let mut labels = serde_json::Map::new(); labels.insert("default".into(), json!("Antigravity's own default"));
    for m in c["models"].as_array().cloned().unwrap_or_default() { let id = gs(&m, "id"); { labels.insert(id.into(), json!(format!("{}{}", gs(&m, "name"), if id == dm { " · Antigravity's default" } else { "" }))); } }
    json!({"cli": if a.have.is_empty() { String::new() } else { format!("Antigravity {}", a.have) }, "labels": labels})
}

// ---- The driver ----
/// Agt.go: the runner script with the sign-in the settings name (agy.auth, agy.account, agy.gcp.project, agy.gcp.location).
pub struct AgyDriver { turn: AgyTurn, st: Acp }
impl AgyDriver { pub fn new() -> AgyDriver { AgyDriver { turn: AgyTurn::default(), st: Acp::default() } } pub fn done(&self) -> bool { self.st.done } }
impl Driver for AgyDriver {
    /// Agx.write before the run: the runner script, renamed into place, so a turn starting from it never sees half of it.
    fn prepare(&mut self, a: &TurnIn) -> Result<(), String> { write_runner(&root(&a.home)).map_err(|e| e.to_string()) }
    fn argv(&self, a: &TurnIn) -> Vec<String> {
        let r = root(&a.home);
        args(&r, &format!("{}/secrets/api.env", a.home), a.setting("agy.auth"), a.setting("agy.account"), a.setting("agy.gcp.project"), a.setting("agy.gcp.location"))
    }
    fn start(&mut self, a: &TurnIn) -> Vec<String> {
        self.turn = AgyTurn { acp: AcpTurn { cwd: a.cwd.clone(), mcp: a.mcp_url.clone(), prompt: a.prompt.clone(), text: a.text.clone(), session: a.session.clone(), ask: mode_runtime(&a.runtime) != 3 },
            method: method(a.setting("agy.auth")).into(), mode: mode(&a.runtime).into(), model: a.model.clone() };
        self.st = Acp::default(); vec![init()]
    }
    fn on_line(&mut self, v: &Value) -> Reaction {
        let o = line(&mut self.st, &self.turn, v);
        let mut shapes = o.emit; for r in o.asks { shapes.push(json!({"type":"bp.ask","request":r})) }
        Reaction { send: o.send, shapes }
    }
    /// Grok.permit: a session/request_permission (the raw line) is an approval card.
    fn ask_info(&self, request: &Value) -> Option<AskInfo> {
        let v: Value = serde_json::from_str(request.as_str()?).ok()?; if v["method"] != "session/request_permission" { return None }
        let p = v.get("params").unwrap_or(&Value::Null);
        Some(AskInfo { kind: "approval".into(), tool: tool(p), detail: detail(p), options: String::new() })
    }
    fn answer(&mut self, request: &Value, answer: &str) -> Vec<String> { request.as_str().map(|r| vec![permit_answer(r, answer)]).unwrap_or_default() }
    fn ends_on_turn_end(&self) -> bool { true }
    /// Proto.install
    fn install_hint(&self) -> Option<String> { Some("Install it under Settings › Agents › Antigravity.".into()) }
    fn exit_why(&self, _code: i32) -> Option<String> { Some("antigravity exited before the turn ended (is it installed and signed in? see Settings › Agents › Antigravity)".into()) }
}
fn mode_runtime(r: &str) -> u8 { match r { "approval-required" => 0, "auto-accept-edits" => 1, "auto" => 2, _ => 3 } }
pub fn driver(provider: &str) -> Option<Made> { if provider == "antigravity" { Some(Made { cli: "sh", driver: Box::new(AgyDriver::new()) }) } else { None } }

#[cfg(test)]
mod tests {
    use super::*;
    fn j(s: &str) -> Value { serde_json::from_str(s).unwrap() }
    fn turn(session: &str, model: &str, ask: bool) -> AgyTurn { AgyTurn { acp: AcpTurn { cwd: "/tmp/p".into(), mcp: "http://127.0.0.1:1/mcp/t?token=x".into(), prompt: "RULES".into(), text: "hello".into(), session: session.into(), ask }, method: "gemini-api-key".into(), mode: "default".into(), model: model.into() } }
    fn m(l: &str) -> String { j(l)["method"].as_str().unwrap_or("").to_string() }
    fn id(l: &str) -> u64 { j(l)["id"].as_u64().unwrap() }
    fn feed(a: &mut Acp, t: &AgyTurn, l: &str) -> Out { line(a, t, &j(l)) }
    fn emits(o: &Out) -> String { o.emit.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" | ") }

    #[test] fn agy_laws() {
        assert_eq!(mode_k(0), "default"); assert_eq!(mode_k(3), "yolo"); assert_eq!(mode_k(1), "auto_edit"); assert_eq!(mode("approval-required"), "default"); assert_eq!(mode("full-access"), "yolo");
        assert_eq!(method(""), "gemini-api-key"); assert_eq!(method("nonsense"), "gemini-api-key"); assert_eq!(method("oauth-personal"), "oauth-personal");
        assert_eq!(note_k(3), warn()); assert_eq!(note("oauth-personal"), warn()); assert!(!callback_ok("", "http://127.0.0.1:1/?code=x&state=s"));
        assert_eq!(key_name("agent-platform"), "GOOGLE_API_KEY"); assert!(browser("oauth-business") && !browser("")); assert!(gcp("agent-platform") && !gcp("oauth-personal"));
    }
    #[test] fn args_settings_accounts() {
        assert_eq!(acct(" My Acct!"), "myacct"); assert_eq!(acct(""), "default"); assert_eq!(acct(&"a".repeat(40)).len(), 32);
        assert_eq!(settings("", "", ""), r#"{"auth":{"type":"gemini-api-key"}}"#); assert_eq!(settings("oauth-business", " p ", "us"), r#"{"auth":{"type":"oauth-business"},"gcp":{"project":"p","location":"us"}}"#);
        assert_eq!(args("/h/antigravity", "/h/secrets/api.env", "", "", "", ""), ["/h/antigravity/agy.sh", "/h/antigravity/1.3.0", "/h/antigravity/profiles/default", "gemini-api-key", "/h/secrets/api.env", r#"{"auth":{"type":"gemini-api-key"}}"#]);
    }
    #[test] fn the_runner_gives_the_agent_only_its_own_key_and_never_reads_a_token() {
        let s = run_sh(); assert!(s.contains("unset GEMINI_API_KEY GOOGLE_API_KEY GOOGLE_CLOUD_PROJECT")); assert!(s.contains("gemini-api-key) n=GEMINI_API_KEY;; agent-platform) n=GOOGLE_API_KEY;; *) n=;;"));
        assert!(!s.contains("acp_token.json"), "the runner never names the agent's token"); assert!(s.contains("BACKPLANE-MISSING") && s.contains("--open"));
        assert!(!install_sh().is_empty() && install_sh().contains("333727150") && install_sh().contains("SHA-256 is not the one Backplane pins"));
    }
    // The recorded exchange of test/fixtures/antigravity/turn.jsonl: the real agent's replies (1.3.0).
    #[test] fn a_fresh_turn_through_the_fixture() {
        let t = turn("", "", true); let mut a = Acp::default();
        let o = feed(&mut a, &t, r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1}}"#); assert_eq!(id(&o.send[0]), 4); assert_eq!(m(&o.send[0]), "authenticate");
        let o = feed(&mut a, &t, r#"{"jsonrpc":"2.0","id":4,"result":{}}"#); assert_eq!(m(&o.send[0]), "session/new");
        let o = feed(&mut a, &t, r#"{"jsonrpc":"2.0","id":2,"result":{"sessionId":"S1"}}"#); assert_eq!(emits(&o), r#"{"type":"thread.started","thread_id":"S1"}"#); assert_eq!(id(&o.send[0]), 5); assert_eq!(j(&o.send[0])["params"]["modeId"], "default");
        let o = feed(&mut a, &t, r#"{"jsonrpc":"2.0","id":5,"result":{}}"#); assert_eq!(m(&o.send[0]), "session/prompt"); assert!(o.send[0].contains("<backplane_instructions>\\nRULES"));
        let u = |s: &str| format!(r#"{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"S1","update":{s}}}}}"#);
        assert_eq!(emits(&feed(&mut a, &t, &u(r#"{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"ECHO"}}"#))), r#"{"type":"item.delta","text":"ECHO"}"#);
        assert_eq!(emits(&feed(&mut a, &t, &u(r#"{"sessionUpdate":"tool_call","toolCallId":"T1","title":"Run ls","kind":"execute"}"#))), r#"{"type":"item.completed","item":{"type":"command_execution","id":"T1","command":"Run ls"}}"#);
        assert_eq!(emits(&feed(&mut a, &t, &u(r#"{"sessionUpdate":"tool_call_update","toolCallId":"T1","status":"completed"}"#))), r#"{"type":"tool.done","id":"T1"}"#);
        let o = feed(&mut a, &t, r#"{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}"#);
        assert_eq!(emits(&o), r#"{"type":"item.completed","item":{"type":"agent_message","text":"ECHO"}} | {"type":"turn.completed"}"#); assert!(a.done);
    }
    #[test] fn resume_model_mode_and_lost_sessions() {
        let t = turn("OLD", "gemini-x", false); let mut a = Acp::default();
        let o = feed(&mut a, &t, r#"{"id":4,"result":{}}"#); assert_eq!(m(&o.send[0]), "session/resume");
        let o = feed(&mut a, &t, r#"{"id":2,"result":{}}"#); assert_eq!(id(&o.send[0]), 15, "a resume answers no id: the thread's own is kept"); assert_eq!(a.sid, "OLD");
        let o = feed(&mut a, &t, r#"{"id":15,"result":{}}"#); assert_eq!(m(&o.send[0]), "session/set_config_option"); assert_eq!(id(&o.send[0]), 16);
        let o = feed(&mut a, &t, r#"{"id":16,"result":{}}"#); assert!(o.send[0].contains("\"text\":\"hello\"") && !o.send[0].contains("backplane_instructions"));
        let bad = feed(&mut a, &t, r#"{"id":16,"error":{"message":"no"}}"#); assert!(emits(&bad).contains("has no model gemini-x"));
        let mut b = Acp::default(); let o = feed(&mut b, &t, r#"{"id":2,"error":{"message":"session not found"}}"#); assert_eq!(id(&o.send[0]), 7, "a session the agent no longer has is opened afresh");
        let o = feed(&mut b, &t, r#"{"id":7,"result":{"sessionId":"NEW"}}"#); assert_eq!(emits(&o), r#"{"type":"thread.started","thread_id":"NEW"}"#); assert_eq!(id(&o.send[0]), 5);
        let mut c = Acp::default(); let o = feed(&mut c, &turn("", "", true), r#"{"id":2,"error":{"message":"missing API key"}}"#);
        assert_eq!(emits(&o), r#"{"type":"turn.failed","message":"missing API key (Settings › Agents › Antigravity: add a key or sign in)"}"#);
    }
    #[test] fn permissions_ask_unless_full_access() {
        let req = r#"{"jsonrpc":"2.0","id":9,"method":"session/request_permission","params":{"sessionId":"S1","toolCall":{"kind":"execute","title":"rm"},"options":[{"optionId":"a1","kind":"allow_once"},{"optionId":"a2","kind":"allow_always"},{"optionId":"r1","kind":"reject_once"}]}}"#;
        let mut a = Acp { sid: "S1".into(), ..Default::default() };
        let o = feed(&mut a, &turn("", "", true), req); assert_eq!(o.asks.len(), 1); assert!(o.send.is_empty());
        let o = feed(&mut a, &turn("", "", false), req); assert_eq!(o.send, vec![r#"{"jsonrpc":"2.0","id":9,"result":{"outcome":{"outcome":"selected","optionId":"a1"}}}"#]);
        assert!(permit_answer(req, "accept").contains("\"a1\"")); assert!(permit_answer(req, "session").contains("\"a2\"")); assert!(permit_answer(req, "no").contains("\"r1\""));
        assert!(permit_answer(r#"{"id":1,"params":{"options":[]}}"#, "accept").contains("cancelled"));
        assert_eq!(tool(&j(req)["params"]), "execute");
    }
    #[test] fn the_runner_words_fail_the_turn() {
        let t = turn("", "", true); let mut a = Acp::default();
        assert_eq!(emits(&line(&mut a, &t, &Value::String("BACKPLANE-MISSING".into()))), format!("{{\"type\":\"turn.failed\",\"message\":\"{MISSING_MSG}\"}}"));
        let mut b = Acp::default(); assert!(emits(&line(&mut b, &t, &Value::String("BACKPLANE-SIGNIN https://x".into()))).contains("not signed in")); assert!(b.done);
        let mut c = Acp::default(); assert!(line(&mut c, &t, &Value::String("noise".into())).emit.is_empty());
    }
    #[test] fn models_come_from_config_options_or_available_models() {
        let r = j(r#"{"sessionId":"S","configOptions":[{"id":"mode"},{"id":"model","currentValue":"m1","options":[{"value":"m1","name":"M One"},{"name":"Group","options":[{"value":"m2"}]}]}]}"#);
        assert_eq!(models(&r), [("m1".into(), "M One".into()), ("m2".into(), "m2".into())]); assert_eq!(current(&r), "m1");
        assert_eq!(cache(&r), r#"{"models":[{"id":"m1","name":"M One"},{"id":"m2","name":"m2"}],"default":"m1"}"#);
        let r2 = j(r#"{"models":{"currentModelId":"x","availableModels":[{"modelId":"x","name":"X"}]}}"#); assert_eq!(models(&r2), [("x".into(), "X".into())]); assert_eq!(current(&r2), "x");
        let i = info("have 1.3.0\nold 1.2.0\nplat Linux-x86_64\nacct 1 default\nacct 0 other\nmodels {\"models\":[{\"id\":\"m1\",\"name\":\"M\"}],\"default\":\"m1\"}\n");
        assert_eq!(i.signed, ["default"]); assert_eq!(i.accts, ["default", "other"]); assert_eq!(info_models(&i), ["m1"]); assert_eq!(info_json(&i)["plat"], true);
        assert_eq!(info_about(&i)["labels"]["m1"], "M · Antigravity's default"); assert!(info_models(&Info::default()).is_empty());
    }
    #[test] fn signing_in() {
        let mut s = AgySign::default(); let o = s.line("oauth-personal", "/h", &j(r#"{"id":1,"result":{}}"#)); assert_eq!(m(&o.send[0]), "authenticate");
        let url = "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&redirect_uri=http%3A%2F%2F127.0.0.1%3A5555%2F&state=ST1";
        let o = s.line("oauth-personal", "/h", &Value::String(format!("BACKPLANE-SIGNIN {url}"))); assert_eq!(o.say, [format!("url\t{url}")]);
        assert!(callback_ok(url, "http://127.0.0.1:5555/?code=C&state=ST1")); assert!(callback_ok(url, "http://127.0.0.1:5555?code=C&state=ST1"));
        assert!(!callback_ok(url, "http://127.0.0.1:5556/?code=C&state=ST1")); assert!(!callback_ok(url, "http://127.0.0.1:5555/?code=C&state=OTHER")); assert!(!callback_ok(url, "http://127.0.0.1:5555/?code=C&state=ST1&code=D"));
        assert!(!callback_ok(url, "http://127.0.0.1:5555/?state=ST1")); assert!(!url_ok("https://evil.example/o/oauth2/v2/auth?redirect_uri=http://127.0.0.1:1/&state=s"));
        let o = s.line("oauth-personal", "/h", &Value::String("BACKPLANE-SIGNIN http://evil/".into())); assert!(o.say[0].contains("not Google's"));
        let mut d = AgySign::default(); let o = d.line("", "/h", &j(r#"{"id":2,"result":{"configOptions":[{"id":"model","currentValue":"m","options":[{"value":"m","name":"M"}]}]}}"#)); assert!(o.say[0].starts_with("ok\t{\"models\"")); assert!(d.done);
        let mut e = AgySign::default(); assert_eq!(e.line("", "/h", &j(r#"{"id":4,"error":{"message":"SUBSCRIPTION_REQUIRED x"}}"#)).say[0], "fail\tGoogle needs an eligible Antigravity plan for this account.");
    }
    #[test] fn the_runner_is_written_whole_by_turns_starting_together() {
        let d = super::super::muse::e2e::dir("agy-runner"); let r = d.to_string_lossy().into_owned();
        let hs: Vec<_> = (0..16).map(|_| { let r = r.clone(); std::thread::spawn(move || { for _ in 0..20 { write_runner(&r).unwrap(); let s = std::fs::read_to_string(format!("{r}/agy.sh")).unwrap(); assert_eq!(s, run_sh(), "a reader never sees half of it") } }) }).collect();
        for h in hs { h.join().unwrap() }
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "no temporary file is left");
    }
    #[test] fn exit_words_name_the_code_and_hide_no_kill() {
        let d = driver("antigravity").unwrap().driver; let w = d.exit_why(1).unwrap(); use crate::agents::rules::exit_say;
        assert!(exit_say(&w, 137, "").starts_with("agent was killed (signal 9, likely out of memory) (signal 9)"), "an OOM-killed agent is not 'is it signed in?'");
        let x = exit_say(&w, 1, "boom"); assert!(x.starts_with("antigravity exited before the turn ended") && x.contains("(exit 1): boom"));
        assert!(d.install_hint().unwrap().contains("Settings › Agents › Antigravity"));
    }
    #[test] fn prepare_writes_the_runner_and_fails_the_turn_when_it_cannot() {
        let d = super::super::muse::e2e::dir("agy-prepare"); let mut dr = AgyDriver::new();
        dr.prepare(&TurnIn { home: d.to_string_lossy().into(), ..Default::default() }).unwrap(); assert_eq!(std::fs::read_to_string(d.join("antigravity/agy.sh")).unwrap(), run_sh());
        std::fs::write(d.join("blocker"), "x").unwrap(); assert!(dr.prepare(&TurnIn { home: d.join("blocker").to_string_lossy().into(), ..Default::default() }).is_err());
    }
    #[test] fn install_answers() { assert_eq!(install_err("working\nok\n"), ""); assert_eq!(install_err("x\nthe download failed\n"), "the download failed"); assert_eq!(install_err(""), "the install failed"); }
    #[test] fn the_driver() {
        let mut d = AgyDriver::new(); let a = TurnIn { thread: "t".into(), home: "/nonexistent-home".into(), cwd: "/tmp".into(), runtime: "approval-required".into(), settings: vec![("agy.auth".into(), "oauth-personal".into())], ..Default::default() };
        let s = d.start(&a); assert!(s[0].contains("\"initialize\"")); assert_eq!(d.turn.method, "oauth-personal");
        let r = d.on_line(&j(r#"{"id":1,"result":{}}"#)); assert!(r.send[0].contains("oauth-personal"));
        assert!(super::super::driver("antigravity").is_some());
        assert!(d.ends_on_turn_end() && d.exit_why(1).unwrap().contains("antigravity exited"));
        assert!(d.on_stop().is_empty(), "interrupt_antigravity_stops: Stop ends the process");
    }
}

#[cfg(test)]
mod standin_tests {
    use super::*;
    use super::super::muse::e2e::{self, project as _p, js, run, script};
    fn setup(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let d = e2e::dir(name); let ag = d.join("antigravity/1.3.0"); std::fs::create_dir_all(&ag).unwrap();
        script(&ag.join("agy_acp_server.par"), include_str!("../../../../../test/gpui/agents/prov/standin-agy.py"));
        std::fs::write(ag.join("agy-turn.jsonl"), include_str!("../../../../../test/gpui/agents/prov/agy-turn.jsonl")).unwrap();
        (d.clone(), d)
    }
    fn ti(d: &std::path::Path, runtime: &str, text: &str, session: &str, settings: Vec<(String, String)>) -> TurnIn {
        TurnIn { thread: "t1700000010-2".into(), cwd: d.to_string_lossy().into(), home: d.to_string_lossy().into(), runtime: runtime.into(), text: text.into(), session: session.into(),
            prompt: "RULES lls>".into(), mcp_url: "http://127.0.0.1:1/mcp/x".into(), gen: "1".into(), settings, ..Default::default() }
    }
    // Stand-in Antigravity (test/gpui/agents/prov/standin-agy.py behind the real runner script) through the driver: equal to the JS hub's events.
    #[tokio::test] async fn agy_turn_matches_the_js_hub() {
        let (d, _) = setup("agy-turn"); let mut dr = AgyDriver::new(); let a = ti(&d, "full-access", "zq hello", "", vec![]); dr.prepare(&a).unwrap();
        let mut argv = dr.argv(&a); let log = d.join("agy.log").to_string_lossy().into_owned();
        let script_path = argv.remove(0); let mut full = vec![script_path]; full.extend(argv);
        struct Sh(Vec<String>, AgyDriver); impl Driver for Sh { fn argv(&self, a: &TurnIn) -> Vec<String> { let _ = a; self.0.clone() } fn start(&mut self, a: &TurnIn) -> Vec<String> { self.1.start(a) } fn on_line(&mut self, l: &Value) -> Reaction { self.1.on_line(l) }
            fn ask_info(&self, r: &Value) -> Option<AskInfo> { self.1.ask_info(r) } fn answer(&mut self, r: &Value, x: &str) -> Vec<String> { self.1.answer(r, x) } }
        let mut sh = Sh(full, dr);
        let (got, wrote) = run(std::path::Path::new("/bin/sh"), &mut sh, &a, "antigravity", "accept", &[("AGY_LOG".into(), log.clone())]).await;
        assert_eq!(got, js("agy-turn"));
        assert!(wrote.iter().any(|w| w.contains("\"authenticate\"") || w.contains("\"initialize\"")));
        let l = std::fs::read_to_string(&log).unwrap(); assert!(l.contains("gemkey=") && !l.contains("gemkey=SECRET"), "no key from the hub's environment reaches the agent");
        assert!(l.contains(&format!("home={}/antigravity/profiles/default", d.display())), "the agent's profile is its GEMINI_HOME");
        let settings = std::fs::read_to_string(d.join("antigravity/profiles/default/antigravity-acp/settings.json")).unwrap(); assert_eq!(settings.trim(), r#"{"auth":{"type":"gemini-api-key"}}"#);
    }
    #[tokio::test] async fn agy_permission_matches_the_js_hub() {
        let (d, _) = setup("agy-perm"); let mut dr = AgyDriver::new(); let a = ti(&d, "approval-required", "zq perm please", "", vec![]); dr.prepare(&a).unwrap();
        let argv = dr.argv(&a);
        struct Sh(Vec<String>, AgyDriver); impl Driver for Sh { fn argv(&self, _a: &TurnIn) -> Vec<String> { self.0.clone() } fn start(&mut self, a: &TurnIn) -> Vec<String> { self.1.start(a) } fn on_line(&mut self, l: &Value) -> Reaction { self.1.on_line(l) }
            fn ask_info(&self, r: &Value) -> Option<AskInfo> { self.1.ask_info(r) } fn answer(&mut self, r: &Value, x: &str) -> Vec<String> { self.1.answer(r, x) } }
        let mut sh = Sh(argv, dr);
        let (got, wrote) = run(std::path::Path::new("/bin/sh"), &mut sh, &a, "antigravity", "accept", &[]).await;
        assert_eq!(got, js("agy-perm")); assert!(wrote.iter().any(|w| w.contains("\"optionId\":\"allow\"")), "the answer reached the agent as the allow option");
    }
}


/// The laws of src/core (LAWS.bend), one test each, named as the law: scripts/gpui/law-map.tsv maps them.
#[cfg(test)]
mod laws {
    use super::*;
    fn turn_in() -> TurnIn { TurnIn { thread: "t1".into(), cwd: "/work/p".into(), home: "/nonexistent-agy-home".into(), model: "gm".into(), session: "S9".into(), runtime: "approval-required".into(), text: "do it".into(), mcp_url: "http://x/mcp".into(), ..Default::default() } }
    #[test] fn agy_mode_supervised_asks() { assert_eq!(mode_k(0), "default"); assert_eq!(mode("approval-required"), "default") }
    #[test] fn agy_mode_full_allows() { assert_eq!(mode_k(3), "yolo"); assert_eq!(mode("full-access"), "yolo") }
    #[test] fn agy_default_api_key() { assert_eq!(method(""), "gemini-api-key") }
    #[test] fn agy_google_warns() { assert_eq!(note_k(3), warn()) }
    #[test] fn agy_callback_needs_signin() { for b in ["", "http://127.0.0.1:1/?code=a&state=b", "x"] { assert!(!callback_ok("", b)) } }
    /// Stop on an Antigravity thread is AgentStop alone: nothing is written to the process first.
    #[test] fn interrupt_antigravity_stops() { let mut d = driver("antigravity").unwrap().driver; d.start(&turn_in()); assert!(d.on_stop().is_empty()) }
    /// An Antigravity thread's turn is one run of Google's agent, through the runner script, with the thread's cwd, model, session, mode and text (over ACP).
    #[test] fn antigravity_run_turn() {
        let made = driver("antigravity").unwrap(); assert_eq!(made.cli, "sh"); assert!(driver("muse").is_none());
        let mut d = made.driver; let a = turn_in();
        let v = d.argv(&a); assert_eq!(v[0], "/nonexistent-agy-home/antigravity/agy.sh"); assert_eq!(v[3], "gemini-api-key");
        assert!(d.start(&a)[0].contains("\"initialize\""));
        let feed = |d: &mut Box<dyn Driver>, l: &str| d.on_line(&serde_json::from_str(l).unwrap());
        feed(&mut d, r#"{"id":1,"result":{}}"#); let r = feed(&mut d, r#"{"id":4,"result":{}}"#);
        assert!(r.send[0].contains("\"session/resume\"") && r.send[0].contains("\"S9\"") && r.send[0].contains("\"cwd\":\"/work/p\""));
        let r = feed(&mut d, r#"{"id":2,"result":{}}"#); assert!(r.send[0].contains("\"modeId\":\"default\""));
        let r = feed(&mut d, r#"{"id":15,"result":{}}"#); assert!(r.send[0].contains("\"value\":\"gm\""));
        let r = feed(&mut d, r#"{"id":16,"result":{}}"#); assert!(r.send[0].contains("session/prompt") && r.send[0].contains("do it"));
    }
}

//! opencode over ACP (`env OPENCODE_CONFIG_CONTENT=... opencode acp`): src/core/opencode (Ocp.*, Ocm.*) and server's Oc.*.
//! Grok's translator (acp.rs) with what opencode does its own way: session/resume (no replay), a lost session opened afresh and told once
//! (a `notice` shape), a tool call logged once at its first tool_call_update named by what it works on, the thread's model set on the session
//! (ids 5 fresh / 15 resumed) before the prompt, opencode's own failure hints. Request ids: 1 initialize, 2 session/new or resume, 7 session/new
//! after a lost session, 5/15 the model, 3 the prompt.
use super::acp::{self, Acp, AcpTurn, Out};
use super::{Driver, Made, Reaction, TurnIn};
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, Default)]
pub struct OcpTurn { pub acp: AcpTurn, pub model: String }

/// Ocp.picked: "" and "default" are opencode's own.
pub fn picked(m: &str) -> bool { !m.is_empty() && m != "default" }
fn set_model(id: u64, sid: &str, model: &str) -> String { acp::rpc(id, "session/set_config_option", json!({"sessionId":sid,"configId":"model","value":model})) }
fn new_rpc(id: u64, cwd: &str, mcp: &str) -> String { acp::rpc(id, "session/new", json!({"cwd":cwd,"mcpServers":acp::mcp(mcp)})) }
/// Ocp.open: a new session, or the thread's own resumed.
pub fn open(t: &AcpTurn) -> String {
    if t.session.is_empty() { new_rpc(2, &t.cwd, &t.mcp) } else { acp::rpc(2, "session/resume", json!({"sessionId":t.session,"cwd":t.cwd,"mcpServers":acp::mcp(&t.mcp)})) }
}
/// Ocp.rel: a path in the thread's folder, from it.
pub fn rel(cwd: &str, p: &str) -> String { if !cwd.is_empty() && p.starts_with(&format!("{cwd}/")) { p[cwd.len() + 1..].to_string() } else { p.into() } }
fn mark(id: &str) -> String { format!("\n{id}\n") }

fn signin(low: &str) -> bool { low.contains("auth") || low.contains("login") || low.contains("no models are available") || low.contains("api key") }
/// Ocp.hint: the reason, and how to fix it when that is known.
pub fn hint(msg: &str) -> String {
    let low = msg.to_lowercase();
    if low.contains("model not found") { format!("{msg} (pick another opencode model in the composer)") }
    else if low.contains("insufficient account funds") { format!("{msg} (the model's provider refused for its account's balance or quota: top it up, or pick another opencode model in the composer)") }
    else if low.contains("provider authentication required") { format!("{msg} (the model's provider refused opencode's sign-in, as an expired OAuth sign-in does: run `opencode auth login` for that provider; with no model picked, opencode's configured default agent chose it)") }
    else if signin(&low) { format!("{msg} (sign in once with `opencode auth login`)") } else { msg.into() }
}
/// Ocp.lost: opencode no longer has the session, or it was made in another folder.
pub fn lost(msg: &str) -> bool { msg.contains("session not found") || msg.contains("does not belong to cwd") }
fn fail(a: &mut Acp, msg: &str) -> Out { a.done = true; Out { send: vec![], shapes: vec![acp::failed(&hint(msg))] } }
fn err(o: &Value) -> String { acp::error(o, "opencode failed") }

fn opened(fresh: bool, a: &mut Acp, t: &OcpTurn, sid: &str) -> Out {
    a.sid = sid.into();
    let next = if picked(&t.model) { set_model(if fresh { 5 } else { 15 }, sid, &t.model) } else { acp::say(fresh, &t.acp, sid) };
    Out { send: vec![next], shapes: vec![acp::event("thread.started", json!({"thread_id":sid}))] }
}
/// Ocp.renewed.msg
pub fn renewed_msg(old: &str) -> String { format!("opencode could not resume its session {old}, so it started a new one without the earlier conversation. This thread's history in Backplane is kept.") }

fn reply(a: &mut Acp, t: &OcpTurn, o: &Value) -> Out {
    let ok = o.get("error").map_or(true, Value::is_null); let id = acp::num_id(o);
    let sid_of = |d: &str| o.get("result").and_then(|r| r.get("sessionId")).and_then(Value::as_str).unwrap_or(d).to_string();
    match (id, ok) {
        (1, true) => Out { send: vec![open(&t.acp)], shapes: vec![] },
        (2, true) => { let sid = sid_of(&t.acp.session); opened(t.acp.session.is_empty(), a, t, &sid) }
        (2, false) => if !t.acp.session.is_empty() && lost(&err(o)) { Out { send: vec![new_rpc(7, &t.acp.cwd, &t.acp.mcp)], shapes: vec![] } } else { fail(a, &err(o)) },
        (5, true) | (15, true) => Out { send: vec![acp::say(id == 5, &t.acp, &a.sid)], shapes: vec![] },
        (5, false) | (15, false) => fail(a, &format!("opencode refused the model {}: {}", t.model, err(o))),
        (7, true) => { let sid = sid_of(""); let mut r = opened(true, a, t, &sid); r.shapes.push(acp::event("notice", json!({"message":renewed_msg(&t.acp.session)}))); r }
        (3, true) => Out { send: vec![], shapes: acp::finished(a) },
        (_, false) => fail(a, &err(o)),
        _ => Out::default(),
    }
}

/// Ocp.where: what a call works on: its file, pattern, URL or query, else the first place it names.
pub fn where_(u: &Value, cwd: &str) -> String {
    let raw = u.get("rawInput").unwrap_or(&Value::Null);
    let a = acp::gs(raw, "filePath", acp::gs(raw, "path", ""));
    let p = acp::gs(raw, "pattern", acp::gs(raw, "url", acp::gs(raw, "query", "")));
    let l = u.get("locations").and_then(Value::as_array).and_then(|l| l.first()).map(|l| acp::gs(l, "path", "").to_string()).unwrap_or_default();
    rel(cwd, &acp::or(a, &acp::or(p, &l)))
}
/// Ocp.say.k: the tool's title (a shell's is its command) and what it works on, unless the title already says it.
pub fn say_k(title: &str, what: &str) -> String { if what.is_empty() || title.contains(what) { title.into() } else { format!("{title} {what}") } }
fn what(u: &Value, cwd: &str) -> String { let t = acp::s(u, "title"); let t = if t.is_empty() { let k = acp::s(u, "kind"); if k.is_empty() { "tool" } else { k } } else { t }; say_k(t, &where_(u, cwd)) }
fn finished(u: &Value) -> bool { matches!(acp::s(u, "status"), "completed" | "failed") }
fn done_line(u: &Value) -> Vec<Value> { if finished(u) { vec![acp::event("tool.done", json!({"id":acp::s(u, "toolCallId")}))] } else { vec![] } }

fn update(a: &mut Acp, t: &AcpTurn, o: &Value) -> Out {
    let u = o.get("params").and_then(|p| p.get("update")).unwrap_or(&Value::Null);
    match acp::s(u, "sessionUpdate") {
        "tool_call" => Out::default(), // no input yet: logged at its first update
        "tool_call_update" => {
            let id = acp::s(u, "toolCallId").to_string();
            if a.seen.contains(&mark(&id)) { Out { send: vec![], shapes: done_line(u) } }
            else { a.seen.push_str(&mark(&id)); let mut v = vec![acp::event("item.completed", json!({"item":{"type":"command_execution","id":id,"command":what(u, &t.cwd)}}))]; v.extend(done_line(u)); Out { send: vec![], shapes: v } }
        }
        _ => acp::request(a, t, o),
    }
}
/// Ocp.line: one line from opencode.
pub fn line(a: &mut Acp, t: &OcpTurn, o: &Value) -> Out {
    if !o.is_object() { return Out::default() }
    if acp::s(o, "method").is_empty() { return reply(a, t, o) }
    if acp::s(o, "method") == "session/update" && a.open_now() { update(a, &t.acp, o) } else { acp::request(a, &t.acp, o) }
}

/// Oc.perm / Oc.config / Oc.args: the model and permissions as config (no flag takes them).
pub fn config(model: &str, runtime: &str) -> String {
    let perm = match runtime { "approval-required" => "ask", "auto-accept-edits" => "edit", _ => "allow" };
    let mut m = Map::new(); if picked(model) { m.insert("model".into(), json!(model)); }
    m.insert("permission".into(), json!({"edit": if perm == "ask" { "ask" } else { "allow" }, "bash": if perm == "allow" { "allow" } else { "ask" }, "webfetch": if perm == "allow" { "allow" } else { "ask" }}));
    Value::Object(m).to_string()
}
pub const GONE: &str = "opencode exited before the turn ended (is it signed in? run `opencode auth login`)";
pub const INSTALL: &str = "Install opencode (https://opencode.ai), then sign in with `opencode auth login`.";

pub struct OcDriver { pub turn: OcpTurn, pub a: Acp }
impl OcDriver { pub fn new() -> OcDriver { OcDriver { turn: OcpTurn::default(), a: Acp::new() } } }
impl Driver for OcDriver {
    fn argv(&self, _: &TurnIn) -> Vec<String> { vec!["acp".into()] }
    fn env(&self, a: &TurnIn) -> Vec<(String, String)> { vec![("OPENCODE_CONFIG_CONTENT".into(), config(&a.model, &a.runtime))] }
    fn start(&mut self, a: &TurnIn) -> Vec<String> {
        self.turn = OcpTurn { acp: AcpTurn { cwd: a.cwd.clone(), mcp: a.mcp_url.clone(), prompt: a.prompt.clone(), text: a.text.clone(), session: a.session.clone(), ask: super::grok::asks(&a.runtime) }, model: a.model.clone() };
        vec![acp::init()]
    }
    fn on_line(&mut self, l: &Value) -> Reaction { let o = line(&mut self.a, &self.turn, l); Reaction { send: o.send, shapes: o.shapes } }
    fn ask_info(&self, r: &Value) -> Option<super::AskInfo> { acp::ask_of(r) }
    fn answer(&mut self, r: &Value, answer: &str) -> Vec<String> { acp::answer_of(r, answer) }
    fn ends_on_turn_end(&self) -> bool { true }
    fn exit_why(&self, _code: i32) -> Option<String> { Some(GONE.into()) } // Proto.gone; the actor adds (exit N | signal N), OOM wording and the stderr tail (Exit.say)
    fn install_hint(&self) -> Option<String> { Some(INSTALL.into()) }
}
pub fn driver(provider: &str) -> Option<Made> { if provider == "opencode" { Some(Made { cli: "opencode", driver: Box::new(OcDriver::new()) }) } else { None } }

// ---- Models (Ocm.*): what Settings and the picker show ----
fn cols(l: &str) -> Vec<&str> { l.split('\t').collect() }
fn mid(l: &str) -> &str { cols(l)[0] }
fn mrest(l: &str) -> String { cols(l)[1..].join(" ") }
/// Ocm.model: "provider/model", a name after a tab allowed.
pub fn is_model(l: &str) -> bool { !l.starts_with('!') && mid(l).contains('/') && !mid(l).contains(' ') }
/// Ocm.models
pub fn models(out: &str) -> Vec<String> { out.lines().filter(|l| is_model(l)).map(|l| mid(l).to_string()).collect() }
/// Ocm.labels: a label per model that has a name.
pub fn labels(out: &str) -> Value {
    let mut m = Map::new(); m.insert("default".into(), json!("opencode's configured default"));
    for l in out.lines().filter(|l| is_model(l) && !mrest(l).is_empty()) { m.insert(mid(l).into(), json!(mrest(l))); }
    Value::Object(m)
}
fn failed_line(out: &str) -> &str { out.lines().find(|l| l.starts_with("!exit ")).unwrap_or("") }
/// Ocm.why: why listing failed, from "!exit <code>\t<said>".
pub fn why(f: &str) -> String {
    let rest = &f[6.min(f.len())..]; let c = mid(rest); let said = mrest(rest).trim().to_string();
    if matches!(c, "124" | "137" | "143") { "timed out listing models".into() } else { format!("could not list models (`opencode models` exited {c}{})", if said.is_empty() { String::new() } else { format!(": {said}") }) }
}
/// Ocm.auth: the sign-ins opencode keeps, "OpenAI (OAuth)" each.
pub fn auth(out: &str) -> Vec<String> {
    out.lines().filter(|l| l.starts_with("!auth\t")).map(|l| { let r = &l[6..]; let rs = mrest(r); format!("{}{}", mid(r), if rs.is_empty() { String::new() } else { format!(" ({rs})") }) }).collect()
}
fn count(n: usize) -> String { format!("{n} {}", if n == 1 { "model" } else { "models" }) }
/// Ocm.state: the models and sign-ins, or why there are none.
pub fn state(out: &str) -> String {
    let f = failed_line(out); if !f.is_empty() { return why(f) }
    let n = models(out).len(); let xs = auth(out);
    if n == 0 { "lists no models: sign in with `opencode auth login`".into() } else { format!("{} · {}", count(n), if xs.is_empty() { "no sign-ins".to_string() } else { format!("sign-ins: {}", xs.join(", ")) }) }
}
/// Ocm.problem: the problem alone ("" when models were listed).
pub fn problem(out: &str) -> String { if models(out).is_empty() { state(out) } else { String::new() } }
/// Ocm.cli: the line Settings shows ("" when opencode is not installed).
pub fn cli(version: &str, out: &str) -> String { if version.is_empty() { String::new() } else { format!("{version} · {}", state(out)) } }
/// Ocm.about
pub fn about(version: &str, out: &str) -> Value { json!({"cli":cli(version, out),"version":version,"labels":labels(out),"error": if version.is_empty() { String::new() } else { problem(out) }}) }
/// Ocm.patch: the models info with opencode's part read again; every other key (and every other provider's about) kept.
pub fn patch(j: &Value, out: &str) -> Value {
    let mut m = Map::new(); if let Some(o) = j.as_object() { for (k, v) in o { if k != "opencode" && k != "about" { m.insert(k.clone(), v.clone()); } } }
    m.insert("opencode".into(), json!(models(out)));
    let mut ab = Map::new(); if let Some(o) = j.get("about").and_then(Value::as_object) { for (k, v) in o { if k != "opencode" { ab.insert(k.clone(), v.clone()); } } }
    let old = j.get("about").and_then(|a| a.get("opencode")).unwrap_or(&Value::Null);
    let ver = old.get("version").and_then(Value::as_str).or_else(|| old.get("cli").and_then(Value::as_str)).unwrap_or("");
    ab.insert("opencode".into(), about(ver, out)); m.insert("about".into(), Value::Object(ab)); Value::Object(m)
}
/// Port.tmo: `tmo SECS CMD...` (timeout(1), gtimeout, or a perl supervisor).
const TMO: &str = r#"tmo() { t=$1; shift; if command -v timeout >/dev/null 2>&1; then timeout -k 1 "$t" "$@"; elif command -v gtimeout >/dev/null 2>&1; then gtimeout -k 1 "$t" "$@"; else perl -e 'use POSIX qw(setpgid); my $t=shift; my $p=fork; defined($p) or exit 125; if (!$p) { setpgid(0,0)==0 or exit 125; exec {$ARGV[0]} @ARGV; exit 127; } setpgid($p,$p); my $stop=sub { my ($sig,$code)=@_; kill $sig,-$p; sleep 1; kill "KILL",-$p; waitpid($p,0); exit $code; }; $SIG{ALRM}=sub { $stop->("TERM",124); }; $SIG{HUP}=sub { $stop->("HUP",129); }; $SIG{INT}=sub { $stop->("INT",130); }; $SIG{QUIT}=sub { $stop->("QUIT",131); }; $SIG{TERM}=sub { $stop->("TERM",143); }; alarm $t; my $r; do { $r=waitpid($p,0); } while ($r<0 && $!{EINTR}); my $s=$?; alarm 0; exit($r<0 ? 125 : ($s & 127) ? 128+($s & 127) : $s >> 8);' "$t" "$@"; fi; }; "#;
/// Oc.models.sh: the listing script (run with `sh -c`): each model, "!exit <code>\t<last error>" on failure, "!auth\t<provider>\t<method>" per sign-in.
pub fn models_sh() -> String {
    format!("{TMO}{}", r#"command -v opencode >/dev/null || exit 0; f=$(mktemp); e=$(mktemp); tmo 30 opencode models > "$f" 2> "$e"; c=$?; grep -E '^[A-Za-z0-9][^ ]*/[^ ]*$' "$f"; [ "$c" -eq 0 ] || printf '!exit %s\t%s\n' "$c" "$(grep -v '^[[:space:]]*$' "$e" | tail -n 1 | sed 's/^[[:space:]]*//' | tr -d '\t' | cut -c1-200)"; tmo 15 opencode auth list 2>/dev/null | awk -F '  +' '{sub(/[[:space:]]+$/, "")} NF >= 3 && $NF == "stored" {printf "!auth\t%s\t%s\n", $1, $2}'; rm -f "$f" "$e"; exit 0"#)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn turn(sid: &str, model: &str, ask: bool) -> OcpTurn { OcpTurn { acp: AcpTurn { cwd: "/tmp/p".into(), mcp: "http://127.0.0.1:1/mcp/t?token=x".into(), prompt: "RULES".into(), text: "hello".into(), session: sid.into(), ask }, model: model.into() } }
    fn ln(a: &mut Acp, t: &OcpTurn, l: &str) -> Out { line(a, t, &serde_json::from_str(l).unwrap()) }
    fn emits(o: &Out) -> String { o.shapes.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" | ") }
    fn sends(o: &Out) -> String { o.send.join(" | ") }
    fn outs(o: &Out) -> String { o.send.iter().cloned().chain(o.shapes.iter().map(|v| v.to_string())).collect::<Vec<_>>().join(" | ") }
    fn method(l: &str) -> String { serde_json::from_str::<Value>(l).ok().map(|v| acp::s(&v, "method").to_string()).unwrap_or_default() }
    fn rid(l: &str) -> String { serde_json::from_str::<Value>(l).ok().and_then(|v| v.get("id").map(|i| i.to_string())).unwrap_or_default() }
    fn upd(u: &str) -> String { format!(r#"{{"jsonrpc":"2.0","method":"session/update","params":{{"sessionId":"S0","update":{u}}}}}"#) }
    const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"sessionCapabilities":{"resume":{}}}}}"#;
    const LOST: &str = "{\"type\":\"notice\",\"message\":\"opencode could not resume its session S0, so it started a new one without the earlier conversation. This thread's history in Backplane is kept.\"}";

    #[test] fn resume_without_replay_and_a_new_session() { // test/opencode_test
        let (old, new) = (turn("S0", "", false), turn("", "", false)); let mut a0 = Acp::new();
        assert_eq!(method(&sends(&ln(&mut a0, &old, INIT))), "session/resume"); assert!(sends(&ln(&mut a0, &old, INIT)).contains(r#""sessionId":"S0""#));
        assert_eq!(method(&sends(&ln(&mut a0, &new, INIT))), "session/new");
        let r2 = ln(&mut a0, &old, r#"{"jsonrpc":"2.0","id":2,"result":{"configOptions":[]}}"#);
        assert_eq!(method(&sends(&r2)), "session/prompt"); assert!(!outs(&r2).contains("<backplane_instructions>")); assert!(outs(&r2).contains(r#"{"type":"thread.started","thread_id":"S0"}"#)); assert!(!outs(&r2).contains(r#""type":"notice""#));
        let mut a = Acp::new(); let n2 = ln(&mut a, &new, r#"{"jsonrpc":"2.0","id":2,"result":{"sessionId":"S1"}}"#);
        assert!(outs(&n2).contains("<backplane_instructions>\\nRULES")); assert!(outs(&n2).contains(r#"{"type":"thread.started","thread_id":"S1"}"#));
    }
    #[test] fn a_lost_session_opens_afresh_and_is_told_once() {
        let old = turn("S0", "", false); let mut a = Acp::new();
        for e in [r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"session not found: S0"}}"#, r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"session S0 does not belong to cwd: /tmp/p"}}"#] {
            let mut a = Acp::new(); let l = ln(&mut a, &old, e); assert_eq!(method(&sends(&l)), "session/new"); assert_eq!(rid(&sends(&l)), "7"); assert!(!a.done); assert_eq!(emits(&l), "");
        }
        let n = ln(&mut a, &old, r#"{"jsonrpc":"2.0","id":7,"result":{"sessionId":"S2"}}"#);
        assert!(outs(&n).contains("<backplane_instructions>\\nRULES")); assert_eq!(emits(&n), format!("{{\"type\":\"thread.started\",\"thread_id\":\"S2\"}} | {LOST}"));
        let mut b = Acp::new(); assert_eq!(emits(&ln(&mut b, &old, r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32000,"message":"No models are available"}}"#)), r#"{"type":"turn.failed","message":"No models are available (sign in once with `opencode auth login`)"}"#);
        let mut c = Acp::new(); let boom = ln(&mut c, &old, r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32603,"message":"boom"}}"#); assert_eq!(outs(&boom), r#"{"type":"turn.failed","message":"boom"}"#); assert!(c.done);
        let mut d = Acp::new(); assert_eq!(method(&outs(&ln(&mut d, &turn("", "", false), r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"session not found: S0"}}"#))), "", "a fresh session is never renewed");
    }
    #[test] fn failures_say_how_to_fix_opencode() {
        let old = turn("S0", "", false); let mut a = Acp::new();
        assert_eq!(outs(&ln(&mut a, &old, r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32000,"message":"Authentication required"}}"#)), r#"{"type":"turn.failed","message":"Authentication required (sign in once with `opencode auth login`)"}"#);
        assert_eq!(outs(&ln(&mut a, &old, r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32602,"message":"model not found: x/y"}}"#)), r#"{"type":"turn.failed","message":"model not found: x/y (pick another opencode model in the composer)"}"#);
        assert!(hint("Insufficient account funds").contains("top it up")); assert!(hint("Provider authentication required").contains("opencode auth login` for that provider"));
    }
    #[test] fn tool_calls_are_logged_once_named_by_what_they_do() {
        let old = turn("S0", "", false); let mut a0 = Acp::new();
        let early = ln(&mut a0, &old, &upd(r#"{"sessionUpdate":"tool_call_update","toolCallId":"c9","status":"in_progress","title":"ls","rawInput":{"command":"ls"}}"#)); assert_eq!(outs(&early), "", "before open ignored");
        let mut a = Acp::new(); ln(&mut a, &old, r#"{"jsonrpc":"2.0","id":2,"result":{"configOptions":[]}}"#);
        let tc = ln(&mut a, &old, &upd(r#"{"sessionUpdate":"tool_call","toolCallId":"c1","name":"bash","title":"bash","kind":"execute","status":"pending","locations":[],"rawInput":{"cwd":"/tmp/p"}}"#)); assert_eq!(outs(&tc), "");
        let u1 = r#"{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"in_progress","kind":"execute","title":"ls -la","locations":[],"rawInput":{"command":"ls -la","cwd":"/tmp/p"}}"#;
        assert_eq!(outs(&ln(&mut a, &old, &upd(u1))), r#"{"type":"item.completed","item":{"type":"command_execution","id":"c1","command":"ls -la"}}"#);
        assert_eq!(outs(&ln(&mut a, &old, &upd(u1))), "");
        assert_eq!(outs(&ln(&mut a, &old, &upd(r#"{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"completed","locations":[],"content":[],"rawOutput":{}}"#))), r#"{"type":"tool.done","id":"c1"}"#);
        assert_eq!(outs(&ln(&mut a, &old, &upd(r#"{"sessionUpdate":"tool_call_update","toolCallId":"c2","status":"in_progress","kind":"read","title":"read","locations":[{"path":"/tmp/p/src/a"}],"rawInput":{"filePath":"/tmp/p/src/a"}}"#))), r#"{"type":"item.completed","item":{"type":"command_execution","id":"c2","command":"read src/a"}}"#);
        assert_eq!(outs(&ln(&mut a, &old, &upd(r#"{"sessionUpdate":"tool_call_update","toolCallId":"c3","status":"failed","kind":"search","title":"grep","locations":[],"rawInput":{"pattern":"Ocp"}}"#))), r#"{"type":"item.completed","item":{"type":"command_execution","id":"c3","command":"grep Ocp"}} | {"type":"tool.done","id":"c3"}"#);
        assert_eq!(outs(&ln(&mut a, &old, &upd(r#"{"sessionUpdate":"agent_message_chunk","messageId":"m1","content":{"type":"text","text":"Hi"}}"#))), r#"{"type":"item.delta","text":"Hi"}"#);
        let end = ln(&mut a, &old, r#"{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}"#); assert!(outs(&end).contains(r#"{"type":"turn.completed"}"#)); assert!(a.done);
    }
    #[test] fn a_permission_request_goes_to_the_person() {
        let mut a = Acp { sid: "S0".into(), ..Default::default() };
        let o = ln(&mut a, &turn("S0", "", true), r#"{"jsonrpc":"2.0","id":0,"method":"session/request_permission","params":{"sessionId":"S0","toolCall":{"toolCallId":"c4","title":"src/a","kind":"edit","rawInput":{"filePath":"/tmp/p/src/a"}},"options":[{"optionId":"once","kind":"allow_once","name":"Allow once"},{"optionId":"always","kind":"allow_always","name":"Always allow"},{"optionId":"reject","kind":"reject_once","name":"Reject"}]}}"#);
        let req: Value = serde_json::from_str(o.shapes[0]["request"].as_str().unwrap()).unwrap();
        assert_eq!(acp::s(&req, "method"), "session/request_permission"); assert_eq!(acp::detail(&req["params"]), "/tmp/p/src/a");
    }
    #[test] fn the_threads_model_goes_to_the_session_before_the_prompt() {
        let ds = "opencode/deepseek-v4.1-flash";
        let mut a = Acp::new(); let t = turn("S0", ds, false);
        let m1 = ln(&mut a, &t, r#"{"jsonrpc":"2.0","id":2,"result":{"configOptions":[]}}"#);
        assert_eq!(sends(&m1), format!(r#"{{"jsonrpc":"2.0","id":15,"method":"session/set_config_option","params":{{"sessionId":"S0","configId":"model","value":"{ds}"}}}}"#)); assert_eq!(emits(&m1), r#"{"type":"thread.started","thread_id":"S0"}"#);
        let m2 = ln(&mut a, &t, r#"{"jsonrpc":"2.0","id":15,"result":{"configOptions":[]}}"#); assert_eq!(method(&sends(&m2)), "session/prompt"); assert!(!sends(&m2).contains("<backplane_instructions>"));
        let mut f = Acp::new(); let tf = turn("", ds, false); let f1 = ln(&mut f, &tf, r#"{"jsonrpc":"2.0","id":2,"result":{"sessionId":"S5"}}"#); assert_eq!(rid(&sends(&f1)), "5");
        let f2 = ln(&mut f, &tf, r#"{"jsonrpc":"2.0","id":5,"result":{"configOptions":[]}}"#); assert_eq!(method(&sends(&f2)), "session/prompt"); assert!(sends(&f2).contains("<backplane_instructions>\\nRULES") && sends(&f2).contains(r#""sessionId":"S5""#));
        let mut x = a.clone(); let tx = turn("S0", "opencode/nope", false);
        let rx = ln(&mut x, &tx, r#"{"jsonrpc":"2.0","id":15,"error":{"code":-32602,"message":"Invalid params: model not found: opencode/nope"}}"#);
        assert_eq!(emits(&rx), r#"{"type":"turn.failed","message":"opencode refused the model opencode/nope: Invalid params: model not found: opencode/nope (pick another opencode model in the composer)"}"#); assert!(x.done);
        let mut d = Acp::new(); assert_eq!(method(&sends(&ln(&mut d, &turn("S0", "default", false), r#"{"jsonrpc":"2.0","id":2,"result":{"configOptions":[]}}"#))), "session/prompt");
        let mut r = Acp::new(); let rr = ln(&mut r, &turn("S0", ds, false), r#"{"jsonrpc":"2.0","id":7,"result":{"sessionId":"S6"}}"#); assert_eq!(rid(&sends(&rr)), "5"); assert!(emits(&rr).contains(r#""type":"notice""#));
    }
    #[test] fn rel_is_inside_the_threads_folder_only() { assert_eq!(rel("/tmp/p", "/tmp/p/x"), "x"); assert_eq!(rel("/tmp/p", "/tmp/pq/x"), "/tmp/pq/x"); }
    #[test] fn the_config_carries_the_model_and_permissions() {
        assert_eq!(config("m/x", "approval-required"), r#"{"model":"m/x","permission":{"edit":"ask","bash":"ask","webfetch":"ask"}}"#);
        assert_eq!(config("", "auto-accept-edits"), r#"{"permission":{"edit":"allow","bash":"ask","webfetch":"ask"}}"#);
        assert_eq!(config("default", "full-access"), r#"{"permission":{"edit":"allow","bash":"allow","webfetch":"allow"}}"#);
    }
    #[test] fn models_listing_and_the_cli_line() {
        let out = "opencode/a\topencode a\nopenai/b\n!auth\tOpenAI\tOAuth\n"; assert_eq!(models(out), ["opencode/a", "openai/b"]); assert_eq!(auth(out), ["OpenAI (OAuth)"]);
        assert_eq!(labels(out), json!({"default":"opencode's configured default","opencode/a":"opencode a"}));
        assert_eq!(cli("v", out), "v · 2 models · sign-ins: OpenAI (OAuth)"); assert_eq!(state("opencode/a\n"), "1 model · no sign-ins");
        assert_eq!(why("!exit 124\tx"), "timed out listing models");
        let j = json!({"claude":["opus"],"opencode":["old/m"],"about":{"grok":{"cli":"g"},"opencode":{"cli":"v1","version":"v1"}}});
        let p = patch(&j, out); assert_eq!(p["claude"], json!(["opus"])); assert_eq!(p["opencode"], json!(["opencode/a", "openai/b"])); assert_eq!(p["about"]["grok"], json!({"cli":"g"})); assert_eq!(p["about"]["opencode"]["cli"], "v1 · 2 models · sign-ins: OpenAI (OAuth)");
        assert!(models_sh().contains("opencode models") && models_sh().starts_with("tmo()"));
    }

    // ---- the laws: each a Rust test named after it ----
    #[test] fn oc_lost_session() { assert!(lost("session not found: ses_1")) }
    #[test] fn oc_other_folder_session() { assert!(lost("session ses_1 does not belong to cwd: /p")) }
    #[test] fn oc_tool_names_target() { assert_eq!(say_k("read", "src/a"), "read src/a") }
    #[test] fn oc_tool_command_once() { assert_eq!(say_k("ls -la src", "src"), "ls -la src") }
    #[test] fn oc_notice_kind() { // Codex.kind("notice") == 7: the shape this driver emits is read as a notice
        let fx = crate::agents::lines::Cx::new("t").codex_line(&acp::event("notice", json!({"message":"m"})), "opencode", "", 1); assert_eq!(fx.events.len(), 1) }
    #[test] fn oc_notice_info() { let mut c = crate::agents::lines::Cx::new("th");
        let fx = c.codex_line(&acp::event("notice", json!({"message":"m"})), "opencode", "", 5); let e = &fx.events[0];
        assert_eq!((e["$"].as_str(), e["tone"].as_str(), e["kind"].as_str(), e["summary"].as_str()), (Some("ActivityLogged"), Some("info"), Some("session"), Some("m"))); assert_eq!(fx.events.len(), 1) }
    #[test] fn oc_models_usage_none() { assert!(models("USAGE\n  opencode models [flags]\n").is_empty()) }
    #[test] fn oc_models_failed_says() { assert_eq!(cli("v", "!exit 1\tx\n"), "v · could not list models (`opencode models` exited 1: x)") }
    #[test] fn oc_models_empty_says() { assert_eq!(cli("v", ""), "v · lists no models: sign in with `opencode auth login`") }
    #[test] fn oc_models_missing_quiet() { assert_eq!(cli("", ""), "") }
}

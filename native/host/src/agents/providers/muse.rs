//! Muse Code (`muse serve`, MSP: msp.rs). One process per turn, resumed by session id, reported in Codex shapes.
//! Ported from server's Muse.* (command line, modes, model/list) and Hub.run.k (laws muse_run_turn, interrupt_muse_stops).
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
/// Muse refuses a command id it has seen, in any session: an empty gen (never from the actor, which always sets it) must not make ids repeat.
fn nonce(gen: &str) -> String { if gen.is_empty() { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos().to_string()).unwrap_or_default() } else { gen.to_string() } }
pub fn ask_shape(request_line: &str) -> Value { json!({"type":"bp.ask","request":request_line}) }

pub struct MuseDriver { turn: msp::MspTurn, st: msp::Msp, runtime: String }
impl MuseDriver { pub fn new() -> MuseDriver { MuseDriver { turn: msp::MspTurn::default(), st: msp::Msp::new(), runtime: String::new() } } pub fn done(&self) -> bool { self.st.done } }
impl Driver for MuseDriver {
    fn argv(&self, a: &TurnIn) -> Vec<String> { args(&a.runtime, &a.model) }
    fn start(&mut self, a: &TurnIn) -> Vec<String> {
        self.runtime = a.runtime.clone();
        self.turn = msp::MspTurn { cwd: a.cwd.clone(), mcp: a.mcp_url.clone(), prompt: a.prompt.clone(), text: a.text.clone(), session: a.session.clone(), mode: msp::mode(&a.runtime).into(), effort: effort(&a.effort).into(), nonce: nonce(&a.gen) };
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
    /// Proto.install
    fn install_hint(&self) -> Option<String> { Some("Install Muse Code, then sign in with `muse login`.".into()) }
    /// Proto.gone
    fn exit_why(&self, _code: i32) -> Option<String> { Some("muse exited before the turn ended (is it signed in? run `muse login`)".into()) }
}
pub fn driver(provider: &str) -> Option<Made> { if provider == "muse" { Some(Made { cli: "muse", driver: Box::new(MuseDriver::new()) }) } else { None } }

// ---- model/list (Muse.probe.sh, Muse.catalog, Models.muse, About.muse): what the Settings job reads ----
pub fn probe_sh() -> String {
    // Port.tmo: timeout, else gtimeout, else a perl one (a Mac without coreutils)
    const PERL: &str = r#"use POSIX qw(setpgid); my $t=shift; my $p=fork; defined($p) or exit 125; if (!$p) { setpgid(0,0)==0 or exit 125; exec {$ARGV[0]} @ARGV; exit 127; } setpgid($p,$p); my $stop=sub { my ($sig,$code)=@_; kill $sig,-$p; sleep 1; kill "KILL",-$p; waitpid($p,0); exit $code; }; $SIG{ALRM}=sub { $stop->("TERM",124); }; $SIG{HUP}=sub { $stop->("HUP",129); }; $SIG{INT}=sub { $stop->("INT",130); }; $SIG{QUIT}=sub { $stop->("QUIT",131); }; $SIG{TERM}=sub { $stop->("TERM",143); }; alarm $t; my $r; do { $r=waitpid($p,0); } while ($r<0 && $!{EINTR}); my $s=$?; alarm 0; exit($r<0 ? 125 : ($s & 127) ? 128+($s & 127) : $s >> 8);"#;
    format!("tmo() {{ t=$1; shift; if command -v timeout >/dev/null 2>&1; then timeout -k 1 \"$t\" \"$@\"; elif command -v gtimeout >/dev/null 2>&1; then gtimeout -k 1 \"$t\" \"$@\"; else perl -e '{PERL}' \"$t\" \"$@\"; fi; }}; command -v muse >/dev/null && printf '%s\\n' \
'{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"clientInfo\":{{\"name\":\"backplane\",\"version\":\"1\"}}}}}}' \
'{{\"jsonrpc\":\"2.0\",\"method\":\"initialized\",\"params\":{{}}}}' \
'{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"model/list\",\"params\":{{}}}}' | tmo 15 muse serve --no-session-log 2>/dev/null")
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
    #[test] fn command_ids_differ_per_turn_even_without_a_gen() {
        let id = |g: &str| { let mut d = MuseDriver::new(); let mut a = ti("full-access", ""); a.gen = g.into(); d.start(&a); msp::uuid(&d.turn.nonce, "3") };
        assert_ne!(id("1-1"), id("1-2")); let (x, y) = (id(""), { std::thread::sleep(std::time::Duration::from_millis(2)); id("") }); assert_ne!(x, y);
    }
    #[test] fn exit_words_name_the_code_and_hide_no_kill() {
        let d = MuseDriver::new(); let w = d.exit_why(1).unwrap(); use crate::agents::rules::exit_say;
        assert!(exit_say(&w, 137, "").starts_with("agent was killed (signal 9, likely out of memory) (signal 9)"), "an OOM-killed agent is not 'is it signed in?'");
        let x = exit_say(&w, 1, "boom"); assert!(x.starts_with("muse exited before the turn ended") && x.contains("(exit 1): boom"));
        assert!(d.install_hint().unwrap().contains("muse login"));
    }
    #[test] fn the_probe_has_a_perl_fallback() { let s = probe_sh(); assert!(s.contains("gtimeout") && s.contains("perl -e 'use POSIX qw(setpgid)") && s.contains("tmo 15 muse serve")); }
    #[test] fn effort_is_one_muse_takes() { assert_eq!(effort("max"), "max"); assert_eq!(effort("off"), ""); assert_eq!(effort(""), ""); }
    #[test] fn model_list_reads_request_two() {
        let out = "{\"id\":1,\"result\":{}}\n{\"id\":2,\"result\":{\"models\":[{\"modelId\":\"a\",\"displayLabel\":\"A\",\"isDefault\":true},{\"modelId\":\"b\"}]}}\n";
        let c = catalog(out); assert_eq!(models(&c), ["a", "b"]);
        assert_eq!(about(&c).to_string(), r#"{"default":"Muse's own default","a":"A · Muse's default","b":"b"}"#); assert_eq!(about(&[]).to_string(), "{}"); assert!(catalog("junk").is_empty());
    }

    #[test] fn muse_args_laws() { assert_eq!(driver("claude").is_none(), true); assert_eq!(super::super::driver("muse").unwrap().cli, "muse"); }
}

/// Test harness shared with agy.rs: run a driver against a stand-in process (never a real CLI), turn its shapes into log events through `Cx::codex_line`
/// as the actor does, answer asks, and project the events the driver decides (what test/agents/prov/js-*.jsonl records from the JS hub).
#[cfg(test)] pub(crate) mod e2e {
    use super::super::{Driver, TurnIn};
    use crate::agents::{lines::Cx, proc};
    use serde_json::{json, Value};
    use std::{path::{Path, PathBuf}, time::Duration};

    pub fn dir(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-prov-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    pub fn script(p: &Path, body: &str) { use std::os::unix::fs::PermissionsExt; std::fs::write(p, body).unwrap(); std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    /// The events a driver decides, in the JS hub's fields (ids and clocks left out).
    pub fn project(e: &Value) -> Option<String> {
        let t = e["$"].as_str().unwrap_or("");
        let m = match t {
            "SessionSet" => json!(["SessionSet", e["session"]]),
            "TodosChanged" => json!(["TodosChanged", e["who"], e["data"]]),
            "AskOpened" => json!(["AskOpened", e["kind"], e["tool"], e["detail"], e["options"]]),
            "ActivityLogged" if e["tone"] == "tool" || e["tone"] == "error" => json!(["ActivityLogged", e["tone"], e["kind"], e["summary"]]),
            "MessagePosted" if e["role"] == "assistant" => json!(["MessagePosted", e["text"]]),
            "TurnChanged" if e["state"] != "running" => json!(["TurnChanged", e["state"]]),
            "SettingSet" if e["key"].as_str().unwrap_or("").starts_with("tok.basis.") && e["value"] != "c:0" => json!(["tok.basis", e["value"]]),
            _ => return None,
        };
        Some(m.to_string())
    }
    pub fn js(name: &str) -> Vec<String> {
        let raw = match name { "muse-turn" => include_str!("../../../../../test/agents/prov/js-muse-turn.jsonl"), "muse-perm" => include_str!("../../../../../test/agents/prov/js-muse-perm.jsonl"),
            "agy-turn" => include_str!("../../../../../test/agents/prov/js-agy-turn.jsonl"), _ => include_str!("../../../../../test/agents/prov/js-agy-perm.jsonl") };
        raw.lines().filter_map(|l| project(&serde_json::from_str(l).unwrap())).collect()
    }
    /// One turn, as the actor runs it. Returns the projected events and every line written to the process.
    async fn run_once(prog: &Path, d: &mut dyn Driver, a: &TurnIn, provider: &str, answer: &str, extra_env: &[(String, String)]) -> (Vec<String>, Vec<String>, bool) {
        let mut env = d.env(a); env.extend(extra_env.iter().cloned());
        let mut p = match proc::spawn(prog, &d.argv(a), Path::new(&a.cwd), None, &env) { Ok(p) => p, Err(e) => return (vec![format!("SPAWN {e}")], vec![], e.raw_os_error() == Some(26)) };
        let mut wrote = vec![]; let mut out = vec![]; let mut cx = Cx::new(&a.thread); cx.running = true;
        for l in d.start(a) { wrote.push(l.clone()); p.input.send(l).await.unwrap(); }
        'turn: loop {
            let l = match tokio::time::timeout(Duration::from_secs(20), p.lines.recv()).await {
                Ok(Some(l)) => l,
                // stdout closed: the process ended before the turn did. Say how, with the tail of its stderr (a closed pipe is not a slow one).
                Ok(None) => { let e = tokio::time::timeout(Duration::from_secs(5), p.exit).await; let (txt, busy) = match e { Ok(Ok(x)) => (format!("EXITED code={:?} signal={:?} stderr={}", x.code, x.signal, x.stderr.trim()), x.stderr.contains("Text file busy")), _ => ("EXITED (no status)".to_string(), false) };
                    out.push(txt); return (out, wrote, busy) }
                Err(_) => { out.push("TIMEOUT".into()); break }
            };
            let line = serde_json::from_str::<Value>(&l).unwrap_or_else(|_| Value::String(l.clone()));
            let r = d.on_line(&line);
            for s in r.send { wrote.push(s.clone()); p.input.send(s).await.unwrap(); }
            for sh in r.shapes {
                if sh["type"] == "bp.ask" {
                    let req = sh["request"].clone(); let info = d.ask_info(&req).expect("an ask the driver describes");
                    out.push(json!(["AskOpened", info.kind, info.tool, info.detail, info.options]).to_string());
                    for s in d.answer(&req, answer) { wrote.push(s.clone()); p.input.send(s).await.unwrap(); }
                    continue;
                }
                let fx = cx.codex_line(&sh, provider, &a.model, 1);
                for e in fx.events { if let Some(x) = project(&e) { out.push(x) } if e["$"] == "TurnChanged" && e["state"] != "running" { break 'turn } }
            }
        }
        proc::terminate(p.pid, Duration::from_millis(200));
        (out, wrote, false)
    }
    /// A script written a moment ago can fail to exec with ETXTBSY when another test thread forks while its file is still open for writing (the child holds the
    /// descriptor until its own exec). That is the harness's race, not the driver's: run the whole turn again, with a fresh driver state (start() resets it).
    pub async fn run(prog: &Path, d: &mut dyn Driver, a: &TurnIn, provider: &str, answer: &str, extra_env: &[(String, String)]) -> (Vec<String>, Vec<String>) {
        for _ in 0..8 {
            let (out, wrote, busy) = run_once(prog, d, a, provider, answer, extra_env).await;
            if !busy { return (out, wrote) }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        (vec!["TEXT FILE BUSY after 8 tries".into()], vec![])
    }
}

#[cfg(test)]
mod standin_tests {
    use super::*;
    use super::e2e::*;
    fn ti(d: &std::path::Path, runtime: &str, text: &str, session: &str, nonce: &str) -> TurnIn {
        TurnIn { thread: "t1700000010-2".into(), cwd: d.to_string_lossy().into(), home: d.to_string_lossy().into(), runtime: runtime.into(), text: text.into(), session: session.into(),
            prompt: "RULES lls>".into(), mcp_url: "http://127.0.0.1:1/mcp/x".into(), gen: nonce.into(), ..Default::default() }
    }
    fn tail(v: Vec<String>) -> Vec<String> { v }
    // Stand-in Muse (test/agents/prov/standin-muse.py) through the driver: the events equal what the JS hub logged for the same exchange.
    #[tokio::test] async fn muse_turn_matches_the_js_hub() {
        let d = dir("muse-turn"); script(&d.join("muse"), include_str!("../../../../../test/agents/prov/standin-muse.py"));
        let mut dr = MuseDriver::new(); let a = ti(&d, "full-access", "zq hello", "", "1793864133-24");
        let (got, wrote) = run(&d.join("muse"), &mut dr, &a, "muse", "accept", &[("MUSE_LOG".into(), d.join("log").to_string_lossy().into())]).await;
        assert_eq!(tail(got), js("muse-turn"));
        assert!(wrote[0].contains("initialize") && wrote.iter().any(|w| w.contains("session/start")) && wrote.iter().any(|w| w.contains("turn/start")));
        assert!(std::fs::read_to_string(d.join("log")).unwrap().starts_with("ARGV serve --trust-workspace --disable-sandbox"));
    }
    #[tokio::test] async fn muse_approval_matches_the_js_hub_and_is_decided() {
        let d = dir("muse-perm"); script(&d.join("muse"), include_str!("../../../../../test/agents/prov/standin-muse.py"));
        let mut dr = MuseDriver::new(); let a = ti(&d, "approval-required", "zq perm please", "", "1793864143-25");
        let (got, wrote) = run(&d.join("muse"), &mut dr, &a, "muse", "accept", &[("MUSE_LOG".into(), d.join("log").to_string_lossy().into())]).await;
        assert_eq!(got, js("muse-perm"));
        assert!(wrote.iter().any(|w| w.contains("approval/decide") && w.contains("allow_once")));
        assert!(std::fs::read_to_string(d.join("log")).unwrap().contains("DECIDE \"allow_once\""), "the stand-in received the decision");
        assert!(wrote.iter().any(|w| w.contains("\"approvalMode\":\"promptUnmatched\"")));
    }
    #[tokio::test] async fn a_resumed_muse_session_sets_its_mode_first_and_repeats_no_command_id() {
        let d = dir("muse-resume"); script(&d.join("muse"), include_str!("../../../../../test/agents/prov/standin-muse.py"));
        let mut dr = MuseDriver::new(); let a = ti(&d, "auto", "zq hi", "OLDSESSION", "1793864150-26");
        let (got, wrote) = run(&d.join("muse"), &mut dr, &a, "muse", "accept", &[]).await;
        assert_eq!(got.last().unwrap(), r#"["TurnChanged","completed"]"#); assert!(got.contains(&r#"["SessionSet","OLDSESSION"]"#.to_string()));
        let m: Vec<&str> = wrote.iter().map(|w| w.as_str()).filter(|w| w.contains("session/resume") || w.contains("setApprovalMode") || w.contains("turn/start")).collect();
        assert!(m[0].contains("session/resume") && m[1].contains("setApprovalMode") && m[1].contains("onRequest") && m[2].contains("turn/start") && !m[2].contains("backplane_instructions"));
    }
}

/// The laws of src/core, one test each, named as the law: scripts/law-map.tsv maps them.
#[cfg(test)]
mod laws {
    use super::*;
    fn turn_in() -> TurnIn { TurnIn { thread: "t1".into(), cwd: "/work/p".into(), model: "m1".into(), session: "S9".into(), runtime: "approval-required".into(), text: "do it".into(), gen: "17-3".into(), mcp_url: "http://x/mcp".into(), ..Default::default() } }
    #[test] fn msp_mode_supervised_asks() { assert_eq!(msp::mode_k(0), "promptUnmatched"); assert_eq!(msp::mode("approval-required"), "promptUnmatched") }
    #[test] fn msp_mode_full_allows() { assert_eq!(msp::mode_k(3), "allowAll"); assert_eq!(msp::mode("full-access"), "allowAll") }
    /// Stop on a Muse thread is AgentStop alone: nothing is written to the process first (the actor ends it).
    #[test] fn interrupt_muse_stops() { let mut d = driver("muse").unwrap().driver; d.start(&turn_in()); assert!(d.on_stop().is_empty()) }
    /// A Muse thread's turn is one Muse process that carries the thread's cwd, model, session, runtime and text.
    #[test] fn muse_run_turn() {
        let made = driver("muse").unwrap(); assert_eq!(made.cli, "muse"); assert!(driver("antigravity").is_none());
        let mut d = made.driver; let a = turn_in();
        assert_eq!(d.argv(&a), ["serve", "--model", "m1", "--trust-workspace"]);
        let s = d.start(&a); assert!(s[0].contains("\"initialize\""));
        let r = d.on_line(&serde_json::from_str(r#"{"id":1,"result":{}}"#).unwrap());
        assert!(r.send[1].contains("\"session/resume\"") && r.send[1].contains("\"S9\""));
        let r = d.on_line(&serde_json::from_str(r#"{"id":2,"result":{"session":{"sessionId":"S9"}}}"#).unwrap());
        assert!(r.send[0].contains("setApprovalMode") && r.send[0].contains("promptUnmatched") && r.send[1].contains("\"text\":\"do it\""));
        let f = driver("muse").unwrap(); let _ = f; let mut full = TurnIn { runtime: "full-access".into(), session: String::new(), ..a };
        full.model.clear(); let mut d2 = driver("muse").unwrap().driver; assert_eq!(d2.argv(&full), ["serve", "--trust-workspace", "--disable-sandbox"]); let _ = d2.start(&full);
        let r = d2.on_line(&serde_json::from_str(r#"{"id":1,"result":{}}"#).unwrap()); assert!(r.send[1].contains("\"session/start\"") && r.send[1].contains("\"workspaceRoot\":\"/work/p\"") && r.send[1].contains("allowAll"));
    }
}

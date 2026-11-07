//! Agents' dialogs: asks a runner hands the hub that are neither ACP permission requests
//! nor Muse approvals: Pi's and Oh My Pi's extension dialogs, and ACP form elicitations. Each is one envelope line, method `bp/ask`,
//! with what the approval or question card shows and what the answer must be written back as.
//!
//! params: kind "approval" | "question"; proto "pi" (Pi's extension_ui_response) | "elicit" (an ACP elicitation's result); tool (an approval's
//! tool: what "Allow for session" remembers); detail (what the card says); options (a question's choices, one a line); key (an
//! elicitation's field); bool (the answer is yes or no); once, session, deny (an approval's labels). id: the agent's request id, as it came.
//!
//! Also here, shared by the Pi and Oh My Pi drivers: the JSON reading helpers that behave as `J.Json.get_*` (absent or mistyped -> the
//! default) and `jshow`, `J.Json.show` (compact, only `"`, `\` and control characters escaped, a number as written).
use serde_json::{json, Map, Value};

// ---- J.Json reading ----
/// J.Json.get_str: the string at `k`, else `d`.
pub fn gs<'a>(o: &'a Value, k: &str, d: &'a str) -> &'a str { o.get(k).and_then(Value::as_str).unwrap_or(d) }
/// J.Json.get_bool.
pub fn gb(o: &Value, k: &str, d: bool) -> bool { o.get(k).and_then(Value::as_bool).unwrap_or(d) }
/// J.Json.get_obj: the value at `k`, else null.
pub fn go<'a>(o: &'a Value, k: &str) -> &'a Value { o.get(k).unwrap_or(&Value::Null) }
/// J.Json.as_raw of `get`: a number's text as written, a string as it is, else `d`.
pub fn raw(o: &Value, k: &str, d: &str) -> String { match o.get(k) { Some(Value::Number(n)) => n.to_string(), Some(Value::String(s)) => s.clone(), _ => d.to_string() } }
/// J.Json.get_nat: `Nat.read` of the raw text (digits only), else `d`.
pub fn gn(o: &Value, k: &str, d: u64) -> u64 { let r = raw(o, k, ""); if !r.is_empty() && r.bytes().all(|c| c.is_ascii_digit()) { r.parse().unwrap_or(d) } else { d } }
/// J.Json.elems: an array's items, else none.
pub fn elems(o: &Value) -> &[Value] { o.as_array().map(Vec::as_slice).unwrap_or(&[]) }
/// Acp.or: `a` unless it is empty.
pub fn or<'a>(a: &'a str, b: &'a str) -> &'a str { if a.is_empty() { b } else { a } }
/// String.take, by characters.
pub fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }
/// String.drop, by characters.
pub fn drop(s: &str, n: usize) -> String { s.chars().skip(n).collect() }
/// A number as written (`Json.Num{raw}`); text that is no number becomes 0.
pub fn num(raw: &str) -> Value { serde_json::from_str::<Value>(raw).ok().filter(Value::is_number).unwrap_or_else(|| json!(0)) }

// ---- J.Json.show ----
fn quote(s: &str, o: &mut String) {
    o.push('"');
    for c in s.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"),
        c if (c as u32) < 32 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
    o.push('"');
}
fn show_into(v: &Value, o: &mut String) {
    match v {
        Value::Null => o.push_str("null"), Value::Bool(b) => o.push_str(if *b { "true" } else { "false" }), Value::Number(n) => o.push_str(&n.to_string()),
        Value::String(s) => quote(s, o),
        Value::Array(a) => { o.push('['); for (i, x) in a.iter().enumerate() { if i > 0 { o.push(',') } show_into(x, o) } o.push(']') }
        Value::Object(m) => { o.push('{'); for (i, (k, x)) in m.iter().enumerate() { if i > 0 { o.push(',') } quote(k, o); o.push(':'); show_into(x, o) } o.push('}') }
    }
}
/// J.Json.show.
pub fn jshow(v: &Value) -> String { let mut o = String::new(); show_into(v, &mut o); o }
/// An object from pairs, in order (J.Json.obj).
pub fn obj(kvs: Vec<(&str, Value)>) -> Value { let mut m = Map::new(); for (k, v) in kvs { m.insert(k.to_string(), v); } Value::Object(m) }
/// Acp.event: `{"type": ty, ...kvs}` as a line.
pub fn event(ty: &str, kvs: Vec<(&str, Value)>) -> Value { let mut v = vec![("type", json!(ty))]; v.extend(kvs); obj(v) }

// ---- Dlg ----
/// Dlg.line: the envelope for an ask.
pub fn line(id: &Value, kind: &str, proto: &str, kvs: Vec<(&str, Value)>) -> Value {
    let mut p = vec![("kind", json!(kind)), ("proto", json!(proto))]; p.extend(kvs);
    obj(vec![("jsonrpc", json!("2.0")), ("id", id.clone()), ("method", json!("bp/ask")), ("params", obj(p))])
}
/// Dlg.id: the request's id as it came (null when absent).
pub fn id_of(o: &Value) -> Value { o.get("id").cloned().unwrap_or(Value::Null) }
/// Dlg.approval.
pub fn is_approval(p: &Value) -> bool { gs(p, "kind", "") == "approval" }
/// Dlg.tool.
pub fn tool(p: &Value) -> &str { gs(p, "tool", "tool") }
/// Dlg.detail.
pub fn detail(p: &Value) -> &str { gs(p, "detail", "") }
/// Dlg.options.
pub fn options(p: &Value) -> &str { gs(p, "options", "") }
/// Dlg.allows: the person allowed it (once or for the session). Only an allow answers yes: a declined or dismissed approval never runs the tool.
pub fn allows(answer: &str) -> bool { answer == "accept" || answer == "session" }
/// Dlg.yes: a yes-or-no question's answer.
pub fn yes(answer: &str) -> bool { answer == "Yes" || answer == "yes" || allows(answer) }

fn pi_line(id: &Value, kv: (&str, Value)) -> String { jshow(&obj(vec![("type", json!("extension_ui_response")), ("id", id.clone()), kv])) }
/// Dlg.label: the label an approval's answer picks: allow for the session (else once), allow once, or deny.
pub fn label(p: &Value, answer: &str) -> String {
    if answer == "session" { or(gs(p, "session", ""), gs(p, "once", "")).to_string() }
    else if answer == "accept" { gs(p, "once", "").to_string() }
    else { gs(p, "deny", "Deny").to_string() }
}
fn pi_reply(approval: bool, p: &Value, id: &Value, answer: &str) -> String {
    if approval {
        // an approval Pi's extension asked as a confirm: yes or no; one Oh My Pi asked as a select (its choices in once, session, deny): the label the answer picks
        if gs(p, "once", "").is_empty() { pi_line(id, ("confirmed", json!(allows(answer)))) } else { pi_line(id, ("value", json!(label(p, answer)))) }
    } else if answer.is_empty() { pi_line(id, ("cancelled", json!(true))) } // dismissed cancels the dialog
    else if gb(p, "bool", false) { pi_line(id, ("confirmed", json!(yes(answer)))) }
    else { pi_line(id, ("value", json!(answer))) }
}
fn result(id: &Value, r: Value) -> String { jshow(&obj(vec![("jsonrpc", json!("2.0")), ("id", id.clone()), ("result", r)])) }
fn accept(key: &str, v: Value) -> Value { obj(vec![("action", json!("accept")), ("content", obj(vec![(key, v)]))]) }
fn elicit_reply(approval: bool, p: &Value, id: &Value, answer: &str) -> String {
    let key = gs(p, "key", "value");
    if approval { result(id, accept(key, json!(label(p, answer)))) }
    else if answer.is_empty() { result(id, obj(vec![("action", json!("cancel"))])) }
    else if gb(p, "bool", false) { result(id, accept(key, json!(yes(answer)))) }
    else { result(id, accept(key, json!(answer))) }
}
/// Dlg.reply: the agent's line for a parked envelope, once the person answered ("accept", "session", anything else declines; a question's
/// answer is its text, "" when dismissed).
pub fn reply(body: &Value, answer: &str) -> String {
    let p = go(body, "params"); let id = id_of(body);
    if gs(p, "proto", "") == "pi" { pi_reply(is_approval(p), p, &id, answer) } else { elicit_reply(is_approval(p), p, &id, answer) }
}

/// Bp.permit: what the hub does with an envelope. An approval already allowed for the session in this thread (`session_ok`, Perm.session_ok over
/// the open and closed asks) is allowed again at once and the agent's line returned; otherwise it opens an ask (AskOpened kind "approval", or
/// "input" with its choices for a question) and the envelope waits, parked, for the answer (`reply`).
#[derive(Debug, PartialEq)]
pub enum Permit { Reply(String), Open { kind: &'static str, tool: String, detail: String, options: String } }
pub fn permit(envelope: &Value, session_ok: bool) -> Permit {
    let p = go(envelope, "params");
    if is_approval(p) {
        if session_ok { Permit::Reply(reply(envelope, "accept")) }
        else { Permit::Open { kind: "approval", tool: tool(p).into(), detail: detail(p).into(), options: String::new() } }
    } else { Permit::Open { kind: "input", tool: tool(p).into(), detail: detail(p).into(), options: options(p).into() } }
}

/// Driver::ask_info: the card for an envelope (the Open case of `permit`); the actor answers a session-allowed approval itself.
pub fn ask_info(envelope: &Value) -> Option<super::AskInfo> {
    match permit(envelope, false) { Permit::Open { kind, tool, detail, options } => Some(super::AskInfo { kind: kind.into(), tool, detail, options }), Permit::Reply(_) => None }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pi_approval() -> Value { line(&json!("e1"), "approval", "pi", vec![("tool", json!("bash")), ("detail", json!("ls"))]) }
    fn omp_approval() -> Value { line(&json!(7), "approval", "pi", vec![("tool", json!("write")), ("detail", json!("a.txt")), ("once", json!("Approve")), ("session", json!("Approve write for this session")), ("deny", json!("Deny"))]) }

    // law dialog_decline_denies / dialog_dismissed_denies / dialog_session_allows
    #[test] fn dialog_decline_denies() { assert!(!allows("decline")) }
    #[test] fn dialog_dismissed_denies() { assert!(!allows("")) }
    #[test] fn dialog_session_allows() { assert!(allows("session") && allows("accept")) }

    #[test] fn the_envelope_is_what_dlg_line_writes() {
        assert_eq!(jshow(&pi_approval()), r#"{"jsonrpc":"2.0","id":"e1","method":"bp/ask","params":{"kind":"approval","proto":"pi","tool":"bash","detail":"ls"}}"#);
    }
    #[test] fn pi_confirm_answers_are_yes_or_no() {
        assert_eq!(reply(&pi_approval(), "accept"), r#"{"type":"extension_ui_response","id":"e1","confirmed":true}"#);
        assert_eq!(reply(&pi_approval(), "session"), r#"{"type":"extension_ui_response","id":"e1","confirmed":true}"#);
        assert_eq!(reply(&pi_approval(), "decline"), r#"{"type":"extension_ui_response","id":"e1","confirmed":false}"#);
        assert_eq!(reply(&pi_approval(), ""), r#"{"type":"extension_ui_response","id":"e1","confirmed":false}"#);
    }
    #[test] fn omp_select_answers_the_label_the_choice_picks() {
        assert_eq!(reply(&omp_approval(), "accept"), r#"{"type":"extension_ui_response","id":7,"value":"Approve"}"#);
        assert_eq!(reply(&omp_approval(), "session"), r#"{"type":"extension_ui_response","id":7,"value":"Approve write for this session"}"#);
        assert_eq!(reply(&omp_approval(), "decline"), r#"{"type":"extension_ui_response","id":7,"value":"Deny"}"#);
    }
    #[test] fn a_question_answers_its_text_a_yes_no_a_bool_and_a_dismissal_cancels() {
        let q = line(&json!("q"), "question", "pi", vec![("tool", json!("Pi")), ("detail", json!("Sure?")), ("options", json!("Yes\nNo")), ("bool", json!(true))]);
        assert_eq!(reply(&q, "Yes"), r#"{"type":"extension_ui_response","id":"q","confirmed":true}"#);
        assert_eq!(reply(&q, "No"), r#"{"type":"extension_ui_response","id":"q","confirmed":false}"#);
        assert_eq!(reply(&q, ""), r#"{"type":"extension_ui_response","id":"q","cancelled":true}"#);
        let s = line(&json!("s"), "question", "pi", vec![("tool", json!("Pi")), ("options", json!("a\nb"))]);
        assert_eq!(reply(&s, "b"), r#"{"type":"extension_ui_response","id":"s","value":"b"}"#);
    }
    #[test] fn an_acp_elicitation_is_accepted_with_the_field_or_cancelled() {
        let a = line(&json!(3), "approval", "elicit", vec![("key", json!("k")), ("once", json!("Once")), ("deny", json!("No"))]);
        assert_eq!(reply(&a, "accept"), r#"{"jsonrpc":"2.0","id":3,"result":{"action":"accept","content":{"k":"Once"}}}"#);
        assert_eq!(reply(&a, "x"), r#"{"jsonrpc":"2.0","id":3,"result":{"action":"accept","content":{"k":"No"}}}"#);
        let q = line(&json!(4), "question", "elicit", vec![("key", json!("f")), ("bool", json!(true))]);
        assert_eq!(reply(&q, ""), r#"{"jsonrpc":"2.0","id":4,"result":{"action":"cancel"}}"#);
        assert_eq!(reply(&q, "Yes"), r#"{"jsonrpc":"2.0","id":4,"result":{"action":"accept","content":{"f":true}}}"#);
        let t = line(&json!(5), "question", "elicit", vec![]);
        assert_eq!(reply(&t, "hi"), r#"{"jsonrpc":"2.0","id":5,"result":{"action":"accept","content":{"value":"hi"}}}"#);
    }
    #[test] fn an_approval_allowed_for_the_session_is_allowed_again_and_a_question_always_opens() {
        assert_eq!(permit(&pi_approval(), true), Permit::Reply(r#"{"type":"extension_ui_response","id":"e1","confirmed":true}"#.into()));
        assert_eq!(permit(&pi_approval(), false), Permit::Open { kind: "approval", tool: "bash".into(), detail: "ls".into(), options: String::new() });
        let q = line(&json!("q"), "question", "pi", vec![("tool", json!("Pi")), ("detail", json!("Pick")), ("options", json!("a\nb"))]);
        assert_eq!(permit(&q, true), Permit::Open { kind: "input", tool: "Pi".into(), detail: "Pick".into(), options: "a\nb".into() });
    }
    #[test] fn show_escapes_as_does() {
        assert_eq!(jshow(&json!({"a":"x\"y\\z\n\u{8}\u{1}é/"})), "{\"a\":\"x\\\"y\\\\z\\n\\u0008\\u0001é/\"}");
        assert_eq!(jshow(&serde_json::from_str::<Value>(r#"{"n":1.50,"b":[true,null]}"#).unwrap()), r#"{"n":1.50,"b":[true,null]}"#);
    }
}

/// End to end, stand-ins only: a Pi or Oh My Pi turn through the real actor against a real Hub on a temp home, its log compared event for event with the
/// one the JS hub wrote for the same stand-in (test/agents/prov-pi/fixtures/js-*.jsonl, recorded by record_pi.mjs from build/gpui/backplane-serve-current),
/// ids and clocks normalised. Settings and bookkeeping the JS hub also writes are not compared; the thread's messages, turn, activity, asks, session and usage are.
#[cfg(test)]
mod e2e {
    use crate::{agents::{proc::Cli, Agents}, api::Caller, hub::{Cfg, Hub}, presenter::Canned};
    use serde_json::{json, Value};
    use std::{path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}};
    const T1: &str = "t1700000001-2";
    const PI: &str = include_str!("../../../../../test/agents/prov-pi/standin-pi.sh");
    const OMP: &str = include_str!("../../../../../test/agents/prov-pi/standin-omp.sh");

    fn dir(name: &str) -> PathBuf { static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0); let p = std::env::temp_dir().join(format!("bp-pi-{}-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed), name)); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn script(d: &Path, name: &str, body: &str) { use crate::plat::fs::PermissionsExt; let p = d.join(name); std::fs::write(&p, body).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    /// Ids and clocks: a run of 3+ digits is N, and a hub's own counter after a dash (`m17N-150`, `c9-171`) is S.
    fn digits(s: &str) -> String {
        let b: Vec<char> = s.chars().collect(); let mut o = String::new(); let mut i = 0;
        while i < b.len() {
            if b[i] == '-' && i + 1 < b.len() && b[i + 1].is_ascii_digit() { o.push_str("-S"); i += 1; while i < b.len() && b[i].is_ascii_digit() { i += 1 } continue }
            if b[i].is_ascii_digit() { let mut j = i; while j < b.len() && b[j].is_ascii_digit() { j += 1 } if j - i >= 3 { o.push('N') } else { o.extend(&b[i..j]) } i = j; continue }
            o.push(b[i]); i += 1;
        }
        o
    }
    const KEEP: &[&str] = &["MessagePosted", "TurnChanged", "ActivityLogged", "AskOpened", "AskClosed", "SessionSet", "UsageSet", "TurnUsage", "PendingChanged"];
    fn core(ev: &[Value]) -> Vec<String> {
        ev.iter().filter(|e| KEEP.contains(&e["$"].as_str().unwrap_or("")) && e["thread"].as_str().map_or(true, |t| t == T1) && !(e["$"] == "MessagePosted" && e["role"] == "user" && false))
            .map(|e| { let mut e = e.clone(); if let Some(o) = e.as_object_mut() { o.remove("at"); } digits(&e.to_string()) }).collect()
    }
    /// One scene: provider, stand-in, runtime, message, the answer to give an ask (None: no ask), the JS hub's events.
    async fn scene(prov: &str, standin: &str, runtime: &str, msg: &str, answer: Option<&str>, js: &str) {
        let sd = dir(&format!("{prov}-{runtime}-bin")); script(&sd, prov, standin);
        for n in ["claude", "codex", "grok", "muse", "pi", "omp"] { if n != prov { script(&sd, n, "#!/bin/sh\nexit 1\n") } }
        let home = dir(&format!("{prov}-{runtime}-{}-home", msg.len())); let work = dir(&format!("{prov}-{runtime}-{}-work", msg.len()));
        let log = format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"fixture\",\"root\":{},\"at\":1700000000}}\n{{\"$\":\"ThreadCreated\",\"id\":\"{T1}\",\"project\":\"p1\",\"title\":\"Fixture 0\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"{prov}\",\"model\":\"\",\"at\":1700000001}}\n{{\"$\":\"ModesSet\",\"id\":\"{T1}\",\"runtime\":\"{runtime}\",\"interaction\":\"default\",\"model\":\"\",\"at\":1700000002}}\n",
            serde_json::to_string(&work.to_string_lossy()).unwrap());
        std::fs::write(home.join("events.jsonl"), log).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let ag = Agents::start_with(hub.clone(), Cli::standin(&sd)).await.unwrap();
        let from = hub.store.len();
        let rpc = |m: &'static str, p: Value| { let hub = hub.clone(); async move { let h = hub.rpc_for(m).expect("handler"); h.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(7), m, &p).await.remove(0) } };
        let events = || { let hub = hub.clone(); async move { hub.store.range(from, usize::MAX).unwrap().iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect::<Vec<_>>() } };
        assert_eq!(rpc("turn.start", json!({"thread":T1,"text":msg})).await["ok"], true);
        let wait = |f: fn(&[Value]) -> bool| { let events = events.clone(); async move { let end = Instant::now() + Duration::from_secs(30); loop { let ev = events().await; if f(&ev) || Instant::now() > end { return ev } tokio::time::sleep(Duration::from_millis(50)).await } } };
        if let Some(a) = answer {
            let ev = wait(|ev| ev.iter().any(|e| e["$"] == "AskOpened")).await;
            let id = ev.iter().find(|e| e["$"] == "AskOpened").unwrap_or_else(|| panic!("the ask opened; events so far: {:?}", ev))["id"].clone();
            assert_eq!(rpc("ask.answer", json!({"id":id,"answer":a})).await["ok"], true);
        }
        let ev = wait(|ev| ev.iter().any(|e| e["$"] == "TurnChanged" && (e["state"] == "completed" || e["state"] == "failed"))).await;
        tokio::time::sleep(Duration::from_millis(800)).await; let ev = if ev.is_empty() { ev } else { events().await }; ag.shutdown();
        let want: Vec<Value> = js.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        let (g, w) = (core(&ev), core(&want));
        eprintln!("rust events for {prov} {msg:?}:\n{}", g.join("\n"));
        for (i, (a, b)) in g.iter().zip(w.iter()).enumerate() { assert_eq!(a, b, "event {i} of {} (rust) / {} (js)", g.len(), w.len()) }
        assert_eq!(g.len(), w.len(), "counts; extra rust: {:?}; extra js: {:?}", g.iter().skip(w.len()).collect::<Vec<_>>(), w.iter().skip(g.len()).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(&home);
    }
    macro_rules! fx { ($n:literal) => { include_str!(concat!("../../../../../test/agents/prov-pi/fixtures/", $n)) } }
    #[tokio::test(flavor = "multi_thread")] async fn pi_turn_matches_the_js_hub() { scene("pi", PI, "full-access", "hello pi", None, fx!("js-pi-turn.jsonl")).await }
    #[tokio::test(flavor = "multi_thread")] async fn pi_failure_matches_the_js_hub() { scene("pi", PI, "full-access", "zqfail please", None, fx!("js-pi-fail.jsonl")).await }
    #[tokio::test(flavor = "multi_thread")] async fn pi_allowed_tool_matches_the_js_hub() { scene("pi", PI, "approval-required", "zqask go", Some("accept"), fx!("js-pi-askaccept.jsonl")).await }
    #[tokio::test(flavor = "multi_thread")] async fn pi_declined_tool_matches_the_js_hub() { scene("pi", PI, "approval-required", "zqask go", Some("decline"), fx!("js-pi-askdeny.jsonl")).await }
    #[tokio::test(flavor = "multi_thread")] async fn pi_other_extensions_question_matches_the_js_hub() { scene("pi", PI, "full-access", "zqquest go", Some("dev"), fx!("js-pi-quest.jsonl")).await }
    #[tokio::test(flavor = "multi_thread")] async fn omp_turn_matches_the_js_hub() { scene("omp", OMP, "full-access", "hello omp", None, fx!("js-omp-turn.jsonl")).await }
    #[tokio::test(flavor = "multi_thread")] async fn omp_failure_matches_the_js_hub() { scene("omp", OMP, "full-access", "zqfail", None, fx!("js-omp-fail.jsonl")).await }
    #[tokio::test(flavor = "multi_thread")] async fn omp_allowed_tool_matches_the_js_hub() { scene("omp", OMP, "approval-required", "zqask go", Some("accept"), fx!("js-omp-askaccept.jsonl")).await }
    #[tokio::test(flavor = "multi_thread")] async fn omp_declined_tool_matches_the_js_hub() { scene("omp", OMP, "approval-required", "zqask go", Some("decline"), fx!("js-omp-askdeny.jsonl")).await }
}

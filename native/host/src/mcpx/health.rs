//! The live check a card's Test runs (and `mcp_test`): start the server as an agent would, `initialize`, then list its tools, resources and
//! prompts. A stdio server runs here (the hub's machine, where its agents run); a remote one is reached with curl (streamable HTTP: POSTs with
//! the session id it gives; SSE: the event stream and the endpoint it names). What it said on stderr is kept, its last lines, for the card.
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// A server ready to start: env and headers with the vault's values put in.
#[derive(Clone, Debug, Default)]
pub struct Resolved { pub transport: String, pub command: String, pub args: Vec<String>, pub env: Vec<(String, String)>, pub cwd: String, pub url: String, pub headers: Vec<(String, String)> }

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tool { pub name: String, #[serde(default)] pub description: String }
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Report {
    pub ok: bool, pub at: u64, pub ms: u64,
    #[serde(default)] pub server: String, #[serde(default)] pub version: String,
    #[serde(default)] pub tools: Vec<Tool>, #[serde(default)] pub resources: usize, #[serde(default)] pub prompts: usize,
    #[serde(default)] pub error: String, #[serde(default)] pub stderr: String,
}

pub const TIMEOUT: Duration = Duration::from_secs(90); // a first `npx -y` or `uvx` downloads the package
const PROTO: &str = "2025-06-18";
fn init() -> Value { json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":PROTO,"capabilities":{},"clientInfo":{"name":"backplane-check","version":"1"}}}) }
fn req(id: u64, m: &str) -> Value { json!({"jsonrpc":"2.0","id":id,"method":m,"params":{}}) }

/// Fills a report from the answers to initialize (1), tools/list (2), resources/list (3), prompts/list (4).
pub fn digest(r: &mut Report, answers: &[Value]) {
    let by = |id: u64| answers.iter().find(|a| a["id"] == id);
    if let Some(a) = by(1) {
        if let Some(e) = a.get("error") { r.error = format!("initialize failed: {}", e["message"].as_str().unwrap_or("")); return }
        r.server = a.pointer("/result/serverInfo/name").and_then(Value::as_str).unwrap_or("").into(); r.version = a.pointer("/result/serverInfo/version").and_then(Value::as_str).unwrap_or("").into();
    } else { if r.error.is_empty() { r.error = "it did not answer initialize".into() } return }
    match by(2) { Some(a) if a.get("error").is_none() => r.tools = a.pointer("/result/tools").and_then(Value::as_array).into_iter().flatten().map(|t| Tool { name: t["name"].as_str().unwrap_or("").into(), description: t["description"].as_str().unwrap_or("").lines().next().unwrap_or("").chars().take(200).collect() }).collect(),
        Some(a) => { r.error = format!("tools/list failed: {}", a["error"]["message"].as_str().unwrap_or("")); return } None => {} }
    r.resources = by(3).and_then(|a| a.pointer("/result/resources")).and_then(Value::as_array).map_or(0, Vec::len);
    r.prompts = by(4).and_then(|a| a.pointer("/result/prompts")).and_then(Value::as_array).map_or(0, Vec::len);
    r.ok = true; r.error.clear();
}

pub async fn check(s: &Resolved, now: u64) -> Report {
    let t0 = Instant::now();
    let mut r = match s.transport.as_str() { "stdio" => stdio(s).await, "sse" => sse(s).await, _ => http(s).await };
    r.at = now; r.ms = t0.elapsed().as_millis() as u64; r
}

async fn stdio(s: &Resolved) -> Report {
    let mut r = Report::default();
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Shell, &s.command);
    c.args(&s.args).envs(s.env.iter().map(|(k, v)| (k.as_str(), v.as_str()))).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    if !s.cwd.is_empty() { c.current_dir(&s.cwd); }
    let mut ch = match c.spawn() { Ok(ch) => ch, Err(e) => { r.error = format!("could not start {}: {e}", s.command); return r } };
    let (mut cin, cout, mut cerr) = (ch.stdin.take().unwrap(), ch.stdout.take().unwrap(), ch.stderr.take().unwrap());
    let errt = tokio::spawn(async move { let mut b = Vec::new(); let _ = (&mut cerr).take(1 << 20).read_to_end(&mut b).await; String::from_utf8_lossy(&b).into_owned() });
    let mut answers = vec![];
    let run = async {
        let mut lines = BufReader::new(cout).lines();
        let send = |v: Value| format!("{v}\n");
        cin.write_all(send(init()).as_bytes()).await.ok()?;
        let mut sent_rest = false;
        while let Ok(Some(l)) = lines.next_line().await {
            let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
            if v.get("method").is_some() { // a request of its own (roots/list, sampling): answered "not supported"
                if !v["id"].is_null() { let _ = cin.write_all(send(json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32601,"message":"not supported"}})).as_bytes()).await; } continue }
            answers.push(v.clone());
            if v["id"] == 1 && !sent_rest {
                sent_rest = true;
                if v.get("error").is_some() { break }
                let mut b = send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
                let caps = v.pointer("/result/capabilities").cloned().unwrap_or(json!({}));
                b.push_str(&send(req(2, "tools/list")));
                if caps.get("resources").is_some() { b.push_str(&send(req(3, "resources/list"))) }
                if caps.get("prompts").is_some() { b.push_str(&send(req(4, "prompts/list"))) }
                cin.write_all(b.as_bytes()).await.ok()?;
                let want: Vec<u64> = [Some(2), caps.get("resources").map(|_| 3), caps.get("prompts").map(|_| 4)].into_iter().flatten().collect();
                answers.retain(|a| a["id"] == 1); // what came before is kept, then the rest is waited for
                let mut have = vec![];
                while have.len() < want.len() { match lines.next_line().await { Ok(Some(l)) => { if let Ok(v) = serde_json::from_str::<Value>(&l) { if let Some(i) = v["id"].as_u64().filter(|i| want.contains(i)) { have.push(i); answers.push(v) } else if v.get("method").is_some() && !v["id"].is_null() { let _ = cin.write_all(send(json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32601,"message":"not supported"}})).as_bytes()).await; } } } _ => break } }
                break
            }
        }
        Some(())
    };
    let timed_out = tokio::time::timeout(TIMEOUT, run).await.is_err();
    let _ = ch.start_kill(); let status = tokio::time::timeout(Duration::from_secs(3), ch.wait()).await.ok().and_then(|s| s.ok());
    r.stderr = tail(&tokio::time::timeout(Duration::from_secs(2), errt).await.ok().and_then(|x| x.ok()).unwrap_or_default());
    digest(&mut r, &answers);
    if timed_out && !r.ok { r.error = format!("no answer in {} s", TIMEOUT.as_secs()) }
    else if !r.ok && answers.is_empty() { r.error = match status.and_then(|s| s.code()) { Some(c) if c != 0 => format!("it exited with code {c} before it answered"), _ => "it closed its output without answering".into() } }
    r
}
/// The last lines of a stderr, 2000 characters at most.
pub fn tail(s: &str) -> String { let ls: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect(); let t = ls[ls.len().saturating_sub(12)..].join("\n"); let n = t.chars().count(); if n > 2000 { t.chars().skip(n - 2000).collect() } else { t } }

/// A curl config file (0600, removed after) with the headers, so no header value is on a command line.
fn curl_cfg(s: &Resolved, extra: &[(String, String)]) -> std::io::Result<tempfile_lite::Tmp> {
    let mut body = String::new();
    for (k, v) in s.headers.iter().chain(extra.iter()) { body.push_str(&format!("header = \"{}: {}\"\n", esc(k), esc(v))) }
    tempfile_lite::Tmp::new(&body)
}
fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('"', "\\\"").replace(['\n', '\r'], " ") }
mod tempfile_lite {
    pub struct Tmp(pub std::path::PathBuf);
    impl Tmp { pub fn new(body: &str) -> std::io::Result<Tmp> {
        let mut b = [0u8; 8]; crate::plat::fs::random(&mut b)?; let d = std::env::temp_dir().join(format!("bp-mcpcheck-{}", hex::encode(b)));
        std::fs::create_dir_all(&d)?; #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700))?; }
        let p = d.join("h"); std::fs::write(&p, body)?; Ok(Tmp(p)) } }
    impl Drop for Tmp { fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); if let Some(d) = self.0.parent() { let _ = std::fs::remove_dir(d); } } }
}
/// The JSON-RPC messages in an HTTP reply body: one JSON value, or SSE `data:` lines.
pub fn messages(body: &str) -> Vec<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(body.trim()) { return match v { Value::Array(a) => a, v => vec![v] } }
    let mut out = vec![]; let mut data = String::new();
    for l in body.lines().chain(std::iter::once("")) { let l = l.trim_end_matches('\r');
        if let Some(d) = l.strip_prefix("data:") { if !data.is_empty() { data.push('\n') } data.push_str(d.trim_start()) }
        else if l.is_empty() && !data.is_empty() { if let Ok(v) = serde_json::from_str::<Value>(&data) { out.push(v) } data.clear() } }
    out
}
/// One POST: (status, the session id it gave, the messages).
async fn post(s: &Resolved, session: &str, body: &Value) -> Result<(u16, String, Vec<Value>), String> {
    let mut extra = vec![("Content-Type".to_string(), "application/json".to_string()), ("Accept".to_string(), "application/json, text/event-stream".to_string()), ("MCP-Protocol-Version".to_string(), PROTO.to_string())];
    if !session.is_empty() { extra.push(("Mcp-Session-Id".into(), session.into())) }
    let cfg = curl_cfg(s, &extra).map_err(|e| e.to_string())?;
    let o = crate::proc::tokio_cmd(crate::proc::Kind::Net, "curl").args(["-sS", "--max-time", "30", "--proto", "=https,http", "-D", "-", "-X", "POST", "--data-binary", "@-", "-K"]).arg(&cfg.0).arg("--url").arg(&s.url)
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true).spawn().map_err(|e| format!("could not run curl: {e}"))?;
    let mut o = o; if let Some(mut i) = o.stdin.take() { let _ = i.write_all(body.to_string().as_bytes()).await; }
    let out = o.wait_with_output().await.map_err(|e| e.to_string())?;
    if !out.status.success() { return Err(String::from_utf8_lossy(&out.stderr).trim().trim_start_matches("curl: ").to_string()) }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    // the last header block (a 100 Continue or a redirect comes first)
    let mut rest = text.as_str(); let mut head = "";
    while rest.starts_with("HTTP/") { match rest.split_once("\r\n\r\n") { Some((h, r)) => { head = h; rest = r } None => { head = rest; rest = ""; break } } }
    let code = head.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    let sid = head.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("mcp-session-id")).map(|(_, v)| v.trim().to_string())).unwrap_or_default();
    Ok((code, sid, messages(rest)))
}
fn http_err(code: u16) -> String { match code { 401 | 403 => format!("{code}: it wants a key or a sign-in (add an Authorization header from the vault; sign-in flows are not supported)"), 404 => "404: no MCP server at that URL".into(), 405 => "405: that URL does not take MCP POSTs (try SSE)".into(), c => format!("it answered HTTP {c}") } }

async fn http(s: &Resolved) -> Report {
    let mut r = Report::default(); let mut answers = vec![];
    let go = async {
        let (code, sid, m) = post(s, "", &init()).await?; if !(200..300).contains(&code) { return Err(http_err(code)) }
        answers.extend(m);
        let caps = answers.iter().find(|a| a["id"] == 1).and_then(|a| a.pointer("/result/capabilities")).cloned().unwrap_or(json!({}));
        if answers.iter().any(|a| a["id"] == 1 && a.get("error").is_some()) { return Ok(()) }
        let _ = post(s, &sid, &json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
        for (id, m, need) in [(2, "tools/list", true), (3, "resources/list", caps.get("resources").is_some()), (4, "prompts/list", caps.get("prompts").is_some())] {
            if !need { continue } let (c, _, mm) = post(s, &sid, &req(id, m)).await?; if (200..300).contains(&c) { answers.extend(mm) } else if id == 2 { return Err(http_err(c)) } }
        Ok::<(), String>(())
    };
    match tokio::time::timeout(TIMEOUT, go).await { Ok(Err(e)) => r.error = e, Err(_) => r.error = format!("no answer in {} s", TIMEOUT.as_secs()), Ok(Ok(())) => {} }
    let e = r.error.clone(); digest(&mut r, &answers); if !e.is_empty() && !r.ok { r.error = e } r
}

/// The older SSE transport: GET the stream, wait for its `endpoint` event, POST there; the answers come on the stream.
async fn sse(s: &Resolved) -> Report {
    let mut r = Report::default(); let mut answers = vec![];
    let cfg = match curl_cfg(s, &[("Accept".into(), "text/event-stream".into())]) { Ok(c) => c, Err(e) => { r.error = e.to_string(); return r } };
    let mut ch = match crate::proc::tokio_cmd(crate::proc::Kind::Net, "curl").args(["-sSN", "--max-time", "120", "--proto", "=https,http", "-K"]).arg(&cfg.0).arg("--url").arg(&s.url)
        .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true).spawn() { Ok(c) => c, Err(e) => { r.error = format!("could not run curl: {e}"); return r } };
    let mut lines = BufReader::new(ch.stdout.take().unwrap()).lines();
    let go = async {
        let (mut ev, mut data) = (String::new(), String::new());
        let mut endpoint = String::new(); let mut want: Vec<u64> = vec![1]; let mut sent_rest = false;
        while let Ok(Some(l)) = lines.next_line().await {
            let l = l.trim_end_matches('\r').to_string();
            if let Some(e) = l.strip_prefix("event:") { ev = e.trim().into(); continue }
            if let Some(d) = l.strip_prefix("data:") { data.push_str(d.trim_start()); continue }
            if !l.is_empty() { continue }
            let (e, d) = (std::mem::take(&mut ev), std::mem::take(&mut data));
            if e == "endpoint" {
                endpoint = if d.starts_with("http") { d } else { let base: String = s.url.splitn(4, '/').take(3).collect::<Vec<_>>().join("/"); format!("{base}{}", if d.starts_with('/') { d } else { format!("/{d}") }) };
                let s2 = Resolved { url: endpoint.clone(), ..s.clone() }; post(&s2, "", &init()).await?;
            } else if let Ok(v) = serde_json::from_str::<Value>(&d) {
                let id = v["id"].as_u64().unwrap_or(0); if !want.contains(&id) { continue } answers.push(v.clone()); want.retain(|x| *x != id);
                if id == 1 && !sent_rest { sent_rest = true; if v.get("error").is_some() { break }
                    let s2 = Resolved { url: endpoint.clone(), ..s.clone() }; let _ = post(&s2, "", &json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await;
                    let caps = v.pointer("/result/capabilities").cloned().unwrap_or(json!({}));
                    for (i, m, need) in [(2, "tools/list", true), (3, "resources/list", caps.get("resources").is_some()), (4, "prompts/list", caps.get("prompts").is_some())] { if need { want.push(i); post(&s2, "", &req(i, m)).await?; } } }
                if want.is_empty() { break }
            }
        }
        if endpoint.is_empty() { return Err("the stream named no endpoint (is it an SSE server?)".to_string()) }
        Ok(())
    };
    match tokio::time::timeout(TIMEOUT, go).await { Ok(Err(e)) => r.error = e, Err(_) => r.error = format!("no answer in {} s", TIMEOUT.as_secs()), Ok(Ok(())) => {} }
    let _ = ch.start_kill();
    let e = r.error.clone(); digest(&mut r, &answers); if !e.is_empty() && !r.ok { r.error = e } r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn answers_make_a_report() {
        let mut r = Report::default();
        digest(&mut r, &[json!({"id":1,"result":{"serverInfo":{"name":"everything","version":"2.0"},"capabilities":{"tools":{}}}}), json!({"id":2,"result":{"tools":[{"name":"echo","description":"Echoes back\nmore"},{"name":"add"}]}}), json!({"id":3,"result":{"resources":[{},{}]}})]);
        assert!(r.ok); assert_eq!((r.server.as_str(), r.tools.len(), r.resources, r.prompts), ("everything", 2, 2, 0)); assert_eq!(r.tools[0].description, "Echoes back");
        let mut r = Report::default(); digest(&mut r, &[json!({"id":1,"error":{"message":"bad version"}})]); assert!(!r.ok && r.error.contains("bad version"));
        let mut r = Report::default(); digest(&mut r, &[]); assert_eq!(r.error, "it did not answer initialize");
    }
    #[test] fn sse_and_json_bodies_both_read() {
        assert_eq!(messages("{\"id\":1}"), vec![json!({"id":1})]);
        assert_eq!(messages("event: message\r\ndata: {\"id\":2,\r\ndata: \"x\":1}\r\n\r\nevent: message\ndata: {\"id\":3}\n"), vec![json!({"id":2,"x":1}), json!({"id":3})]);
        assert!(messages("").is_empty());
        assert_eq!(tail(&(0..30).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n")).lines().count(), 12);
    }
    /// A real stdio server, written here as a shell script: initialize, then tools/list with two tools.
    #[cfg(unix)] #[tokio::test] async fn a_stdio_server_is_started_and_listed() {
        let d = crate::plat::tmp::dir("mcpx-h"); let f = d.join("srv.sh");
        std::fs::write(&f, "#!/bin/sh\necho starting >&2\nwhile read -r l; do case \"$l\" in\n*'\"initialize\"'*) echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{\"tools\":{},\"prompts\":{}},\"serverInfo\":{\"name\":\"sh-srv\",\"version\":\"0.1\"}}}';;\n*tools/list*) echo '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"tools\":[{\"name\":\"a\"},{\"name\":\"b\"}]}}';;\n*prompts/list*) echo '{\"jsonrpc\":\"2.0\",\"id\":4,\"result\":{\"prompts\":[{\"name\":\"p\"}]}}';;\nesac; done\n").unwrap();
        let r = check(&Resolved { transport: "stdio".into(), command: "sh".into(), args: vec![f.to_string_lossy().into()], env: vec![("X".into(), "1".into())], ..Default::default() }, 5).await;
        assert!(r.ok, "{r:?}"); assert_eq!((r.server.as_str(), r.tools.len(), r.prompts), ("sh-srv", 2, 1)); assert_eq!(r.stderr, "starting"); assert_eq!(r.at, 5);
        let bad = check(&Resolved { transport: "stdio".into(), command: "sh".into(), args: vec!["-c".into(), "echo boom >&2; exit 3".into()], ..Default::default() }, 0).await;
        assert!(!bad.ok); assert!(bad.error.contains("code 3"), "{}", bad.error); assert_eq!(bad.stderr, "boom");
        let none = check(&Resolved { transport: "stdio".into(), command: "/nonexistent/x".into(), ..Default::default() }, 0).await; assert!(none.error.starts_with("could not start"));
    }
}

//! `backplane-hub --mcp-run <id>`: how an agent starts a server from the list (gen.rs). It asks the hub (`BACKPLANE_MCP_URL`, the thread's
//! own endpoint for this agent process, agents/mcp.rs) for the server's resolved spec (stdio command/env or remote URL/headers with the
//! vault's values put in) and passes JSON-RPC lines both ways. Tools turned off are taken out of `tools/list` and a call to one is
//! answered with an error without reaching the server. Each tool call is reported to the hub (the card's last use), and a failure is
//! reported with the error and stderr.
//!
//! `backplane-hub --mcp-headers <id> <url>`: Claude's `headersHelper` for a remote server with secret headers: prints them as one JSON object.
use crate::agents::mcp_stdio::{endpoint, post};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// A private method on the thread's endpoint: its result, or the error's message.
pub fn ask(url: &str, method: &str, params: Value) -> Result<Value, String> {
    let (hp, path) = endpoint(url).ok_or("BACKPLANE_MCP_URL is not this hub's loopback address")?;
    let (code, body) = post(&hp, &path, &json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string()).map_err(|e| format!("Backplane is not reachable: {e}"))?;
    if code != 200 { return Err(format!("Backplane answered {code}")) }
    let v: Value = serde_json::from_str(body.trim()).map_err(|_| "Backplane's answer is not JSON".to_string())?;
    if let Some(e) = v.get("error") { return Err(e["message"].as_str().unwrap_or("refused").to_string()) }
    Ok(v["result"].clone())
}
fn tell(url: &str, method: &str, params: Value) {
    if url.is_empty() { return }
    let (u, m) = (url.to_string(), method.to_string());
    std::thread::spawn(move || { let _ = ask(&u, &m, params); });
}

/// A `tools/list` result with the turned-off tools taken out.
pub fn filter_list(mut msg: Value, off: &[String]) -> Value {
    if let Some(a) = msg.pointer_mut("/result/tools").and_then(Value::as_array_mut) { a.retain(|t| !off.iter().any(|o| t["name"].as_str() == Some(o))) }
    msg
}
/// The answer to a call of a turned-off tool, None for any other line.
pub fn refuse(line: &Value, off: &[String]) -> Option<Value> {
    if line["method"] != "tools/call" { return None }
    let name = line.pointer("/params/name").and_then(Value::as_str)?;
    off.iter().any(|o| o == name).then(|| json!({"jsonrpc":"2.0","id":line["id"],"error":{"code":-32601,"message":format!("{name} is turned off for this server in Backplane's Settings > MCP")}}))
}
fn key(id: &Value) -> String { id.to_string() }
fn write_out<W: Write>(o: &Arc<Mutex<W>>, v: &str) { let mut o = o.lock().unwrap(); let _ = writeln!(o, "{v}"); let _ = o.flush(); }
fn rpc_err_val(v: &Value, code: i64, msg: &str) -> Option<String> {
    let id = v.get("id").filter(|i| !i.is_null())?;
    Some(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}}).to_string())
}
fn headers_from_spec(spec: &Value) -> Vec<(String, String)> {
    spec["headers"].as_array().into_iter().flatten()
        .filter_map(|kv| Some((kv[0].as_str()?.to_string(), kv[1].as_str()?.to_string())))
        .collect()
}
fn report_tool_call(hub_url: &str, id: &str, tool: &str, msg: &Value) {
    let ok = msg.get("error").is_none() && msg.pointer("/result/isError") != Some(&Value::Bool(true));
    let err: String = if ok { String::new() } else { msg.pointer("/error/message").or_else(|| msg.pointer("/result/content/0/text")).and_then(Value::as_str).unwrap_or("").chars().take(300).collect() };
    tell(hub_url, "backplane/mcp_used", json!({"id": id, "tool": tool, "ok": ok, "error": err}));
}

fn esc(s: &str) -> String { s.replace('\\', "\\\\").replace('"', "\\\"").replace(['\n', '\r'], " ") }
struct Tmp(std::path::PathBuf);
impl Tmp {
    fn new(body: &str) -> std::io::Result<Tmp> {
        let mut b = [0u8; 8];
        crate::plat::fs::random(&mut b)?;
        let d = std::env::temp_dir().join(format!("bp-mcprun-{}", hex::encode(b)));
        std::fs::create_dir_all(&d)?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700))?; }
        let p = d.join("h");
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&p)?.write_all(body.as_bytes())?;
        }
        #[cfg(not(unix))] {
            std::fs::write(&p, body)?;
        }
        Ok(Tmp(p))
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        if let Some(d) = self.0.parent() { let _ = std::fs::remove_dir(d); }
    }
}
fn has_header(headers: &[(String, String)], name: &str) -> bool { headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name)) }
fn curl_cfg_text(remote_url: &str, headers: &[(String, String)], extra: &[(&str, &str)]) -> String {
    let mut body = format!("url = \"{}\"\n", esc(remote_url));
    for (k, v) in headers { body.push_str(&format!("header = \"{}: {}\"\n", esc(k), esc(v))); }
    for (k, v) in extra { body.push_str(&format!("header = \"{}: {}\"\n", esc(k), esc(v))); }
    body
}
fn split_header_block(s: &str) -> Option<(&str, &str)> {
    match (s.find("\r\n\r\n"), s.find("\n\n")) {
        (Some(a), Some(b)) => if a <= b { Some((&s[..a], &s[a + 4..])) } else { Some((&s[..b], &s[b + 2..])) },
        (Some(a), None) => Some((&s[..a], &s[a + 4..])),
        (None, Some(b)) => Some((&s[..b], &s[b + 2..])),
        (None, None) => None,
    }
}

/// POST one JSON-RPC line to a remote MCP endpoint via `curl` with URL and headers in a private `-K` file.
pub fn http_post(remote_url: &str, headers: &[(String, String)], sid: &str, proto: &str, body: &str) -> Result<(u16, String, Vec<Value>), String> {
    let mut extra: Vec<(&str, &str)> = Vec::new();
    if !has_header(headers, "Content-Type") { extra.push(("Content-Type", "application/json")); }
    if !has_header(headers, "Accept") { extra.push(("Accept", "application/json, text/event-stream")); }
    if !proto.is_empty() && !has_header(headers, "MCP-Protocol-Version") { extra.push(("MCP-Protocol-Version", proto)); }
    if !sid.is_empty() && !has_header(headers, "Mcp-Session-Id") { extra.push(("Mcp-Session-Id", sid)); }
    let cfg = Tmp::new(&curl_cfg_text(remote_url, headers, &extra)).map_err(|e| e.to_string())?;
    let mut child = crate::proc::std_cmd(crate::proc::Kind::Net, "curl")
        .args(["-sS", "--max-time", "120", "--proto", "=https,http", "-D", "-", "-X", "POST", "--data-binary", "@-", "-K"])
        .arg(&cfg.0)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if let Some(mut cin) = child.stdin.take() {
        let _ = cin.write_all(body.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().trim_start_matches("curl: ").to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut rest = text.as_str();
    let mut head = "";
    while rest.starts_with("HTTP/") {
        match split_header_block(rest) {
            Some((h, r)) => { head = h; rest = r; }
            None => { head = rest; rest = ""; break; }
        }
    }
    let code = head.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    let new_sid = head.lines().find_map(|l| {
        l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("mcp-session-id")).map(|(_, v)| v.trim().to_string())
    }).unwrap_or_default();
    Ok((code, new_sid, super::health::messages(rest)))
}

pub fn main(args: &[String]) -> i32 {
    let url = std::env::var("BACKPLANE_MCP_URL").or_else(|_| std::env::var(super::gen::CODEX_URL_VAR)).unwrap_or_default();
    let id = args.first().cloned().unwrap_or_default();
    let spec = match ask(&url, "backplane/mcp_spec", json!({"id": id})) { Ok(s) => s, Err(e) => { eprintln!("backplane --mcp-run {id}: {e}"); return 2 } };
    let off: Vec<String> = spec["disabled_tools"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    match spec["transport"].as_str().unwrap_or("stdio") {
        "http" => run_http(&url, &id, &spec, &off),
        "sse" => run_sse(&url, &id, &spec, &off),
        _ => run_stdio(&url, &id, &spec, &off),
    }
}

fn run_http(hub_url: &str, id: &str, spec: &Value, off: &[String]) -> i32 {
    let remote_url = spec["url"].as_str().unwrap_or("");
    let headers = headers_from_spec(spec);
    run_http_io(std::io::stdin().lock(), std::io::stdout(), hub_url, id, remote_url, &headers, off);
    0
}

/// Bridge stdio JSON-RPC lines to a Streamable HTTP MCP endpoint.
pub fn run_http_io<R: BufRead, W: Write + Send + 'static>(
    reader: R,
    out: W,
    hub_url: &str,
    id: &str,
    remote_url: &str,
    headers: &[(String, String)],
    off: &[String],
) {
    let out = Arc::new(Mutex::new(out));
    let session_id = Arc::new(Mutex::new(String::new()));
    let proto = Arc::new(Mutex::new("2025-06-18".to_string()));
    let mut hs: Vec<std::thread::JoinHandle<()>> = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() { continue }
        let v = serde_json::from_str::<Value>(&line).ok();
        if let Some(ref v) = v {
            if let Some(r) = refuse(v, off) { write_out(&out, &r.to_string()); continue }
        }
        let method = v.as_ref().and_then(|v| v["method"].as_str()).unwrap_or("").to_string();
        let tool_name = v.as_ref().and_then(|v| v.pointer("/params/name").and_then(Value::as_str)).unwrap_or("").to_string();
        let is_init = method == "initialize";
        let is_sync = is_init || method == "notifications/initialized";
        if is_init {
            if let Some(pv) = v.as_ref().and_then(|v| v.pointer("/params/protocolVersion").and_then(Value::as_str)).filter(|s| !s.is_empty()) {
                *proto.lock().unwrap() = pv.to_string();
            }
        }
        let handle_one = {
            let out = out.clone();
            let session_id = session_id.clone();
            let proto = proto.clone();
            let hub_url = hub_url.to_string();
            let id = id.to_string();
            let remote_url = remote_url.to_string();
            let headers = headers.to_vec();
            let off = off.to_vec();
            move || {
                let cur_sid = session_id.lock().unwrap().clone();
                let cur_proto = proto.lock().unwrap().clone();
                match http_post(&remote_url, &headers, &cur_sid, &cur_proto, &line) {
                    Ok((code, new_sid, msgs)) => {
                        if !new_sid.is_empty() { *session_id.lock().unwrap() = new_sid; }
                        if is_init {
                            if let Some(pv) = msgs.iter().find_map(|m| m.pointer("/result/protocolVersion").and_then(Value::as_str)).filter(|s| !s.is_empty()) {
                                *proto.lock().unwrap() = pv.to_string();
                            }
                        }
                        if msgs.is_empty() {
                            if (200..300).contains(&code) || method == "server/discover" {
                                if let Some(ref v) = v {
                                    if let Some(err) = rpc_err_val(v, -32601, "Method not found") { write_out(&out, &err); }
                                }
                            } else {
                                let msg = format!("MCP server answered HTTP {code}");
                                if is_init { tell(&hub_url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""})); }
                                else if method == "tools/call" { tell(&hub_url, "backplane/mcp_used", json!({"id": id, "tool": tool_name, "ok": false, "error": msg})); }
                                if let Some(ref v) = v {
                                    if let Some(err) = rpc_err_val(v, -32000, &msg) { write_out(&out, &err); }
                                }
                            }
                        } else {
                            if is_init && !(200..300).contains(&code) {
                                let err = msgs.iter().find_map(|m| m.pointer("/error/message").and_then(Value::as_str)).unwrap_or("");
                                let msg = if err.is_empty() { format!("MCP server answered HTTP {code}") } else { err.to_string() };
                                tell(&hub_url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""}));
                            }
                            for m in msgs {
                                let text = if method == "tools/list" && m.get("result").is_some() {
                                    filter_list(m, &off).to_string()
                                } else {
                                    if method == "tools/call" && m.get("method").is_none() {
                                        report_tool_call(&hub_url, &id, &tool_name, &m);
                                    }
                                    m.to_string()
                                };
                                write_out(&out, &text);
                            }
                        }
                    }
                    Err(e) => {
                        if method == "server/discover" {
                            if let Some(ref v) = v {
                                if let Some(err) = rpc_err_val(v, -32601, "Method not found") { write_out(&out, &err); }
                            }
                        } else {
                            let msg = format!("MCP server is not reachable: {e}");
                            if is_init { tell(&hub_url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""})); }
                            else if method == "tools/call" { tell(&hub_url, "backplane/mcp_used", json!({"id": id, "tool": tool_name, "ok": false, "error": msg})); }
                            if let Some(ref v) = v {
                                if let Some(err) = rpc_err_val(v, -32000, &msg) { write_out(&out, &err); }
                            }
                        }
                    }
                }
            }
        };
        if is_sync {
            handle_one();
        } else {
            hs.push(std::thread::spawn(handle_one));
            hs.retain(|h| !h.is_finished());
        }
    }
    for h in hs { let _ = h.join(); }
}

fn resolve_sse_endpoint(base_url: &str, d: &str) -> String {
    if d.starts_with("http://") || d.starts_with("https://") {
        d.to_string()
    } else {
        let base: String = base_url.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
        format!("{base}{}", if d.starts_with('/') { d.to_string() } else { format!("/{d}") })
    }
}

fn run_sse(hub_url: &str, id: &str, spec: &Value, off: &[String]) -> i32 {
    let remote_url = spec["url"].as_str().unwrap_or("");
    let headers = headers_from_spec(spec);
    run_sse_io(std::io::stdin().lock(), std::io::stdout(), hub_url, id, remote_url, &headers, off)
}

struct SseState {
    ep: Option<String>,
    closed: bool,
    pending: HashMap<String, (String, String, Value)>,
}

/// Bridge stdio JSON-RPC lines to a legacy HTTP+SSE MCP endpoint.
pub fn run_sse_io<R: BufRead, W: Write + Send + 'static>(
    reader: R,
    out: W,
    hub_url: &str,
    id: &str,
    remote_url: &str,
    headers: &[(String, String)],
    off: &[String],
) -> i32 {
    let extra: Vec<(&str, &str)> = if has_header(headers, "Accept") { vec![] } else { vec![("Accept", "text/event-stream")] };
    let cfg_body = curl_cfg_text(remote_url, headers, &extra);
    let mut child = match crate::proc::std_cmd(crate::proc::Kind::Net, "curl")
        .args(["-sSN", "--proto", "=https,http", "-K", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("could not run curl: {e}");
            eprintln!("{msg}");
            tell(hub_url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""}));
            return 127;
        }
    };
    if let Some(mut cin) = child.stdin.take() {
        let _ = cin.write_all(cfg_body.as_bytes());
    }
    let cout = child.stdout.take().unwrap();
    let cerr = child.stderr.take().unwrap();
    let tail: Arc<Mutex<VecDeque<String>>> = Default::default();
    {
        let tail = tail.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::new(cerr);
            let mut line = String::new();
            while r.read_line(&mut line).ok().filter(|&n| n > 0).is_some() {
                let trimmed = line.trim_end().chars().take(400).collect::<String>();
                if !trimmed.is_empty() {
                    let mut t = tail.lock().unwrap();
                    t.push_back(trimmed);
                    if t.len() > 40 { t.pop_front(); }
                }
                line.clear();
            }
        });
    }
    let out = Arc::new(Mutex::new(out));
    let state = Arc::new((Mutex::new(SseState { ep: None, closed: false, pending: HashMap::new() }), Condvar::new()));

    let sse_thread = {
        let state = state.clone();
        let out = out.clone();
        let hub_url = hub_url.to_string();
        let id = id.to_string();
        let remote_url = remote_url.to_string();
        let off = off.to_vec();
        std::thread::spawn(move || {
            let mut ev = String::new();
            let mut data = String::new();
            let mut lines = BufReader::new(cout).lines();
            let mut handle_event = |e: &str, d: &str| {
                if e == "endpoint" {
                    let ep = resolve_sse_endpoint(&remote_url, d.trim());
                    let (lk, cv) = &*state;
                    lk.lock().unwrap().ep = Some(ep);
                    cv.notify_all();
                } else if !d.trim().is_empty() {
                    if let Ok(v) = serde_json::from_str::<Value>(d) {
                        let mut text = v.to_string();
                        if v.get("method").is_none() && v.get("id").is_some() {
                            let removed = {
                                let (lk, cv) = &*state;
                                let r = lk.lock().unwrap().pending.remove(&key(&v["id"]));
                                cv.notify_all();
                                r
                            };
                            if let Some((m, tool, _)) = removed {
                                if m == "tools/list" && v.get("result").is_some() {
                                    text = filter_list(v, &off).to_string();
                                } else if m == "tools/call" {
                                    report_tool_call(&hub_url, &id, &tool, &v);
                                }
                            }
                        }
                        write_out(&out, &text);
                    }
                }
            };
            while let Some(Ok(l)) = lines.next() {
                let l = l.trim_end_matches('\r');
                if let Some(e) = l.strip_prefix("event:") { ev = e.trim().into(); continue }
                if let Some(d) = l.strip_prefix("data:") {
                    if !data.is_empty() { data.push('\n'); }
                    data.push_str(d.trim_start());
                    continue;
                }
                if l.is_empty() {
                    let (e, d) = (std::mem::take(&mut ev), std::mem::take(&mut data));
                    handle_event(&e, &d);
                }
            }
            if !data.is_empty() {
                handle_event(&ev, &data);
            }
            let leftover = {
                let (lk, cv) = &*state;
                let mut st = lk.lock().unwrap();
                st.closed = true;
                let drained = std::mem::take(&mut st.pending);
                cv.notify_all();
                drained
            };
            let msg = "SSE stream closed before reply";
            for (_, (method, tool_name, req_val)) in leftover {
                if method == "initialize" { tell(&hub_url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""})); }
                else if method == "tools/call" { tell(&hub_url, "backplane/mcp_used", json!({"id": id, "tool": tool_name, "ok": false, "error": msg})); }
                if let Some(err) = rpc_err_val(&req_val, -32000, msg) { write_out(&out, &err); }
            }
        })
    };

    let mut hs: Vec<std::thread::JoinHandle<()>> = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() { continue }
        let v = serde_json::from_str::<Value>(&line).ok();
        if let Some(ref v) = v {
            if let Some(r) = refuse(v, off) { write_out(&out, &r.to_string()); continue }
        }
        let method = v.as_ref().and_then(|v| v["method"].as_str()).unwrap_or("").to_string();
        let tool_name = v.as_ref().and_then(|v| v.pointer("/params/name").and_then(Value::as_str)).unwrap_or("").to_string();
        if method == "server/discover" {
            if let Some(ref v) = v {
                if let Some(err) = rpc_err_val(v, -32601, "Method not found") { write_out(&out, &err); }
            }
            continue;
        }
        let (ep, closed, req_id_key) = {
            let (lk, cv) = &*state;
            let mut st = lk.lock().unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            while st.ep.is_none() && !st.closed {
                let rem = deadline.saturating_duration_since(Instant::now());
                if rem.is_zero() { break }
                let (next, res) = cv.wait_timeout(st, rem).unwrap();
                st = next;
                if res.timed_out() { break }
            }
            let rk = if st.ep.is_some() && !st.closed {
                v.as_ref().and_then(|v| v.get("id").filter(|i| !i.is_null())).map(|id_v| {
                    let k = key(id_v);
                    st.pending.insert(k.clone(), (method.clone(), tool_name.clone(), v.clone().unwrap()));
                    k
                })
            } else {
                None
            };
            (st.ep.clone(), st.closed, rk)
        };
        let Some(ep) = ep.filter(|_| !closed) else {
            let msg = if closed { "SSE stream closed" } else { "SSE stream ended without naming an endpoint" };
            let stderr = tail.lock().unwrap().iter().cloned().collect::<Vec<_>>().join("\n");
            if method == "initialize" { tell(hub_url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": stderr})); }
            else if method == "tools/call" { tell(hub_url, "backplane/mcp_used", json!({"id": id, "tool": tool_name, "ok": false, "error": msg})); }
            if let Some(ref v) = v {
                if let Some(err) = rpc_err_val(v, -32000, msg) { write_out(&out, &err); }
            }
            continue;
        };
        let out = out.clone();
        let state = state.clone();
        let hub_url = hub_url.to_string();
        let id = id.to_string();
        let headers = headers.to_vec();
        let off = off.to_vec();
        hs.push(std::thread::spawn(move || {
            let take_pending = || -> bool {
                match req_id_key.as_ref() {
                    Some(rk) => {
                        let (lk, cv) = &*state;
                        let had = lk.lock().unwrap().pending.remove(rk).is_some();
                        cv.notify_all();
                        had
                    }
                    None => true,
                }
            };
            match http_post(&ep, &headers, "", "", &line) {
                Ok((code, _, msgs)) if (200..300).contains(&code) => {
                    if !msgs.is_empty() && take_pending() {
                        for m in msgs {
                            let text = if method == "tools/list" && m.get("result").is_some() {
                                filter_list(m, &off).to_string()
                            } else {
                                if method == "tools/call" && m.get("method").is_none() {
                                    report_tool_call(&hub_url, &id, &tool_name, &m);
                                }
                                m.to_string()
                            };
                            write_out(&out, &text);
                        }
                    }
                }
                Ok((code, _, msgs)) => {
                    if take_pending() {
                        if !msgs.is_empty() {
                            for m in msgs { write_out(&out, &m.to_string()); }
                        } else {
                            let msg = format!("SSE POST answered HTTP {code}");
                            if method == "initialize" { tell(&hub_url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""})); }
                            else if method == "tools/call" { tell(&hub_url, "backplane/mcp_used", json!({"id": id, "tool": tool_name, "ok": false, "error": msg})); }
                            if let Some(ref v) = v {
                                if let Some(err) = rpc_err_val(v, -32000, &msg) { write_out(&out, &err); }
                            }
                        }
                    }
                }
                Err(e) => {
                    if take_pending() {
                        let msg = format!("MCP server is not reachable: {e}");
                        if method == "initialize" { tell(&hub_url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""})); }
                        else if method == "tools/call" { tell(&hub_url, "backplane/mcp_used", json!({"id": id, "tool": tool_name, "ok": false, "error": msg})); }
                        if let Some(ref v) = v {
                            if let Some(err) = rpc_err_val(v, -32000, &msg) { write_out(&out, &err); }
                        }
                    }
                }
            }
        }));
        hs.retain(|h| !h.is_finished());
    }
    for h in hs { let _ = h.join(); }
    {
        let (lk, cv) = &*state;
        let mut st = lk.lock().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !st.pending.is_empty() && !st.closed {
            let rem = deadline.saturating_duration_since(Instant::now());
            if rem.is_zero() { break }
            let (next, res) = cv.wait_timeout(st, rem).unwrap();
            st = next;
            if res.timed_out() { break }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = sse_thread.join();
    0
}

fn run_stdio(url: &str, id: &str, spec: &Value, off: &[String]) -> i32 {
    let s = |k: &str| spec[k].as_str().unwrap_or("").to_string();
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Shell, s("command"));
    if id.starts_with("project-") {
        c.env_clear();
        for name in ["HOME", "USERPROFILE", "SYSTEMROOT", "TEMP", "TMP", "TMPDIR", "APPDATA", "LOCALAPPDATA"] {
            if let Some(value) = std::env::var_os(name) { c.env(name, value); }
        }
    }
    c.args(spec["args"].as_array().into_iter().flatten().filter_map(Value::as_str));
    for kv in spec["env"].as_array().into_iter().flatten() { if let (Some(k), Some(v)) = (kv[0].as_str(), kv[1].as_str()) { c.env(k, v); } }
    for name in ["BACKPLANE_MCP_URL", super::gen::CODEX_URL_VAR, "BACKPLANE_PI_MCP", "BACKPLANE_OMP_MCP"] {
        c.env_remove(name);
    }
    let cwd = s("cwd"); if !cwd.is_empty() { c.current_dir(&cwd); }
    c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = match c.spawn() { Ok(ch) => ch, Err(e) => {
        let msg = format!("could not start {}: {e}", s("command")); eprintln!("{msg}");
        let _ = ask(url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""})); return 127 } };
    let (mut cin, cout, mut cerr) = (child.stdin.take().unwrap(), child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let out = Arc::new(Mutex::new(std::io::stdout()));
    // requests in flight we look at the answer of: tools/list (filtered), tools/call (reported)
    let pending: Arc<Mutex<HashMap<String, (String, String)>>> = Default::default();
    let tail: Arc<Mutex<VecDeque<String>>> = Default::default();
    { let tail = tail.clone(); std::thread::spawn(move || { let mut buf = [0u8; 4096]; let mut err = std::io::stderr(); let mut acc = String::new();
        loop { match cerr.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => { let _ = err.write_all(&buf[..n]); acc.push_str(&String::from_utf8_lossy(&buf[..n]));
            while let Some(i) = acc.find('\n') { let l: String = acc.drain(..=i).collect(); let mut t = tail.lock().unwrap(); t.push_back(l.trim_end().chars().take(400).collect()); if t.len() > 40 { t.pop_front(); } } } } } }); }
    { let (out, pending, off) = (out.clone(), pending.clone(), off.to_vec()); std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break }; if line.trim().is_empty() { continue }
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if let Some(r) = refuse(&v, &off) { write_out(&out, &r.to_string()); continue }
                let m = v["method"].as_str().unwrap_or("");
                if (m == "tools/list" || m == "tools/call") && !v["id"].is_null() { pending.lock().unwrap().insert(key(&v["id"]), (m.to_string(), v.pointer("/params/name").and_then(Value::as_str).unwrap_or("").to_string())); }
            }
            if writeln!(cin, "{line}").and_then(|_| cin.flush()).is_err() { break }
        }
        drop(cin);
    }); }
    let mut answered = 0u64;
    for line in BufReader::new(cout).lines() {
        let Ok(line) = line else { break };
        let mut text = line;
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            if v.get("method").is_none() { if let Some((m, tool)) = pending.lock().unwrap().remove(&key(&v["id"])) {
                if m == "tools/list" { text = filter_list(v, off).to_string() }
                else { report_tool_call(url, id, &tool, &v); }
            } }
            answered += 1;
        }
        write_out(&out, &text);
    }
    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(1);
    if code != 0 { std::thread::sleep(std::time::Duration::from_millis(100)); let t: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
        let _ = ask(url, "backplane/mcp_failed", json!({"id": id, "error": format!("exited with code {code}{}", if answered == 0 { " before it answered" } else { "" }), "stderr": t.join("\n")})); }
    code
}

/// `--mcp-headers <id> <url>`: the remote server's headers, secrets put in, as one JSON object on stdout.
pub fn headers_main(args: &[String]) -> i32 {
    let (id, url) = (args.first().cloned().unwrap_or_default(), args.get(1).cloned().unwrap_or_default());
    match ask(&url, "backplane/mcp_headers", json!({"id": id})) { Ok(h) => { println!("{}", h); 0 } Err(e) => { eprintln!("backplane --mcp-headers {id}: {e}"); 1 } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn turned_off_tools_are_hidden_and_refused() {
        let off = vec!["delete".to_string()];
        let l = filter_list(json!({"jsonrpc":"2.0","id":3,"result":{"tools":[{"name":"echo"},{"name":"delete"}]}}), &off);
        assert_eq!(l["result"]["tools"], json!([{"name":"echo"}]));
        let r = refuse(&json!({"jsonrpc":"2.0","id":"x","method":"tools/call","params":{"name":"delete","arguments":{}}}), &off).unwrap();
        assert_eq!(r["id"], "x"); assert!(r["error"]["message"].as_str().unwrap().contains("turned off"));
        assert!(refuse(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"echo"}}), &off).is_none());
        assert!(refuse(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}), &off).is_none());
    }

    fn read_http_req(s: &mut std::net::TcpStream) -> (String, String, String) {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let head_end = loop {
            let n = s.read(&mut tmp).unwrap();
            assert!(n > 0, "EOF before headers");
            buf.extend_from_slice(&tmp[..n]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") { break i + 4; }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let clen = head.lines()
            .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("content-length")).and_then(|(_, v)| v.trim().parse::<usize>().ok()))
            .unwrap_or(0);
        while buf.len() < head_end + clen {
            let n = s.read(&mut tmp).unwrap();
            if n == 0 { break }
            buf.extend_from_slice(&tmp[..n]);
        }
        let req_line = head.lines().next().unwrap_or("").to_string();
        let body = String::from_utf8_lossy(&buf[head_end..head_end + clen]).into_owned();
        (req_line, head, body)
    }

    #[test] fn remote_http_bridge_forwards_headers_session_and_filters_tools() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let seen_secret = Arc::new(Mutex::new(false));
        let seen_secret_srv = seen_secret.clone();
        let server = std::thread::spawn(move || {
            for s in l.incoming().take(4) {
                let mut s = s.unwrap();
                let (_line, head, body) = read_http_req(&mut s);
                assert!(head.contains("Authorization: Bearer secret-tok"), "missing auth header: {head}");
                assert!(head.contains("X-Mode: ro"), "missing custom header: {head}");
                *seen_secret_srv.lock().unwrap() = true;
                let v: Value = serde_json::from_str(&body).unwrap();
                let resp = match v["method"].as_str().unwrap_or("") {
                    "server/discover" => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
                    "initialize" => {
                        let b = json!({"jsonrpc":"2.0","id":v["id"],"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"qrpedia","version":"1.0"}}}).to_string();
                        format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: sess-1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len())
                    }
                    "tools/list" => {
                        assert!(head.contains("Mcp-Session-Id: sess-1"), "missing session id: {head}");
                        let b = format!("event: message\ndata: {}\n\n", json!({"jsonrpc":"2.0","id":v["id"],"result":{"tools":[{"name":"qr_create"},{"name":"qr_delete"}]}}));
                        format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len())
                    }
                    "tools/call" => {
                        assert!(head.contains("Mcp-Session-Id: sess-1"), "missing session id: {head}");
                        let name = v["params"]["name"].as_str().unwrap();
                        assert_eq!(name, "qr_create", "disabled tool must never reach remote server");
                        let b = json!({"jsonrpc":"2.0","id":v["id"],"result":{"content":[{"type":"text","text":"ok"}]}}).to_string();
                        format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len())
                    }
                    other => panic!("unexpected method: {other}"),
                };
                s.write_all(resp.as_bytes()).unwrap();
            }
        });

        let input = [
            r#"{"jsonrpc":"2.0","id":1,"method":"server/discover"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"qr_delete","arguments":{}}}"#,
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"qr_create","arguments":{"text":"hi"}}}"#,
        ].join("\n") + "\n";

        let out_buf: Arc<Mutex<Vec<u8>>> = Default::default();
        struct SharedBuf(Arc<Mutex<Vec<u8>>>);
        impl Write for SharedBuf {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(buf); Ok(buf.len()) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let headers = vec![("Authorization".into(), "Bearer secret-tok".into()), ("X-Mode".into(), "ro".into())];
        run_http_io(std::io::Cursor::new(input), SharedBuf(out_buf.clone()), "", "qrpedia", &format!("http://{addr}/mcp"), &headers, &["qr_delete".into()]);
        server.join().unwrap();
        assert!(*seen_secret.lock().unwrap());

        let lines: Vec<Value> = String::from_utf8(out_buf.lock().unwrap().clone()).unwrap()
            .lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();
        let by_id = |i: u64| lines.iter().find(|v| v["id"] == i).cloned().unwrap_or_else(|| panic!("missing id {i} in {lines:#?}"));
        assert_eq!(by_id(1)["error"]["code"], -32601);
        assert_eq!(by_id(2)["result"]["serverInfo"]["name"], "qrpedia");
        assert_eq!(by_id(3)["result"]["tools"], json!([{"name":"qr_create"}]));
        assert!(by_id(4)["error"]["message"].as_str().unwrap().contains("turned off"));
        assert_eq!(by_id(5)["result"]["content"][0]["text"], "ok");
    }

    #[test] fn remote_sse_bridge_connects_and_forwards_calls() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut sse_stream, _) = l.accept().unwrap();
            let (req_line, head, _) = read_http_req(&mut sse_stream);
            assert!(req_line.starts_with("GET /sse "), "{req_line}");
            assert!(head.contains("Authorization: Bearer sse-tok"), "{head}");
            sse_stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\nevent: endpoint\ndata: /msg\n\n").unwrap();
            sse_stream.flush().unwrap();

            for _ in 0..3 {
                let (mut s, _) = l.accept().unwrap();
                let (req_line, head, body) = read_http_req(&mut s);
                assert!(req_line.starts_with("POST /msg "), "{req_line}");
                assert!(head.contains("Authorization: Bearer sse-tok"), "{head}");
                s.write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                let v: Value = serde_json::from_str(&body).unwrap();
                let reply = match v["method"].as_str().unwrap_or("") {
                    "initialize" => json!({"jsonrpc":"2.0","id":v["id"],"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"legacy","version":"1"}}}),
                    "tools/list" => json!({"jsonrpc":"2.0","id":v["id"],"result":{"tools":[{"name":"echo"},{"name":"bad"}]}}),
                    "tools/call" => {
                        assert_eq!(v["params"]["name"], "echo");
                        json!({"jsonrpc":"2.0","id":v["id"],"result":{"content":[{"type":"text","text":"pong"}]}})
                    }
                    other => panic!("unexpected method: {other}"),
                };
                sse_stream.write_all(format!("event: message\ndata: {reply}\n\n").as_bytes()).unwrap();
                sse_stream.flush().unwrap();
            }
        });

        let input = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bad","arguments":{}}}"#,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"echo","arguments":{}}}"#,
        ].join("\n") + "\n";

        let out_buf: Arc<Mutex<Vec<u8>>> = Default::default();
        struct SharedBuf(Arc<Mutex<Vec<u8>>>);
        impl Write for SharedBuf {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(buf); Ok(buf.len()) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let headers = vec![("Authorization".into(), "Bearer sse-tok".into())];
        let code = run_sse_io(std::io::Cursor::new(input), SharedBuf(out_buf.clone()), "", "legacy", &format!("http://{addr}/sse"), &headers, &["bad".into()]);
        assert_eq!(code, 0);
        server.join().unwrap();

        let lines: Vec<Value> = String::from_utf8(out_buf.lock().unwrap().clone()).unwrap()
            .lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();
        let by_id = |i: u64| lines.iter().find(|v| v["id"] == i).cloned().unwrap_or_else(|| panic!("missing id {i} in {lines:#?}"));
        assert_eq!(by_id(1)["result"]["serverInfo"]["name"], "legacy");
        assert_eq!(by_id(2)["result"]["tools"], json!([{"name":"echo"}]));
        assert!(by_id(3)["error"]["message"].as_str().unwrap().contains("turned off"));
        assert_eq!(by_id(4)["result"]["content"][0]["text"], "pong");
    }

    #[test] fn curl_cfg_text_puts_url_in_config_and_escapes_headers() {
        let cfg = curl_cfg_text(
            "https://example.com/mcp?tok=a\"b\nc",
            &[("Authorization".into(), "Bearer x\"\\y\nz".into())],
            &[("Accept", "application/json")],
        );
        assert_eq!(
            cfg,
            "url = \"https://example.com/mcp?tok=a\\\"b c\"\nheader = \"Authorization: Bearer x\\\"\\\\y z\"\nheader = \"Accept: application/json\"\n"
        );
    }

    #[test] fn remote_sse_mid_stream_drop_fails_pending_immediately() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut sse_stream, _) = l.accept().unwrap();
            let _ = read_http_req(&mut sse_stream);
            sse_stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: endpoint\ndata: /msg\n\n").unwrap();
            sse_stream.flush().unwrap();

            let (mut s, _) = l.accept().unwrap();
            let _ = read_http_req(&mut s);
            s.write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            // Drop SSE stream before replying to the pending call.
            drop(sse_stream);
        });

        let input = [
            r#"{"jsonrpc":"2.0","id":1,"method":"server/discover"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{}}}"#,
        ].join("\n") + "\n";

        let out_buf: Arc<Mutex<Vec<u8>>> = Default::default();
        struct SharedBuf(Arc<Mutex<Vec<u8>>>);
        impl Write for SharedBuf {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(buf); Ok(buf.len()) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let code = run_sse_io(std::io::Cursor::new(input), SharedBuf(out_buf.clone()), "", "legacy", &format!("http://{addr}/sse"), &[], &[]);
        assert_eq!(code, 0);
        server.join().unwrap();

        let lines: Vec<Value> = String::from_utf8(out_buf.lock().unwrap().clone()).unwrap()
            .lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();
        let by_id = |i: u64| lines.iter().find(|v| v["id"] == i).cloned().unwrap_or_else(|| panic!("missing id {i} in {lines:#?}"));
        assert_eq!(by_id(1)["error"]["code"], -32601);
        assert_eq!(by_id(2)["error"]["code"], -32000);
        assert!(by_id(2)["error"]["message"].as_str().unwrap().contains("SSE stream closed"));
    }
}

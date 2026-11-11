//! `backplane-hub --mcp-run <id>`: how an agent starts a stdio server from the list (gen.rs). It asks the hub (`BACKPLANE_MCP_URL`, the thread's
//! own endpoint for this agent process, agents/mcp.rs) for the server's command with its env and the vault's values put in, starts it and passes
//! JSON-RPC lines both ways. Tools turned off are taken out of `tools/list` and a call to one is answered with an error without reaching the
//! server. Each tool call is reported to the hub (the card's last use), and a server that dies is reported with the end of its stderr.
//!
//! `backplane-hub --mcp-headers <id> <url>`: Claude's `headersHelper` for a remote server with secret headers: prints them as one JSON object.
use crate::agents::mcp_stdio::{endpoint, post};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::{Arc, Mutex};

/// A private method on the thread's endpoint: its result, or the error's message.
pub fn ask(url: &str, method: &str, params: Value) -> Result<Value, String> {
    let (hp, path) = endpoint(url).ok_or("BACKPLANE_MCP_URL is not this hub's loopback address")?;
    let (code, body) = post(&hp, &path, &json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string()).map_err(|e| format!("Backplane is not reachable: {e}"))?;
    if code != 200 { return Err(format!("Backplane answered {code}")) }
    let v: Value = serde_json::from_str(body.trim()).map_err(|_| "Backplane's answer is not JSON".to_string())?;
    if let Some(e) = v.get("error") { return Err(e["message"].as_str().unwrap_or("refused").to_string()) }
    Ok(v["result"].clone())
}
fn tell(url: &str, method: &str, params: Value) { let (u, m) = (url.to_string(), method.to_string()); std::thread::spawn(move || { let _ = ask(&u, &m, params); }); }

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

pub fn main(args: &[String]) -> i32 {
    let url = std::env::var("BACKPLANE_MCP_URL").unwrap_or_default();
    let id = args.first().cloned().unwrap_or_default();
    let spec = match ask(&url, "backplane/mcp_spec", json!({"id": id})) { Ok(s) => s, Err(e) => { eprintln!("backplane --mcp-run {id}: {e}"); return 2 } };
    let s = |k: &str| spec[k].as_str().unwrap_or("").to_string();
    let off: Vec<String> = spec["disabled_tools"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Shell, s("command"));
    c.args(spec["args"].as_array().into_iter().flatten().filter_map(Value::as_str));
    for kv in spec["env"].as_array().into_iter().flatten() { if let (Some(k), Some(v)) = (kv[0].as_str(), kv[1].as_str()) { c.env(k, v); } }
    c.env_remove("BACKPLANE_MCP_URL");
    let cwd = s("cwd"); if !cwd.is_empty() { c.current_dir(&cwd); }
    c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = match c.spawn() { Ok(ch) => ch, Err(e) => {
        let msg = format!("could not start {}: {e}", s("command")); eprintln!("{msg}");
        let _ = ask(&url, "backplane/mcp_failed", json!({"id": id, "error": msg, "stderr": ""})); return 127 } };
    let (mut cin, cout, mut cerr) = (child.stdin.take().unwrap(), child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let write = |o: &Arc<Mutex<std::io::Stdout>>, v: &str| { let mut o = o.lock().unwrap(); let _ = writeln!(o, "{v}"); let _ = o.flush(); };
    // requests in flight we look at the answer of: tools/list (filtered), tools/call (reported)
    let pending: Arc<Mutex<HashMap<String, (String, String)>>> = Default::default();
    let tail: Arc<Mutex<VecDeque<String>>> = Default::default();
    { let tail = tail.clone(); std::thread::spawn(move || { let mut buf = [0u8; 4096]; let mut err = std::io::stderr(); let mut acc = String::new();
        loop { match cerr.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => { let _ = err.write_all(&buf[..n]); acc.push_str(&String::from_utf8_lossy(&buf[..n]));
            while let Some(i) = acc.find('\n') { let l: String = acc.drain(..=i).collect(); let mut t = tail.lock().unwrap(); t.push_back(l.trim_end().chars().take(400).collect()); if t.len() > 40 { t.pop_front(); } } } } } }); }
    { let (out, pending, off) = (out.clone(), pending.clone(), off.clone()); std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break }; if line.trim().is_empty() { continue }
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if let Some(r) = refuse(&v, &off) { write(&out, &r.to_string()); continue }
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
                if m == "tools/list" { text = filter_list(v, &off).to_string() }
                else { let ok = v.get("error").is_none() && v.pointer("/result/isError") != Some(&Value::Bool(true));
                    let err = if ok { String::new() } else { v.pointer("/error/message").or_else(|| v.pointer("/result/content/0/text")).and_then(Value::as_str).unwrap_or("").chars().take(300).collect() };
                    tell(&url, "backplane/mcp_used", json!({"id": id, "tool": tool, "ok": ok, "error": err})) }
            } }
            answered += 1;
        }
        write(&out, &text);
    }
    let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(1);
    if code != 0 { std::thread::sleep(std::time::Duration::from_millis(100)); let t: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
        let _ = ask(&url, "backplane/mcp_failed", json!({"id": id, "error": format!("exited with code {code}{}", if answered == 0 { " before it answered" } else { "" }), "stderr": t.join("\n")})); }
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
}

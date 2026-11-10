//! `backplane-hub --mcp-stdio`: Backplane's MCP server for an agent that only starts stdio servers (Antigravity's agy CLI, providers/agy_cli.rs).
//! Each JSON-RPC line on stdin is POSTed to `BACKPLANE_MCP_URL` (the thread's endpoint for this agent process, agents/mcp.rs: plain HTTP on
//! loopback, one JSON reply or 202 for a notification), and the reply is written to stdout as one line. Requests run side by side (a
//! thread_wait may hold its reply a long time), their replies written whole, one at a time.
use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

/// The loopback endpoint of an `http://127.0.0.1:PORT/path?query` URL: (host:port, path?query). None for anything else.
pub fn endpoint(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("http://")?; let (hp, path) = match rest.find('/') { Some(i) => (&rest[..i], &rest[i..]), None => (rest, "/") };
    let host = hp.rsplit_once(':').map(|(h, _)| h).unwrap_or(hp);
    if !matches!(host, "127.0.0.1" | "localhost" | "[::1]") { return None }
    Some((hp.to_string(), path.to_string()))
}
/// One POST: the reply's status and body.
pub fn post(hp: &str, path: &str, body: &str) -> std::io::Result<(u16, String)> {
    let mut s = TcpStream::connect(hp)?;
    let req = format!("POST {path} HTTP/1.1\r\nHost: {hp}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    s.write_all(req.as_bytes())?; // one write: the hub reads the head and body it was promised
    let mut buf = Vec::new(); s.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let (head, rest) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = head.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    Ok((status, rest.to_string()))
}
/// A JSON-RPC error for request `id` (none for a notification).
fn rpc_err_code(line: &str, code: i64, msg: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?; let id = v.get("id").filter(|i| !i.is_null())?.clone();
    Some(serde_json::json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}}).to_string())
}
fn rpc_err(line: &str, msg: &str) -> Option<String> { rpc_err_code(line, -32000, msg) }
/// What to write for one line: the server's reply, an error for a request that got none, nothing for a notification.
pub fn answer(hp: &str, path: &str, line: &str) -> Option<String> {
    match post(hp, path, line) {
        Ok((200, b)) if !b.trim().is_empty() => Some(b.trim().replace('\n', " ")),
        // the hub answers a method it does not know with 202 like a notification; a request must still be answered, or a client that tries a newer
        // method first (agy 1.4: `server/discover`, MCP 2026-07-28) waits for ever instead of going on to `initialize`
        Ok((202, _)) => rpc_err_code(line, -32601, "Method not found"),
        Ok((c, _)) => rpc_err(line, &format!("Backplane's MCP server answered {c}")),
        Err(e) => rpc_err(line, &format!("Backplane's MCP server is not reachable: {e}")),
    }
}

pub fn main() -> i32 {
    let url = std::env::var("BACKPLANE_MCP_URL").unwrap_or_default();
    let Some((hp, path)) = endpoint(&url) else { eprintln!("--mcp-stdio: BACKPLANE_MCP_URL is not a loopback http URL"); return 2 };
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let mut hs = vec![];
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break }; if line.trim().is_empty() { continue }
        let (hp, path, out) = (hp.clone(), path.clone(), out.clone());
        hs.push(std::thread::spawn(move || { if let Some(r) = answer(&hp, &path, &line) { let mut o = out.lock().unwrap(); let _ = writeln!(o, "{r}"); let _ = o.flush(); } }));
        hs.retain(|h| !h.is_finished());
    }
    for h in hs { let _ = h.join(); }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_loopback() {
        assert_eq!(endpoint("http://127.0.0.1:5/mcp/t1.2?k=x"), Some(("127.0.0.1:5".into(), "/mcp/t1.2?k=x".into())));
        assert_eq!(endpoint("https://127.0.0.1:5/mcp"), None); assert_eq!(endpoint("http://example.com:5/mcp"), None); assert_eq!(endpoint(""), None);
    }
    #[test] fn replies_notifications_and_failures() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let hp = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || { for s in l.incoming().take(2) { let mut s = s.unwrap(); let mut req = String::new(); let mut b = [0u8; 4096];
            while !(req.contains("\r\n\r\n") && req.trim_end().ends_with('}')) { let n = s.read(&mut b).unwrap(); if n == 0 { break } req.push_str(&String::from_utf8_lossy(&b[..n])) }
            let r = if req.contains("\"id\"") { let body = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}"; format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()) } else { "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".into() };
            s.write_all(r.as_bytes()).unwrap(); } });
        assert_eq!(answer(&hp, "/mcp/t?k=1", r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).as_deref(), Some(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#));
        assert_eq!(answer(&hp, "/mcp/t?k=1", r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
        assert_eq!(rpc_err_code(r#"{"jsonrpc":"2.0","id":7,"method":"server/discover"}"#, -32601, "Method not found").as_deref(), Some(r#"{"jsonrpc":"2.0","id":7,"error":{"code":-32601,"message":"Method not found"}}"#));
        let gone = answer("127.0.0.1:1", "/", r#"{"jsonrpc":"2.0","id":"a","method":"tools/list"}"#).unwrap(); assert!(gone.contains("\"id\":\"a\"") && gone.contains("not reachable"));
        assert_eq!(answer("127.0.0.1:1", "/", r#"{"jsonrpc":"2.0","method":"x"}"#), None);
    }
}

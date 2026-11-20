//! An attached agent (docs/codex-cloud.md, spike): an agent the hub did not start (a Codex Cloud task) gets Backplane's tools for a project.
//! `GET /dev/attach?dir=<folder>` (behind the hub's token, as /dev/turns) finds or makes the project of that folder and its one attached
//! thread (provider `outside`, never given a turn), and answers that thread's MCP URL with no process generation: there is no process to
//! go stale. `backplane-hub --tool <name> '<json>'` and `--mcp-attach` (below) are its two clients: one call from a shell, or a stdio
//! MCP server bridged as `--mcp-stdio` is. Both find the hub from `BACKPLANE_HOME` (its `hub.lock` says the port, its `token` the key).
use super::{lines::hub_id, now_s, Agents};
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub const PROVIDER: &str = "outside";
fn key(project: &str) -> String { format!("attach.thread.{project}") }
/// `%XX` decoded (a folder in a query).
pub fn unpct(s: &str) -> String {
    let b = s.as_bytes(); let mut o = Vec::with_capacity(b.len()); let mut i = 0;
    while i < b.len() { if b[i] == b'%' && i + 2 < b.len() { if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) { o.push(v); i += 3; continue } } o.push(if b[i] == b'+' { b' ' } else { b[i] }); i += 1 }
    String::from_utf8_lossy(&o).into_owned()
}
pub fn pct(s: &str) -> String { s.bytes().map(|c| if c.is_ascii_alphanumeric() || b"/-_.~".contains(&c) { (c as char).to_string() } else { format!("%{c:02X}") }).collect() }

impl Agents {
    /// The attached thread of the project at `dir` (made with the project when missing) and its MCP URL.
    pub async fn attach(&self, dir: &str) -> Result<(String, String), String> {
        let root = std::fs::canonicalize(dir).map_err(|e| format!("{dir}: {e}"))?.to_string_lossy().trim_end_matches('/').to_string();
        let now = now_s();
        let w = self.world().await;
        let mut ev = vec![];
        let proj = match w.roots.iter().find(|(id, r)| r.trim_end_matches('/') == root && w.projects.get(*id).map_or(false, |p| !p.1)) { Some((id, _)) => id.clone(), None => {
            let id = hub_id("p", now, self.seq_n()); let title = root.rsplit('/').find(|x| !x.is_empty()).unwrap_or("project").to_string();
            ev.push(json!({"$":"ProjectCreated","id":id,"title":title,"root":root,"at":now})); id } };
        let have = w.setting(&key(&proj)).to_string();
        let th = if !have.is_empty() && w.threads.get(&have).map_or(false, |t| !t.deleted) { have } else {
            let th = hub_id("t", now, self.seq_n());
            ev.push(json!({"$":"ThreadCreated","id":th,"project":proj,"title":"Codex Cloud","env":"local","branch":"","worktree":"","provider":PROVIDER,"model":"","at":now}));
            ev.push(json!({"$":"SettingSet","key":key(&proj),"value":th,"at":now})); th };
        drop(w);
        if !ev.is_empty() { self.commit_events(ev).await }
        let url = self.mcp.get().map(|m| m.url(&th, "")).ok_or("Backplane's MCP server is not running")?;
        Ok((th, url))
    }
}

pub struct AttachHttp(pub Arc<Agents>);
impl HttpHandler for AttachHttp {
    fn routes(&self) -> &'static [&'static str] { &["/dev/attach"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let dir = unpct(&req.param("dir"));
            if dir.is_empty() { return HttpReply::text("400 Bad Request", "name the folder: ?dir=") }
            let (status, body) = match self.0.attach(&dir).await { Ok((th, url)) => ("200 OK", json!({"thread":th,"url":url})), Err(e) => ("400 Bad Request", json!({"error":e})) };
            HttpReply { status, ctype: "application/json".into(), extra: "Cache-Control: no-store\r\n".into(), body: body.to_string().into_bytes() }
        })
    }
}

// ---- the clients ----
fn home() -> std::path::PathBuf {
    std::env::var("BACKPLANE_HOME").ok().filter(|s| !s.is_empty()).map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".backplane-cloud/home"))
}
/// The attached thread's MCP URL for the folder, asked of the hub serving `BACKPLANE_HOME`.
pub fn url_for(dir: &str) -> Result<String, String> {
    let h = home();
    let lock = std::fs::read_to_string(h.join("hub.lock")).map_err(|e| format!("no hub serves {}: {e} (start it: backplane-hub --home {} --port <port> --headless)", h.display(), h.display()))?;
    let port = lock.split_whitespace().nth(1).ok_or("hub.lock names no port")?;
    let token = std::fs::read_to_string(h.join("token")).map_err(|e| format!("the hub's token: {e}"))?;
    let hp = format!("127.0.0.1:{port}");
    let req = format!("GET /dev/attach?dir={} HTTP/1.1\r\nHost: {hp}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n", pct(dir), token.trim());
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(&hp).map_err(|e| format!("the hub on {hp}: {e}"))?;
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?; let mut b = vec![]; s.read_to_end(&mut b).map_err(|e| e.to_string())?;
    let t = String::from_utf8_lossy(&b); let body = t.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("");
    let v: Value = serde_json::from_str(body).map_err(|_| format!("the hub answered: {}", t.lines().next().unwrap_or("")))?;
    v["url"].as_str().map(str::to_string).ok_or_else(|| v["error"].as_str().unwrap_or("no url").to_string())
}
/// The project folder: the nearest folder up from the working one with `.backplane.json` or `.git`, else the working one.
fn cwd() -> String {
    let here = std::env::current_dir().unwrap_or_else(|_| ".".into());
    here.ancestors().find(|d| d.join(".backplane.json").exists() || d.join(".git").exists()).unwrap_or(&here).to_string_lossy().into_owned()
}
/// `--dir D` out of the arguments (the working folder when absent).
fn take_dir(a: &mut Vec<String>) -> String { match a.iter().position(|x| x == "--dir") { Some(i) if i + 1 < a.len() => { let d = a.remove(i + 1); a.remove(i); d } _ => cwd() } }

/// `backplane-hub --mcp-attach [--dir D]`: Backplane's MCP over stdio for an agent the hub did not start.
pub fn mcp_main(args: &[String]) -> i32 {
    let mut a = args.to_vec(); let dir = take_dir(&mut a);
    match url_for(&dir) { Ok(u) => { std::env::set_var("BACKPLANE_MCP_URL", u); super::mcp_stdio::main() } Err(e) => { eprintln!("--mcp-attach: {e}"); 2 } }
}
/// `backplane-hub --tool <name> '<json>' [--dir D]` | `--tool --list`: one tool call; its text on stdout; exit 1 when the tool says it failed.
pub fn tool_main(args: &[String]) -> i32 {
    let mut a = args.to_vec(); let dir = take_dir(&mut a);
    let url = match url_for(&dir) { Ok(u) => u, Err(e) => { eprintln!("--tool: {e}"); return 2 } };
    let Some((hp, path)) = super::mcp_stdio::endpoint(&url) else { eprintln!("--tool: not a loopback URL"); return 2 };
    let name = a.first().cloned().unwrap_or_default();
    let body = if name.is_empty() || name == "--list" || name == "--describe" { json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}) } else {
        let mut args: Value = match a.get(1) { Some(j) => match serde_json::from_str(j) { Ok(v) => v, Err(e) => { eprintln!("--tool: the arguments are not JSON: {e}"); return 2 } }, None => json!({}) };
        // a path relative to the project, as an agent in a repo writes it, made absolute (the tools take absolute paths)
        if let Some(p) = args.get("path").and_then(Value::as_str).filter(|p| !p.is_empty() && !p.starts_with('/')) { args["path"] = json!(std::path::Path::new(&dir).join(p).to_string_lossy()) }
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}) };
    let (st, out) = match super::mcp_stdio::post(&hp, &path, &body.to_string()) { Ok(x) => x, Err(e) => { eprintln!("--tool: {e}"); return 2 } };
    let v: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
    if st != 200 || v.get("error").is_some() { eprintln!("--tool: {}", if v.is_null() { out } else { v["error"].to_string() }); return 2 }
    let r = &v["result"];
    if let Some(ts) = r["tools"].as_array() {
        // `--tool --describe <name>`: that tool's whole definition (description and input schema)
        if name == "--describe" { let want = a.get(1).cloned().unwrap_or_default(); return match ts.iter().find(|t| t["name"] == want.as_str()) { Some(t) => { println!("{}", serde_json::to_string_pretty(t).unwrap_or_default()); 0 } None => { eprintln!("--tool: no tool {want}"); 1 } } }
        for t in ts { println!("{}\t{}", t["name"].as_str().unwrap_or(""), t["description"].as_str().unwrap_or("").chars().take(140).collect::<String>()) } return 0 }
    for c in r["content"].as_array().into_iter().flatten() { if let Some(t) = c["text"].as_str() { println!("{t}") } else { println!("{c}") } }
    if r["isError"] == true { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_folder_goes_through_the_query_whole() { for d in ["/w/a b/c", "/r/proj%1", "/x/ü"] { assert_eq!(unpct(&pct(d)), d) } }
}

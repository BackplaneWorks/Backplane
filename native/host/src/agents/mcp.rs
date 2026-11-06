//! The Backplane MCP server an agent is started with (`--mcp-config`): JSON-RPC over HTTP POST on a loopback port,
//! one URL per thread and process generation (`/mcp/<thread>.<gen>?k=<HMAC>`), as src/core/mcp.bend and server.bend's
//! Mcp.* do. Ported tools: permission_prompt (with its parked answer), thread_list, thread_read, thread_send,
//! thread_interrupt, thread_wait, thread_update (title), viewer_show, orchestrator_capabilities, kicad_ipc and kicad_ipc_status (the files stream's crate::files::kicad::ipc). Every other tool the JS hub
//! lists (delegation and tasks, work/wake/lock, secrets, terminals, browser, kicad_ipc, deploy, order, review, told, task board,
//! settings) is not listed here and answers "unknown tool": it belongs to the stream that owns that area.
//! tools/list is the JS hub's own definitions (fixtures/mcp-tools.json) for the tools implemented.
use super::{lines::hub_id, now_s, Agents};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{sync::{Arc, Weak}, time::Duration};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::{oneshot, Semaphore}};

/// A set of MCP tools another module serves (the bots' tools): listed per thread, called by name. Register it with `Agents::add_tools` (agents::handle()).
/// `list(thread)` is what that thread's agent is shown in addition to Backplane's own tools (Bots.list); `owns(name)` claims a name before the "unknown tool" arm
/// (after Backplane's own and the term tools); `call` answers with (ok, body): a String body is raw text, {"$content":[..]} a content array, anything else JSON text.
/// The stale-process gate, the secret masks (shaped()) and the ride block stay the agents module's.
pub trait ToolSet: Send + Sync {
    fn list(&self, thread: &str) -> Vec<Value>;
    fn owns(&self, name: &str) -> bool;
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> crate::api::Fut<'a, (bool, Value)>;
}
pub const TOOLS_JSON: &str = include_str!("../../../../test/gpui/agents/fixtures/mcp-tools.json");
pub const IMPLEMENTED: &[&str] = &["orchestrator_capabilities", "review_comment", "review_comments", "work_run", "work_watch", "work_list", "work_cancel", "work_unwatch", "thread_launch", "delegate_task", "task_status", "task_cancel", "thread_list", "thread_read", "thread_send", "thread_update", "thread_wait", "thread_interrupt", "thread_settle", "viewer_show", "permission_prompt", "kicad_ipc", "kicad_ipc_status", "terminal_open", "terminal_read", "terminal_send", "terminal_close", "preview_open", "preview_navigate", "preview_snapshot", "preview_click", "preview_type", "preview_press", "preview_scroll", "preview_evaluate", "preview_wait_for", "preview_resize", "preview_status", "secret_request", "secret_list", "secret_run", "settings_inspect", "settings_propose"];
const MAX_BODY: usize = 4 << 20;
const MAX_CONNS: usize = 256;
const MAX_PARKED_PER_THREAD: usize = 16;

pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 { k[..32].copy_from_slice(&Sha256::digest(key)) } else { k[..key.len()].copy_from_slice(key) }
    let (mut i, mut o) = (k, k); for b in i.iter_mut() { *b ^= 0x36 } for b in o.iter_mut() { *b ^= 0x5c }
    let inner = Sha256::new().chain_update(i).chain_update(msg).finalize();
    Sha256::new().chain_update(o).chain_update(inner).finalize().into()
}
/// Auth.mcp.key: the key a thread's URL carries.
pub fn mcp_key(secret: &str, seg: &str) -> String { hex::encode(hmac_sha256(secret.as_bytes(), format!("mcp\n{seg}").as_bytes())) }
fn same(a: &str, b: &str) -> bool { a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0 }

/// Mcp.result / Mcp.error / Mcp.content
pub fn result(id: &Value, r: Value) -> Value { json!({"jsonrpc":"2.0","id":id,"result":r}) }
pub fn error(id: &Value, code: i64, msg: &str) -> Value { json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":msg}}) }
pub fn content(ok: bool, body: &Value) -> Value { let mut b = body.clone(); crate::settings::mask::json(&mut b); /* Mcp.masked */ json!({"content":[{"type":"text","text":b.to_string()}],"isError":!ok}) }

/// The text of a result that does not pass through `content`'s JSON masking (a terminal's screen, a page's text blocks): the one place a mask is applied to it.
/// The vault's mask (settings lane), as Hub.tagged / Mcp.masked apply it.
fn mask_text(s: &str) -> String { crate::settings::mask::mask(s) }
/// A tool body's shapes (term/tools.rs docs): a String is the raw text, {"$content":[blocks]} is the content array (text blocks masked, images as they are), anything else JSON text.
pub fn shaped(ok: bool, body: &Value) -> Value {
    match body {
        Value::String(s) => json!({"content":[{"type":"text","text":mask_text(s)}],"isError":!ok}),
        Value::Object(o) if o.contains_key("$content") => {
            let bs: Vec<Value> = o["$content"].as_array().cloned().unwrap_or_default().into_iter().map(|mut b| { if b["type"] == "text" { if let Some(t) = b["text"].as_str().map(mask_text) { b["text"] = json!(t) } } b }).collect();
            json!({"content":bs,"isError":!ok})
        }
        _ => content(ok, body),
    }
}
fn fail(msg: &str) -> (bool, Value) { (false, json!({"error":msg})) }

/// Perm.* (mcp.bend): the verdicts and the ask cards.
pub fn perm_text(v: &Value) -> Value { json!({"content":[{"type":"text","text":v.to_string()}]}) }
pub fn perm_allow(input: &Value) -> Value { json!({"behavior":"allow","updatedInput":input}) }
pub fn perm_deny(msg: &str) -> Value { json!({"behavior":"deny","message":msg}) }
pub const PLAN_NOTE: &str = "The client captured your proposed plan. Stop here and wait for the user's feedback or implementation request.";
fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
pub fn perm_detail(input: &Value) -> String {
    for k in ["command", "file_path", "url", "pattern"] { let x = sv(input, k); if !x.is_empty() { return x.to_string() } }
    input.to_string().chars().take(400).collect()
}
fn first_q(input: &Value) -> Value { input.get("questions").and_then(Value::as_array).and_then(|a| a.first()).cloned().unwrap_or(Value::Null) }
fn labels(os: &Value) -> Vec<String> { os.as_array().map(|a| a.iter().map(|o| o.get("label").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| o.to_string())).collect()).unwrap_or_default() }
fn described(os: &Value) -> Vec<String> { os.as_array().map(|a| a.iter().filter(|o| !sv(o, "description").is_empty()).map(|o| format!("{}: {}", sv(o, "label"), sv(o, "description"))).collect()).unwrap_or_default() }
pub fn perm_many(input: &Value) -> bool { input.get("questions").and_then(Value::as_array).map_or(0, Vec::len) > 1 }
pub fn perm_question(input: &Value) -> String { let q = first_q(input); let s = sv(&q, "question"); if s.is_empty() { sv(input, "question").to_string() } else { s.to_string() } }
pub fn perm_card(input: &Value) -> String {
    if perm_many(input) {
        let qs = input["questions"].as_array().unwrap();
        let lines: Vec<String> = qs.iter().enumerate().map(|(i, q)| { let os = q.get("options").cloned().unwrap_or(Value::Null); let ls = labels(&os); let ds = described(&os);
            format!("{}. {}{}{}", i + 1, sv(q, "question"), if ls.is_empty() { String::new() } else { format!(" ({})", ls.join(" / ")) }, if ds.is_empty() { String::new() } else { format!("\n   {}", ds.join("; ")) }) }).collect();
        format!("{}\nType your answers below, then answer with them.", lines.join("\n"))
    } else { let ds = described(&first_q(input).get("options").cloned().unwrap_or(Value::Null)); let q = perm_question(input); if ds.is_empty() { q } else { format!("{q}\n{}", ds.join("\n")) } }
}
pub fn perm_options(input: &Value) -> String { if perm_many(input) { String::new() } else { labels(&first_q(input).get("options").cloned().unwrap_or(Value::Null)).join("\n") } }
/// Perm.answers: the input with an "answers" object put first, every question keyed by its text.
pub fn perm_answers(input: &Value, answer: &str) -> Value {
    let mut answers = serde_json::Map::new();
    if perm_many(input) { for q in input["questions"].as_array().unwrap() { answers.insert(sv(q, "question").to_string(), json!(answer)); } } else { answers.insert(perm_question(input), json!(answer)); }
    let mut o = serde_json::Map::new(); o.insert("answers".into(), Value::Object(answers));
    if let Some(m) = input.as_object() { for (k, v) in m { if k != "answers" { o.insert(k.clone(), v.clone()); } } } else { return input.clone() }
    Value::Object(o)
}
/// Perm.verdict.k
pub fn perm_verdict(tool: &str, input: &Value, answer: &str) -> Value {
    if tool == "AskUserQuestion" { if answer.is_empty() { perm_deny("The user dismissed the question.") } else { perm_allow(&perm_answers(input, answer)) } }
    else if answer == "accept" || answer == "session" { perm_allow(input) } else { perm_deny("The user declined this tool call.") }
}
/// Ans.line: what the person decided, kept in the thread.
pub fn ans_line(kind: &str, tool: &str, q: &str, answer: &str) -> String {
    if kind == "input" { let first = q.lines().next().unwrap_or(q).chars().take(200).collect::<String>(); format!("{} → {}", first.trim(), if answer.trim().is_empty() { "dismissed" } else { answer.trim() }) }
    else { format!("{}: {tool} {}", match answer { "accept" => "Allowed", "session" => "Allowed for this session", _ => "Denied" }, q.chars().take(160).collect::<String>()) }
}

pub struct Parked { pub thread: String, pub tool: String, pub input: Value, pub id: Value, pub tx: oneshot::Sender<String> }

pub struct Mcp { pub port: u16, secret: String, ag: Weak<Agents>, tools: Vec<Value>, sem: Arc<Semaphore> }

impl Mcp {
    pub async fn start(ag: &Arc<Agents>) -> anyhow::Result<Arc<Mcp>> {
        let l = TcpListener::bind("127.0.0.1:0").await?; let port = l.local_addr()?.port();
        let mut b = [0u8; 32]; std::fs::File::open("/dev/urandom")?.read_exact_sync(&mut b)?;
        let all: Value = serde_json::from_str(TOOLS_JSON)?;
        let tools: Vec<Value> = all["result"]["tools"].as_array().cloned().unwrap_or_default().into_iter().filter(|t| IMPLEMENTED.contains(&sv(t, "name"))).collect();
        let m = Arc::new(Mcp { port, secret: hex::encode(b), ag: Arc::downgrade(ag), tools, sem: Arc::new(Semaphore::new(MAX_CONNS)) });
        let mm = m.clone();
        tokio::spawn(async move { loop { let Ok((s, _)) = l.accept().await else { continue }; let Ok(p) = mm.sem.clone().try_acquire_owned() else { continue }; let m2 = mm.clone(); tokio::spawn(async move { let _ = m2.conn(s).await; drop(p) }); } });
        Ok(m)
    }
    pub fn seg(thread: &str, gen: &str) -> String { if gen.is_empty() { thread.to_string() } else { format!("{thread}.{gen}") } }
    /// Hub.mcp_url.g
    pub fn url(&self, thread: &str, gen: &str) -> String { let seg = Mcp::seg(thread, gen); format!("http://127.0.0.1:{}/mcp/{seg}?k={}", self.port, mcp_key(&self.secret, &seg)) }

    async fn conn(self: Arc<Self>, mut s: TcpStream) -> anyhow::Result<()> {
        let mut buf: Vec<u8> = Vec::new(); let mut tmp = [0u8; 8192];
        let (head_len, method, target, clen) = loop {
            let n = tokio::time::timeout(Duration::from_secs(30), s.read(&mut tmp)).await??; if n == 0 { return Ok(()) } buf.extend_from_slice(&tmp[..n]);
            if buf.len() > 16384 && !buf.windows(4).any(|w| w == b"\r\n\r\n") { return Ok(()) }
            let mut hs = [httparse::EMPTY_HEADER; 32]; let mut r = httparse::Request::new(&mut hs);
            match r.parse(&buf) { Ok(httparse::Status::Complete(l)) => { let cl = r.headers.iter().find(|h| h.name.eq_ignore_ascii_case("content-length")).and_then(|h| std::str::from_utf8(h.value).ok()).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
                break (l, r.method.unwrap_or("").to_string(), r.path.unwrap_or("").to_string(), cl) } Ok(_) => continue, Err(_) => return Ok(()) }
        };
        let reply = |status: &str, body: &str| format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n{body}", body.len());
        if clen > MAX_BODY { s.write_all(reply("413 Payload Too Large", "").as_bytes()).await?; return Ok(()) }
        while buf.len() < head_len + clen { let n = tokio::time::timeout(Duration::from_secs(30), s.read(&mut tmp)).await??; if n == 0 { return Ok(()) } buf.extend_from_slice(&tmp[..n]) }
        let body = String::from_utf8_lossy(&buf[head_len..head_len + clen]).into_owned();
        let (path, query) = target.split_once('?').unwrap_or((&target, ""));
        let seg = path.strip_prefix("/mcp/").unwrap_or("");
        let k = query.split('&').find_map(|p| p.strip_prefix("k=")).unwrap_or("");
        if method != "POST" || seg.is_empty() || !same(k, &mcp_key(&self.secret, seg)) { s.write_all(reply("401 Unauthorized", "").as_bytes()).await?; return Ok(()) }
        let (thread, gen) = seg.split_once('.').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((seg.to_string(), String::new()));
        let out = self.handle(&thread, &gen, &body, &mut s).await;
        match out { Some(v) => s.write_all(reply("200 OK", &v.to_string()).as_bytes()).await?, None => s.write_all(reply("202 Accepted", "").as_bytes()).await? }
        s.shutdown().await.ok(); Ok(())
    }

    /// Mcp.text: one JSON-RPC message from an agent of `thread`; None: a notification (no reply).
    pub async fn handle(&self, thread: &str, gen: &str, body: &str, sock: &mut TcpStream) -> Option<Value> {
        let Ok(o) = serde_json::from_str::<Value>(body) else { return Some(error(&Value::Null, -32700, "parse error")) };
        let id = o.get("id").cloned().unwrap_or(Value::Null); let p = o.get("params").cloned().unwrap_or(Value::Null);
        let ag = self.ag.upgrade()?;
        match sv(&o, "method") {
            "initialize" => { let pv = if sv(&p, "protocolVersion").is_empty() { "2025-06-18" } else { sv(&p, "protocolVersion") }; Some(result(&id, json!({"protocolVersion":pv,"capabilities":{"tools":{}},"serverInfo":{"name":"backplane","version":ag.hub.cfg.version}}))) }
            "tools/list" => { let mut tools = self.tools.clone(); for s in ag.tool_sets() { tools.extend(s.list(thread)) } Some(result(&id, json!({"tools":tools}))) }
            "tools/call" => self.call(&ag, thread, gen, &id, &p, sock).await,
            "ping" => Some(result(&id, json!({}))),
            _ => None, // notifications and anything unknown: no reply (Mcp.method.pos)
        }
    }

    async fn call(&self, ag: &Arc<Agents>, thread: &str, gen: &str, id: &Value, p: &Value, sock: &mut TcpStream) -> Option<Value> {
        let name = sv(p, "name"); let a = p.get("arguments").cloned().unwrap_or(Value::Null);
        const READS: [&str; 5] = ["orchestrator_capabilities", "thread_list", "thread_read", "thread_wait", "thread_wait_now"];
        if !gen.is_empty() && ag.gen_of(thread).map_or(true, |g| g != gen) && !READS.contains(&name) { // Gen.refused
            return Some(result(id, content(false, &json!({"error":"stale process: this agent process is no longer the thread's, so it may read but not change anything"})))) }
        // Rules.gate: an enforced standing rule refuses the call (an error result; for the permission prompt, a deny) unless the person's latest message says its word
        { let tool = if name == "permission_prompt" { sv(&a, "tool_name") } else { name };
          let why = ag.rule_refusal(thread, tool).await;
          if !why.is_empty() { return Some(if name == "permission_prompt" { result(id, perm_text(&perm_deny(&why))) } else { result(id, shaped(false, &json!(why))) }) } }
        if name == "permission_prompt" {
            if let Some(m) = ag.mailroom() { if let Some(v) = m.permit(thread, sv(&a, "tool_name"), a.get("input").unwrap_or(&Value::Null)).await { return Some(result(id, perm_text(&v))) } } // Perm.run: a mail run's charter decides, no ask
            return self.permit(ag, thread, id, &a, sock).await
        }
        if let Some(m) = ag.mailroom() { if let Some((ok, body)) = m.gate(thread, name, &a).await { return Some(result(id, shaped(ok, &body))) } } // Hub.mcp.charter
        let (ok, body) = match name {
            "orchestrator_capabilities" => ag.tool_caps(thread).await,
            "thread_list" => ag.tool_list(thread).await,
            "thread_read" => ag.tool_read(thread, &a).await,
            "review_comment" => ag.tool_review_comment(thread, &a).await,
            "review_comments" => ag.tool_review_comments(thread).await,
            "work_run" => ag.tool_work_run(thread, &a).await,
            "work_watch" => ag.tool_work_watch(thread, &a).await,
            "work_list" => ag.tool_work_list(thread).await,
            "work_cancel" => ag.tool_work_cancel(thread, &a).await,
            "work_unwatch" => ag.tool_work_unwatch(thread, &a).await,
            "thread_launch" => ag.tool_launch(thread, &a).await,
            "delegate_task" => ag.tool_delegate(thread, &a).await,
            "task_status" => ag.tool_task_status(thread, &a).await,
            "task_cancel" => ag.tool_task_cancel(thread, &a).await,
            "thread_send" => ag.tool_send(thread, &a).await,
            "thread_update" => ag.tool_update(thread, &a).await,
            "thread_wait" => ag.tool_wait(thread, &a).await,
            "thread_interrupt" => ag.tool_interrupt(thread, &a).await,
            "thread_settle" => ag.tool_settle(thread, &a).await,
            "kicad_ipc" => match crate::files::kicad::ipc::kicad_ipc_in(&ag.hub, Some(thread), sv(&a, "path"), sv(&a, "type"), a.get("message").cloned().unwrap_or(json!({}))).await { Ok(v) => (true, v), Err(e) => fail(&e) },
            "kicad_ipc_status" => match crate::files::kicad::ipc::kicad_ipc_status_in(&ag.hub, Some(thread), sv(&a, "path")).await { Ok(v) => (true, v), Err(e) => fail(&e) },
            "kicad_export10" => crate::files::export10::mcp(&ag.hub, sv(&a, "path"), a.get("force").and_then(Value::as_bool).unwrap_or(false)).await,
            "settings_inspect" => crate::settings::proposal::tool_inspect(&ag.hub, &ag.host.models).await,
            "settings_propose" => { let now = now_s(); let id = hub_id("settings", now, ag.seq_n()); crate::settings::proposal::tool_propose(ag, thread, &ag.host.models, &a, now, &id).await }
            "secret_request" => crate::settings::secret::tool_request(&ag.hub, thread, &a).await,
            "secret_list" => crate::settings::secret::tool_list(),
            "secret_run" => { let cwd = ag.world().await.cwd(thread); crate::settings::secret::tool_run(thread, &cwd, &a).await }
            n if crate::term::tools::NAMES.contains(&n) => crate::term::tools::call(&ag.hub, thread, n, &a).await.unwrap_or_else(|| fail("unknown tool")),
            "viewer_show" => {
                // the answer carries what the viewer then shows (kind, source, part, sheet, layer, selection) as the client reports it; a client that does not answer in 2 s leaves the last report, marked
                let before = crate::files::kicad::watch::viewer_state(thread).0;
                ag.hub.push_all(&json!({"t":"view","thread":thread,"path":sv(&a, "path"),"kind":sv(&a, "kind")}));
                let mut now = crate::files::kicad::watch::viewer_state(thread);
                for _ in 0..40 { if now.0 > before { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await; now = crate::files::kicad::watch::viewer_state(thread); }
                (true, json!({"shown":sv(&a, "path"),"viewer":now.1,"settled":now.0 > before}))
            }
            n if ag.tool_sets().iter().any(|s| s.owns(n)) => { let s = ag.tool_sets().into_iter().find(|s| s.owns(n)).unwrap(); s.call(thread, n, &a).await }
            _ => fail("unknown tool"),
        };
        let mut res = result(id, shaped(ok, &body));
        if let Some(block) = ag.ride_take(thread).await { if let Some(a) = res.pointer_mut("/result/content").and_then(Value::as_array_mut) { a.push(json!({"type":"text","text":block})) } }
        Some(res)
    }

    /// Mcp.permit: Claude asks before a tool runs outside full access. The request becomes an ask the person answers and the reply waits.
    async fn permit(&self, ag: &Arc<Agents>, thread: &str, id: &Value, a: &Value, sock: &mut TcpStream) -> Option<Value> {
        let tool = sv(a, "tool_name").to_string(); let input = a.get("input").cloned().unwrap_or(Value::Null);
        let now = now_s(); let ask = hub_id("q", now, ag.seq_n());
        let (kind, detail, options) = match tool.as_str() { "AskUserQuestion" => ("input", perm_card(&input), perm_options(&input)), "ExitPlanMode" => ("plan", sv(&input, "plan").to_string(), String::new()), _ => ("approval", perm_detail(&input), String::new()) };
        if kind == "approval" && ag.world().await.sessions.contains(&(thread.to_string(), tool.clone())) { // Perm.session_ok: allowed for this session
            return Some(result(id, perm_text(&perm_allow(&input)))) }
        let ev = json!({"$":"AskOpened","id":ask,"thread":thread,"kind":kind,"tool":tool,"detail":detail,"options":options,"at":now});
        let pend = ag.pending_ev(thread, 1, "", now).await;
        if kind == "plan" { ag.commit_events(vec![ev, pend]).await; return Some(result(id, perm_text(&perm_deny(PLAN_NOTE)))) }
        let (tx, mut rx) = oneshot::channel();
        { let mut m = ag.parked.lock().unwrap(); if m.values().filter(|x| x.thread == thread).count() >= MAX_PARKED_PER_THREAD { return Some(result(id, content(false, &json!({"error":"too many questions are waiting for the person"})))) }
          m.insert(ask.clone(), Parked { thread: thread.into(), tool: tool.clone(), input: input.clone(), id: id.clone(), tx }); }
        ag.commit_events(vec![ev, pend]).await;
        // wait for the answer, or for the agent to hang up (its reply is then lost; the ask stays open for the person)
        let mut probe = [0u8; 1];
        let answer = loop { tokio::select! { a = &mut rx => break a.ok(), r = sock.read(&mut probe) => { if matches!(r, Ok(0) | Err(_)) { ag.parked.lock().unwrap().remove(&ask); return None } } } };
        let answer = answer?; Some(result(id, perm_text(&perm_verdict(&tool, &input, &answer))))
    }
}
trait ReadSync { fn read_exact_sync(&mut self, b: &mut [u8]) -> std::io::Result<()>; }
impl ReadSync for std::fs::File { fn read_exact_sync(&mut self, b: &mut [u8]) -> std::io::Result<()> { std::io::Read::read_exact(self, b) } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn hmac_sha256_matches_the_rfc_4231_vectors() {
        assert_eq!(hex::encode(hmac_sha256(&[0x0b; 20], b"Hi There")), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
        assert_eq!(hex::encode(hmac_sha256(b"Jefe", b"what do ya want for nothing?")), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        assert_eq!(hex::encode(hmac_sha256(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First")), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
    }
    #[test] fn the_url_key_belongs_to_one_thread_and_generation() { let a = mcp_key("s", "t1.g1"); assert_eq!(a.len(), 64); assert_ne!(a, mcp_key("s", "t1.g2")); assert_ne!(a, mcp_key("t", "t1.g1")); assert!(same(&a, &mcp_key("s", "t1.g1")) && !same(&a, &a[..63])); }
    #[test] fn verdicts_follow_the_answer() {
        let input = json!({"command":"rm x"});
        assert_eq!(perm_verdict("Bash", &input, "accept"), perm_allow(&input)); assert_eq!(perm_verdict("Bash", &input, "session"), perm_allow(&input)); assert_eq!(perm_verdict("Bash", &input, "decline"), perm_deny("The user declined this tool call."));
        let q = json!({"questions":[{"question":"Which port?","options":[{"label":"USB-C","description":"reversible"},{"label":"Micro"}]}]});
        assert_eq!(perm_verdict("AskUserQuestion", &q, ""), perm_deny("The user dismissed the question."));
        let v = perm_verdict("AskUserQuestion", &q, "USB-C"); assert_eq!(v["updatedInput"]["answers"]["Which port?"], "USB-C"); assert_eq!(v["updatedInput"].as_object().unwrap().keys().next().unwrap(), "answers", "answers go first");
    }
    #[test] fn ask_cards_say_the_question_and_describe_each_option() {
        let q = json!({"questions":[{"question":"Which port?","options":[{"label":"USB-C","description":"reversible"},{"label":"Micro"}]}]});
        assert_eq!(perm_card(&q), "Which port?\nUSB-C: reversible"); assert_eq!(perm_options(&q), "USB-C\nMicro");
        let many = json!({"questions":[{"question":"A?","options":[{"label":"x"},{"label":"y"}]},{"question":"B?"}]});
        assert_eq!(perm_card(&many), "1. A? (x / y)\n2. B?\nType your answers below, then answer with them."); assert_eq!(perm_options(&many), "");
        assert_eq!(perm_answers(&many, "ok")["answers"], json!({"A?":"ok","B?":"ok"}));
    }
    #[test] fn approval_details_prefer_the_command() { assert_eq!(perm_detail(&json!({"file_path":"/a","command":"ls"})), "ls"); assert_eq!(perm_detail(&json!({"url":"u"})), "u"); assert_eq!(perm_detail(&json!({"x":1})), "{\"x\":1}"); }
    #[test] fn decisions_stay_in_the_thread() {
        assert_eq!(ans_line("approval", "Bash", "ls -la", "accept"), "Allowed: Bash ls -la"); assert_eq!(ans_line("approval", "Bash", "ls", "session"), "Allowed for this session: Bash ls"); assert_eq!(ans_line("approval", "Bash", "ls", "decline"), "Denied: Bash ls");
        assert_eq!(ans_line("input", "AskUserQuestion", "Which?\nUSB-C: x", " USB-C "), "Which? → USB-C"); assert_eq!(ans_line("input", "t", "Q", " "), "Q → dismissed");
    }
    #[test] fn the_tool_list_is_the_js_hubs_for_every_tool_we_serve() {
        let all: Value = serde_json::from_str(TOOLS_JSON).unwrap(); let names: Vec<&str> = all["result"]["tools"].as_array().unwrap().iter().map(|t| sv(t, "name")).collect();
        for n in IMPLEMENTED { assert!(names.contains(n), "{n} is not a tool the JS hub lists") }
    }
}

#[cfg(test)] mod shape_tests {
    use super::*;
    #[test] fn a_string_body_is_the_raw_text_and_a_content_body_keeps_its_blocks() {
        assert_eq!(shaped(true, &json!("line1\nline2"))["content"][0]["text"], "line1\nline2");
        let r = shaped(true, &json!({"$content":[{"type":"text","text":"hi"},{"type":"image","data":"AAA","mimeType":"image/png"}]}));
        assert_eq!(r["content"][0]["text"], "hi"); assert_eq!(r["content"][1]["data"], "AAA"); assert_eq!(r["isError"], false);
        assert_eq!(shaped(false, &json!({"error":"x"}))["content"][0]["text"], "{\"error\":\"x\"}");
    }
}

//! The agents' MCP tools: mcp_list, mcp_add, mcp_update, mcp_remove, mcp_test and mcp_import, on the same list and the same per-provider
//! config as Settings > MCP. A server runs code on this machine, so adding one, turning one on, or changing what it runs waits for the
//! person's one-tap OK (an approval ask, tool `mcp_server`) unless the thread's project allows its agents to (`mcp.agents.<project>`).
//! A key never comes in plain text: a value that looks like one is refused, and the agent is told to `secret_request` it and pass
//! `{"secret": name}`. What is added reaches agents from their next turn.
use super::{model::{secretish, Kv, Server}, parse, svc::{self, Mcpx}};
use crate::{agents::{mcp::ToolSet, Agents}, api::Fut};
use serde_json::{json, Value};
use std::sync::Arc;

pub const TOOL: &str = "mcp_server";
pub const NAMES: &[&str] = &["mcp_list", "mcp_add", "mcp_update", "mcp_remove", "mcp_test", "mcp_import"];
pub struct McpTools;

fn schema(name: &str, desc: &str, props: Value, req: &[&str]) -> Value { json!({"name":name,"description":desc,"inputSchema":{"type":"object","properties":props,"required":req}}) }
fn st(d: &str) -> Value { json!({"type":"string","description":d}) }
fn arr(d: &str) -> Value { json!({"type":"array","items":{"type":"string"},"description":d}) }
fn kvmap(d: &str) -> Value { json!({"type":"object","description":d,"additionalProperties":{"anyOf":[{"type":"string"},{"type":"object","properties":{"secret":{"type":"string"}},"required":["secret"]}]}}) }
fn fail(m: &str) -> (bool, Value) { (false, json!({"error": m})) }

fn defs() -> Vec<Value> {
    let scope = |p: &mut serde_json::Map<String, Value>| { p.insert("providers".into(), arr("providers that get it (claude, codex, antigravity, grok, opencode, muse); empty or absent: all"));
        p.insert("projects".into(), arr("project ids whose threads get it; empty: all")); p.insert("bots".into(), json!({"description":"bot ids that get it; absent: every bot; []: none","type":["array","null"],"items":{"type":"string"}})); };
    let mut add = json!({"name":st("what agents call it (mcp__<name>__tool): lower case, digits, - and _"),"transport":st("stdio, http or sse"),"command":st("stdio: the program"),"args":arr("stdio: its arguments"),
        "env":kvmap("stdio: environment. A key or token is {\"secret\": \"vault-name\"} (secret_request it first), never its text"),"cwd":st("stdio: where it starts"),"url":st("http/sse: the URL"),
        "headers":kvmap("http/sse: headers; a key or token is {\"secret\": \"vault-name\"}"),"snippet":st("instead of the fields: an mcpServers JSON, a Codex [mcp_servers] TOML table, a `claude mcp add` line or a command, as vendor docs give it"),
        "gallery":st("instead: a Settings > MCP gallery id (fetch, git, time, memory, everything, filesystem, deepwiki, context7, github, microchip-docs, microchip-products, esp-components, stm32-docs, kicad-pro)"),
        "gallery_arg":st("what a gallery entry needs (filesystem: the folder)"),"enabled":{"type":"boolean","description":"default true"},"disabled_tools":arr("tools agents are not given")});
    scope(add.as_object_mut().unwrap());
    let mut upd = add.clone(); upd["id"] = st("the server's id (mcp_list)"); for k in ["snippet", "gallery", "gallery_arg"] { upd.as_object_mut().unwrap().remove(k); }
    vec![
        schema("mcp_list", "The MCP servers Backplane gives agents (Settings > MCP): each one's id, transport, command or URL, scope, status, tool count and last error. Built in: backplane and pcbparts.", json!({}), &[]),
        schema("mcp_add", "Add MCP servers for every agent Backplane runs (Claude, Codex, Antigravity, Grok, ...): give the fields, or a snippet, or a gallery id. Unless this project allows agents to, the person confirms it with one tap (a server runs code); end your turn and a message says when it is added. Secrets: secret_request a name, then pass {\"secret\": name}. Agents get it from their next turn.", add, &[]),
        schema("mcp_update", "Change a server: turn it on or off, its scope, its turned-off tools, or what it runs. Turning one on or changing what it runs asks the person, as mcp_add does.", upd, &["id"]),
        schema("mcp_remove", "Remove a server from every agent.", json!({"id":st("the server's id")}), &["id"]),
        schema("mcp_test", "Start a server as an agent would and list its tools, resources and prompts; its error and stderr when it fails.", json!({"id":st("the server's id")}), &["id"]),
        schema("mcp_import", "The MCP servers set up in the person's desktop tools (Claude Code, Codex, Cursor, VS Code, Claude Desktop, Windsurf, Gemini CLI, OpenCode). Without picks: the list, each with a pick key. With picks: import those (the person confirms, as mcp_add).", json!({"picks":arr("pick keys from the list"),"sync":{"type":"boolean","description":"keep them in step with their files (default true)"}}), &[]),
    ]
}

impl ToolSet for McpTools {
    fn list(&self, _thread: &str) -> Vec<Value> { defs() }
    fn owns(&self, name: &str) -> bool { NAMES.contains(&name) }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> Fut<'a, (bool, Value)> {
        Box::pin(async move {
            let Some(m) = svc::get() else { return fail("MCP is not running on this hub") };
            match name {
                "mcp_list" => list(&m, thread).await,
                "mcp_add" => add(&m, thread, a).await,
                "mcp_update" => update(&m, thread, a).await,
                "mcp_remove" => { let id = a["id"].as_str().unwrap_or(""); match m.change(|l| { let i = l.servers.iter().position(|s| s.id == id).ok_or(format!("no server {id}"))?; l.servers.remove(i); Ok(()) }).await { Ok(()) => (true, json!({"removed": id, "note": "Agents no longer get it from their next turn."})), Err(e) => fail(&e) } }
                "mcp_test" => match m.test(a["id"].as_str().unwrap_or("")).await { Ok(r) => (r.ok, json!(r)), Err(e) => fail(&e) },
                "mcp_import" => import(&m, thread, a).await,
                _ => fail("unknown tool"),
            }
        })
    }
}

async fn list(m: &Arc<Mcpx>, thread: &str) -> (bool, Value) {
    let info = m.info().await; let machine = info["machine"].as_str().unwrap_or("").to_string();
    let (p, pr, b) = m.thread_ctx(thread).await;
    let servers: Vec<Value> = info["servers"].as_array().into_iter().flatten().map(|x| {
        let s: Server = serde_json::from_value(x["server"].clone()).unwrap_or_default(); let mut o = s.public();
        o["status"] = json!(super::page::status(x, &machine).0); o["this_thread_gets_it"] = json!(s.applies(&p, &pr, &b));
        if let Some(r) = x["st"]["report"].as_object() { o["tools"] = json!(r.get("tools").and_then(Value::as_array).map(|a| a.iter().filter_map(|t| t["name"].as_str()).collect::<Vec<_>>()).unwrap_or_default()); if r.get("ok") != Some(&json!(true)) { o["error"] = r.get("error").cloned().unwrap_or(Value::Null) } }
        let missing: Vec<&str> = x["secrets"].as_array().into_iter().flatten().filter(|k| k["set"] == false).filter_map(|k| k["name"].as_str()).collect(); if !missing.is_empty() { o["secrets_missing"] = json!(missing) }
        o }).collect();
    let allowed = m.hub.setting(&svc::allow_key(&pr)).await.as_deref() == Some("allow");
    (true, json!({"builtin": [{"id":"backplane","always":true},{"id":"pcbparts","enabled":info["pcbparts"]}], "servers": servers, "adding_asks_the_person": !allowed, "machine": machine}))
}

/// A value an agent gives for an env variable or header: a vault reference, or text (refused when it looks like a key).
fn kv_of(k: &str, v: &Value) -> Result<Kv, String> {
    match v {
        Value::Object(o) => { let n = o.get("secret").and_then(Value::as_str).ok_or(format!("{k}: an object value is {{\"secret\": name}}"))?; let pre = o.get("prefix").and_then(Value::as_str).unwrap_or("");
            if !crate::settings::vault::valid_name(n) { return Err(format!("{k}: {n:?} is not a vault name")) } Ok(Kv::sec(k, n, pre)) }
        Value::String(s) => {
            let looks = secretish(k) && !super::model::placeholder(s) && s.len() >= 8;
            if looks { return Err(format!("{k} looks like a key: never put one in plain text. secret_request {{name: \"{}\", purpose}} and pass {{\"secret\": \"{}\"}}", super::model::vault_name("x", k).trim_start_matches("mcp-x-"), super::model::vault_name("x", k).trim_start_matches("mcp-x-"))) }
            if s.starts_with("${") && secretish(k) { return Ok(Kv::sec(k, &super::model::vault_name("x", k), "")) } // fixed up with the id below
            Ok(Kv::lit(k, s))
        }
        x => Ok(Kv::lit(k, &x.to_string())),
    }
}
fn strs(v: &Value) -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default() }
fn apply_scope(s: &mut Server, a: &Value) {
    if a.get("providers").is_some() { s.providers = strs(&a["providers"]) }
    if a.get("projects").is_some() { s.projects = strs(&a["projects"]) }
    if let Some(b) = a.get("bots") { s.bots = if b.is_null() { None } else { Some(strs(b)) } }
    if a.get("disabled_tools").is_some() { s.disabled_tools = strs(&a["disabled_tools"]) }
    if let Some(e) = a.get("enabled").and_then(Value::as_bool) { s.enabled = e }
}
/// The servers an mcp_add asks for.
pub fn wanted(m: &Mcpx, thread: &str, a: &Value) -> Result<Vec<Server>, String> {
    let taken = m.list().ids();
    let mut out = if let Some(g) = a["gallery"].as_str() { vec![m.gallery_server(g, a["gallery_arg"].as_str().unwrap_or(""))?] }
    else if let Some(t) = a["snippet"].as_str() {
        let f = parse::paste(t, &taken)?;
        if !f.plain.is_empty() { let ks: Vec<String> = f.servers.iter().flat_map(|s| s.env.iter().chain(s.headers.iter()).filter(|k| k.secret && f.plain.iter().any(|(n, _)| *n == k.v)).map(|k| k.k.clone())).collect();
            return Err(format!("The snippet has a key in it ({}): never pass one in plain text. Replace it with a placeholder like ${{{}}}, then secret_request it", ks.join(", "), ks.first().cloned().unwrap_or_default())) }
        f.servers
    } else {
        let name = a["name"].as_str().unwrap_or("").to_string();
        let transport = a["transport"].as_str().map(str::to_string).unwrap_or_else(|| if a["url"].is_string() { if a["url"].as_str().unwrap_or("").trim_end_matches('/').ends_with("/sse") { "sse".into() } else { "http".into() } } else { "stdio".into() });
        let id = super::model::unique(&super::model::slug(if name.is_empty() { a["command"].as_str().or(a["url"].as_str()).unwrap_or("server") } else { &name }), &taken);
        let fix = |mut k: Kv| { if k.secret && k.v.starts_with("mcp-x-") { k.v = super::model::vault_name(&id, &k.k) } k };
        let kvs = |v: &Value| -> Result<Vec<Kv>, String> { v.as_object().map(|o| o.iter().map(|(k, x)| kv_of(k, x).map(fix)).collect()).unwrap_or(Ok(vec![])) };
        vec![Server { id: id.clone(), name: if name.is_empty() || super::model::slug(&name) == id { String::new() } else { name }, transport, command: a["command"].as_str().unwrap_or("").into(), args: strs(&a["args"]),
            env: kvs(&a["env"])?, cwd: a["cwd"].as_str().unwrap_or("").into(), url: a["url"].as_str().unwrap_or("").into(), headers: kvs(&a["headers"])?, enabled: true, ..Default::default() }]
    };
    for s in out.iter_mut() { apply_scope(s, a); s.source = format!("agent:{thread}"); s.added_at = svc::now_s(); if let Some(p) = s.problem() { return Err(format!("{}: {p}", s.id)) } }
    Ok(out)
}

/// What the person reads on the confirm card.
pub fn describe(op: &str, servers: &[Server]) -> String {
    let mut t = format!("{} {} for agents?", match op { "on" => "Turn on", "change" => "Change", _ => "Add" }, if servers.len() == 1 { "this MCP server".to_string() } else { format!("{} MCP servers", servers.len()) });
    for s in servers {
        t.push_str(&format!("\n\n{} ({})\n{}", s.id, if s.remote() { s.transport.to_uppercase() } else { "runs on this machine".into() }, s.line()));
        let sec = s.secrets(); if !sec.is_empty() { t.push_str(&format!("\nSecrets from the vault: {}", sec.join(", "))) }
        let plain: Vec<String> = s.env.iter().chain(s.headers.iter()).filter(|k| !k.secret).map(|k| format!("{}={}", k.k, k.v)).collect(); if !plain.is_empty() { t.push_str(&format!("\n{}", plain.join(" "))) }
        if !s.providers.is_empty() { t.push_str(&format!("\nFor: {}", s.providers.join(", "))) }
    }
    t.push_str("\n\nA stdio server runs as you, with your files. Allow only what you trust.");
    t
}

/// Applies an op now (allowed, or the person said yes). The text the agent is told.
pub async fn apply(m: &Arc<Mcpx>, op: &str, servers: Vec<Server>) -> Result<String, String> {
    let ids: Vec<String> = servers.iter().map(|s| s.id.clone()).collect();
    match op {
        "add" => { let ids = m.add(servers, &[], "agent").await?; Ok(format!("Added MCP server{} {}. Agents get {} from their next turn; it is being tested now (mcp_test, mcp_list).", if ids.len() == 1 { "" } else { "s" }, ids.join(", "), if ids.len() == 1 { "it" } else { "them" })) }
        _ => { m.change(|l| { for s in servers { if let Some(x) = l.servers.iter_mut().find(|x| x.id == s.id) { *x = s } else { return Err(format!("no server {}", s.id)) } } Ok(()) }).await?; for id in &ids { m.test_bg(id) } Ok(format!("Changed {}. Agents get the change from their next turn.", ids.join(", "))) }
    }
}

/// Applies the op when the thread's project allows it, else opens the person's confirm.
async fn gate(m: &Arc<Mcpx>, thread: &str, op: &str, servers: Vec<Server>) -> (bool, Value) {
    let (_, project, _) = m.thread_ctx(thread).await;
    let allowed = !project.is_empty() && m.hub.setting(&svc::allow_key(&project)).await.as_deref() == Some("allow");
    let missing: Vec<String> = servers.iter().flat_map(|s| s.missing(&|n| crate::settings::vault::person_get(n).is_some())).collect();
    let need = if missing.is_empty() { Value::Null } else { json!({"secrets_missing": missing, "note": "secret_request each of these names; the server needs them to start"}) };
    if allowed { return match apply(m, op, servers).await { Ok(t) => (true, json!({"status": "done", "note": t, "secrets": need})), Err(e) => fail(&e) } }
    let Some(ag) = m.agents() else { return fail("no agents here") };
    let now = svc::now_s(); let ask = crate::agents::lines::hub_id("mcp", now, ag.seq_n());
    let ev = json!({"$":"AskOpened","id":ask,"thread":thread,"kind":"approval","tool":TOOL,"detail":describe(op, &servers),"options":json!({"op": op, "servers": servers}).to_string(),"at":now});
    let pend = ag.pending_ev(thread, 1, "", now).await;
    if ag.hub.commit_many(&[ev, pend]).await.is_err() { return fail("could not ask the person") }
    (true, json!({"status": "awaiting person", "ask": ask, "note": "The person confirms this with one tap. End your turn: a message says when it is done; agents get it from their next turn.", "secrets": need}))
}

async fn add(m: &Arc<Mcpx>, thread: &str, a: &Value) -> (bool, Value) {
    match wanted(m, thread, a) { Ok(s) => gate(m, thread, "add", s).await, Err(e) => fail(&e) }
}

async fn update(m: &Arc<Mcpx>, thread: &str, a: &Value) -> (bool, Value) {
    let id = a["id"].as_str().unwrap_or("");
    let Some(old) = m.list().get(id).cloned() else { return fail(&format!("no server {id} (mcp_list)")) };
    let mut s = old.clone(); apply_scope(&mut s, a);
    if let Some(n) = a["name"].as_str() { s.name = n.into() }
    let kvs = |v: &Value| -> Result<Vec<Kv>, String> { v.as_object().map(|o| o.iter().map(|(k, x)| kv_of(k, x).map(|mut kv| { if kv.secret && kv.v.starts_with("mcp-x-") { kv.v = super::model::vault_name(id, k) } kv })).collect()).unwrap_or(Ok(vec![])) };
    if let Some(c) = a["command"].as_str() { s.command = c.into() } if a.get("args").is_some() { s.args = strs(&a["args"]) } if let Some(c) = a["cwd"].as_str() { s.cwd = c.into() } if let Some(u) = a["url"].as_str() { s.url = u.into() }
    if let Some(t) = a["transport"].as_str() { s.transport = t.into() }
    if a.get("env").is_some() { match kvs(&a["env"]) { Ok(e) => s.env = e, Err(e) => return fail(&e) } }
    if a.get("headers").is_some() { match kvs(&a["headers"]) { Ok(h) => s.headers = h, Err(e) => return fail(&e) } }
    if let Some(p) = s.problem() { return fail(&p) }
    let runs_new = s.command != old.command || s.args != old.args || s.env != old.env || s.cwd != old.cwd || s.url != old.url || s.headers != old.headers || s.transport != old.transport;
    let turns_on = s.enabled && !old.enabled;
    if runs_new || turns_on { return gate(m, thread, if runs_new { "change" } else { "on" }, vec![s]).await }
    match apply(m, "change", vec![s]).await { Ok(t) => (true, json!({"status": "done", "note": t})), Err(e) => fail(&e) }
}

async fn import(m: &Arc<Mcpx>, thread: &str, a: &Value) -> (bool, Value) {
    let picks = strs(&a["picks"]);
    let sc = m.scan_imports().await;
    if picks.is_empty() {
        let v: Vec<Value> = sc.iter().flat_map(|(s, here)| s.found.servers.iter().enumerate().map(move |(i, x)| { let mut o = x.public(); o["pick"] = json!(format!("{}|{i}", s.path)); o["tool"] = json!(s.tool); o["already_here_as"] = json!(here[i]); o })).collect();
        return (true, json!({"servers": v, "note": "Pass picks to import them."}))
    }
    let sync = a["sync"].as_bool().unwrap_or(true);
    let mut servers = vec![]; let mut keys = 0;
    for (s, here) in &sc { for (i, x) in s.found.servers.iter().enumerate() { if here[i].is_none() && picks.contains(&format!("{}|{i}", s.path)) { let names = x.secrets(); keys += s.found.plain.iter().filter(|(n, _)| names.contains(n)).count(); let mut x = x.clone(); x.sync = sync; x.source = format!("import:{}", s.path); servers.push(x) } } }
    if servers.is_empty() { return fail("none of those picks is there to import (mcp_import without picks lists them)") }
    if keys > 0 { return (false, json!({"error": "Some of these carry keys in their files. Keys move to the vault only when the person imports them: ask them to use Settings > MCP > Import."})) }
    gate(m, thread, "add", servers).await
}

/// The person answers an `mcp_server` ask: closed, and applied when accepted; the thread is told either way.
pub async fn answer(ag: &Arc<Agents>, ask: &str, thread: &str, options: &str, answer: &str) -> Result<(), String> {
    let now = svc::now_s(); let yes = matches!(answer, "accept" | "session");
    let o: Value = serde_json::from_str(options).unwrap_or(Value::Null);
    let op = o["op"].as_str().unwrap_or("add").to_string();
    let servers: Vec<Server> = serde_json::from_value(o["servers"].clone()).unwrap_or_default();
    let names = servers.iter().map(|s| s.id.clone()).collect::<Vec<_>>().join(", ");
    let pend = ag.pending_ev(thread, 0, ask, now).await;
    let told = if !yes { format!("The person declined the MCP server change ({names}); nothing was changed.") } else {
        match svc::get() { Some(m) => apply(&m, &op, servers).await.unwrap_or_else(|e| format!("The person allowed it, but it could not be applied: {e}")), None => "MCP is not running on this hub.".into() } };
    ag.commit_events(vec![json!({"$":"AskClosed","id":ask,"answer":answer,"at":now}), pend,
        json!({"$":"ActivityLogged","thread":thread,"act":format!("mcp:{ask}"),"tone":"info","kind":"answer","summary":if yes { format!("Allowed MCP: {names}") } else { format!("Declined MCP: {names}") },"at":now})]).await;
    let _ = ag.deliver_as("user", thread, &format!("t:{thread}:mcp-{ask}"), &told, None, vec![]).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_key_in_plain_text_is_refused_and_a_reference_kept() {
        assert!(kv_of("GITHUB_TOKEN", &json!("ghp_realvalue123")).unwrap_err().contains("secret_request"));
        assert_eq!(kv_of("GITHUB_TOKEN", &json!({"secret": "gh-pat"})).unwrap(), Kv::sec("GITHUB_TOKEN", "gh-pat", ""));
        assert_eq!(kv_of("Authorization", &json!({"secret": "gh-pat", "prefix": "Bearer "})).unwrap(), Kv::sec("Authorization", "gh-pat", "Bearer "));
        assert!(kv_of("X", &json!({"secret": "Bad Name"})).is_err());
        assert_eq!(kv_of("LOG", &json!("debug")).unwrap(), Kv::lit("LOG", "debug")); assert_eq!(kv_of("N", &json!(3)).unwrap(), Kv::lit("N", "3"));
        assert!(kv_of("API_KEY", &json!("${API_KEY}")).unwrap().secret);
    }
    #[test] fn the_confirm_card_says_what_runs() {
        let s = Server { id: "fetch".into(), transport: "stdio".into(), command: "uvx".into(), args: vec!["mcp-server-fetch".into()], env: vec![Kv::sec("K", "mcp-fetch-k", ""), Kv::lit("LOG", "1")], enabled: true, providers: vec!["codex".into()], ..Default::default() };
        let d = describe("add", &[s]); assert!(d.starts_with("Add this MCP server for agents?\n\nfetch (runs on this machine)\nuvx mcp-server-fetch\nSecrets from the vault: mcp-fetch-k\nLOG=1\nFor: codex"), "{d}");
        assert!(describe("on", &[]).starts_with("Turn on"));
    }
    #[test] fn every_tool_has_a_schema() { let d = defs(); assert_eq!(d.iter().map(|t| t["name"].as_str().unwrap()).collect::<Vec<_>>(), NAMES); assert!(d.iter().all(|t| t["inputSchema"]["type"] == "object")); }
}

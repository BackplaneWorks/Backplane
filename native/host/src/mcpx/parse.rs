//! What a person pastes, or a tool's config file holds, read as servers: Claude Code / Claude Desktop / Cursor / Windsurf / Gemini `mcpServers`,
//! VS Code `servers` (and `mcp.servers` in its settings, with `inputs`), OpenCode `mcp`, Codex TOML `[mcp_servers.*]`, a single server object,
//! a `claude mcp add` / `codex mcp add` line, or a bare command (`npx -y @scope/server`). JSON with comments and trailing commas (JSONC) is read too.
//! A secret is never kept as text: a value under a secret-looking name, or a placeholder (`${TOKEN}`, `<YOUR_KEY>`, `${input:x}`), becomes a vault
//! reference; a real value given for one is handed back in `Found::plain` for the caller to put in the vault.
use super::model::{placeholder, secretish, slug, unique, vault_name, Kv, Server};
use serde_json::{Map, Value};

#[derive(Debug, Default, PartialEq)]
pub struct Found {
    pub servers: Vec<Server>,
    /// (vault name, value): secrets that came with a value, to store before the servers are kept
    pub plain: Vec<(String, String)>,
    /// what was skipped and why
    pub notes: Vec<String>,
    /// the format it was read as
    pub format: &'static str,
}

/// Comments (`//`, `/* */`) and trailing commas out of JSONC, strings left alone.
pub fn strip_jsonc(s: &str) -> String {
    let b: Vec<char> = s.chars().collect(); let mut o = String::with_capacity(s.len()); let mut i = 0; let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str { o.push(c); if c == '\\' && i + 1 < b.len() { o.push(b[i + 1]); i += 2; continue } if c == '"' { in_str = false } i += 1; continue }
        if c == '"' { in_str = true; o.push(c); i += 1; continue }
        if c == '/' && b.get(i + 1) == Some(&'/') { while i < b.len() && b[i] != '\n' { i += 1 } continue }
        if c == '/' && b.get(i + 1) == Some(&'*') { i += 2; while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') { i += 1 } i += 2; continue }
        if c == ',' { let mut j = i + 1; while j < b.len() && b[j].is_whitespace() { j += 1 } if j < b.len() && (b[j] == '}' || b[j] == ']') { i += 1; continue } }
        o.push(c); i += 1;
    }
    o
}

/// Anything pasted. `taken`: the ids already in use (new ones are made unique against them and each other).
pub fn paste(text: &str, taken: &[String]) -> Result<Found, String> {
    let t = text.trim();
    if t.is_empty() { return Err("Paste a config snippet or a command".into()) }
    let stripped = strip_jsonc(t); let j = stripped.trim();
    if j.starts_with('{') || j.starts_with('"') {
        let t = j;
        let body = if t.starts_with('"') { format!("{{{t}}}") } else { t.to_string() }; // `"name": {...}` copied without its braces
        let v: Value = serde_json::from_str(&strip_jsonc(&body)).map_err(|e| format!("That is not JSON: {e}"))?;
        return json_doc(&v, taken)
    }
    if t.contains("[mcp_servers") || t.lines().any(|l| l.trim_start().starts_with("mcp_servers.")) {
        let v = super::toml::parse(t).map_err(|e| format!("That is not TOML: {e}"))?;
        return Ok(codex(&v, taken))
    }
    command_line(t, taken)
}

/// A JSON document in any of the formats.
pub fn json_doc(v: &Value, taken: &[String]) -> Result<Found, String> {
    let o = v.as_object().ok_or("The JSON is not an object")?;
    let inputs = v.get("inputs").or_else(|| v.pointer("/mcp/inputs"));
    let (map, format) = if let Some(m) = o.get("mcpServers").and_then(Value::as_object) { (m.clone(), "mcpServers") }
        else if let Some(m) = o.get("servers").and_then(Value::as_object) { (m.clone(), "VS Code") }
        else if let Some(m) = v.pointer("/mcp/servers").and_then(Value::as_object) { (m.clone(), "VS Code settings") }
        else if let Some(m) = o.get("mcp").and_then(Value::as_object).filter(|m| m.values().any(looks_like_server)) { (m.clone(), "OpenCode") }
        else if let Some(m) = o.get("context_servers").and_then(Value::as_object) { (m.clone(), "Zed") }
        else if looks_like_server(v) { let mut m = Map::new(); m.insert(name_for(v), v.clone()); (m, "server") }
        else if !o.is_empty() && o.values().all(looks_like_server) { (o.clone(), "mcpServers") }
        else { return Err("No MCP servers found in it (looked for mcpServers, servers, mcp_servers and a command or url)".into()) };
    let mut f = Found { format, ..Default::default() };
    let mut ids: Vec<String> = taken.to_vec();
    let secret_inputs: Vec<String> = inputs.and_then(Value::as_array).map(|a| a.iter().filter(|i| i["password"] == true).filter_map(|i| i["id"].as_str().map(str::to_string)).collect()).unwrap_or_default();
    for (name, sv) in &map {
        match one(name, sv, &ids, &secret_inputs, &mut f.plain) { Ok(s) => { ids.push(s.id.clone()); f.servers.push(s) } Err(e) => f.notes.push(format!("{name}: {e}")) }
    }
    if f.servers.is_empty() && !f.notes.is_empty() { return Err(f.notes.join("; ")) }
    Ok(f)
}

fn looks_like_server(v: &Value) -> bool { v.is_object() && (v.get("command").is_some() || v.get("url").is_some() || v.get("httpUrl").is_some() || v.get("serverUrl").is_some()) }
/// A name for a server pasted without one: its package (`@modelcontextprotocol/server-fetch` → `fetch`), its host, or "server".
fn name_for(v: &Value) -> String {
    if let Some(u) = v.get("url").or_else(|| v.get("httpUrl")).and_then(Value::as_str) { return host_name(u) }
    let args: Vec<&str> = v.get("args").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let cmd = match v.get("command") { Some(Value::String(c)) => c.clone(), Some(Value::Array(a)) => a.first().and_then(Value::as_str).unwrap_or("").to_string(), _ => String::new() };
    package_name(&cmd, &args)
}
pub fn host_name(u: &str) -> String {
    let h = u.split("//").nth(1).unwrap_or(u).split(['/', ':', '?']).next().unwrap_or("");
    let parts: Vec<&str> = h.split('.').filter(|p| !matches!(*p, "www" | "mcp" | "api" | "com" | "io" | "dev" | "net" | "org" | "ai" | "app" | "cloud" | "co")).collect();
    slug(parts.last().copied().unwrap_or("remote"))
}
fn package_name(cmd: &str, args: &[&str]) -> String {
    let base = cmd.rsplit(['/', '\\']).next().unwrap_or(cmd);
    let words: Vec<&str> = args.iter().copied().filter(|a| !a.starts_with('-') && !matches!(*a, "run" | "exec" | "dlx" | "tool" | "x")).collect();
    let runner = matches!(base, "npx" | "bunx" | "pnpx" | "uvx" | "pipx" | "deno" | "uv" | "node" | "python" | "python3" | "npm" | "pnpm" | "yarn");
    // docker: the image is the last word (after -e NAME pairs); a runner: its first word is the package
    let pkg = if base == "docker" { words.last().copied().unwrap_or(base) } else if runner { words.first().copied().unwrap_or(base) } else { base };
    let last = pkg.rsplit('/').next().unwrap_or(pkg);
    let last = last.split('@').next().filter(|s| !s.is_empty()).unwrap_or(last); // pkg@latest
    let last = last.split(':').next().unwrap_or(last); // image:tag
    let s = last.trim_start_matches("mcp-server-").trim_start_matches("server-").trim_end_matches("-mcp-server").trim_end_matches("-mcp").trim_end_matches(".py").trim_end_matches(".js");
    slug(if s.is_empty() { last } else { s })
}

fn strs(v: Option<&Value>) -> Vec<String> {
    match v { Some(Value::Array(a)) => a.iter().map(|x| match x { Value::String(s) => s.clone(), o => o.to_string() }).collect(), Some(Value::String(s)) => vec![s.clone()], _ => vec![] }
}
/// One env or header value: literal, or a vault reference (with the value to store, if it came with one).
fn kv(id: &str, k: &str, v: &Value, secret_inputs: &[String], plain: &mut Vec<(String, String)>) -> Kv {
    let raw = match v { Value::String(s) => s.clone(), Value::Null => String::new(), o => o.to_string() };
    // `Bearer ${TOKEN}` / `Bearer <token>`: the vault holds the token, the prefix stays
    let (pre, val) = match raw.split_once(' ') { Some((p, rest)) if matches!(p.to_ascii_lowercase().as_str(), "bearer" | "token" | "basic") => (format!("{p} "), rest.to_string()), _ => (String::new(), raw.clone()) };
    let input_secret = val.strip_prefix("${input:").and_then(|r| r.strip_suffix('}')).map_or(false, |i| secret_inputs.iter().any(|s| s == i) || secretish(i));
    let is_secret = secretish(k) || input_secret || (placeholder(&val) && !val.trim().is_empty() && (val.starts_with("<") || val.to_ascii_uppercase().contains("YOUR")));
    if !is_secret { return Kv::lit(k, &raw) }
    let name = vault_name(id, k);
    if !placeholder(&val) { plain.push((name.clone(), val)) }
    Kv::sec(k, &name, &pre)
}
fn kvs(id: &str, v: Option<&Value>, secret_inputs: &[String], plain: &mut Vec<(String, String)>) -> Vec<Kv> {
    v.and_then(Value::as_object).map(|m| m.iter().map(|(k, x)| kv(id, k, x, secret_inputs, plain)).collect()).unwrap_or_default()
}

/// One entry of an `mcpServers`-like map.
fn one(name: &str, v: &Value, taken: &[String], secret_inputs: &[String], plain: &mut Vec<(String, String)>) -> Result<Server, String> {
    if !v.is_object() { return Err("not an object".into()) }
    let id = unique(&slug(name), taken);
    let ty = v.get("type").or_else(|| v.get("transport")).and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    let url = ["url", "httpUrl", "serverUrl"].iter().find_map(|k| v.get(*k).and_then(Value::as_str)).unwrap_or("").to_string();
    let mut s = Server { id: id.clone(), name: name.to_string(), enabled: !(v.get("disabled") == Some(&Value::Bool(true)) || v.get("enabled") == Some(&Value::Bool(false))), source: "paste".into(), ..Default::default() };
    if !url.is_empty() {
        s.transport = if ty.contains("sse") || (ty.is_empty() && v.get("httpUrl").is_none() && url.trim_end_matches('/').ends_with("/sse")) { "sse".into() } else { "http".into() };
        s.url = url;
        s.headers = kvs(&id, v.get("headers").or_else(|| v.get("http_headers")), secret_inputs, plain);
    } else {
        s.transport = "stdio".into();
        // OpenCode: "command": ["npx", "-y", "x"], "environment": {}
        match v.get("command") { Some(Value::Array(_)) => { let c = strs(v.get("command")); s.command = c.first().cloned().unwrap_or_default(); s.args = c[1.min(c.len())..].to_vec(); s.args.extend(strs(v.get("args"))) }
            Some(Value::String(c)) => { s.command = c.clone(); s.args = strs(v.get("args")) } _ => return Err("no command or url".into()) }
        if s.command.trim().is_empty() { return Err("an empty command".into()) }
        s.env = kvs(&id, v.get("env").or_else(|| v.get("environment")), secret_inputs, plain);
        s.cwd = v.get("cwd").and_then(Value::as_str).unwrap_or("").to_string();
    }
    s.disabled_tools = strs(v.get("disabledTools").or_else(|| v.get("disabled_tools")));
    s.problem().map_or(Ok(s), Err)
}

/// Codex's `[mcp_servers.<name>]` tables (a config.toml or a snippet of one).
pub fn codex(v: &Value, taken: &[String]) -> Found {
    let mut f = Found { format: "Codex TOML", ..Default::default() };
    let Some(m) = v.get("mcp_servers").and_then(Value::as_object) else { f.notes.push("no [mcp_servers] tables".into()); return f };
    let mut ids = taken.to_vec();
    for (name, t) in m {
        let id = unique(&slug(name), &ids);
        let mut s = Server { id: id.clone(), name: name.clone(), enabled: t.get("enabled") != Some(&Value::Bool(false)), source: "paste".into(), ..Default::default() };
        if let Some(u) = t.get("url").and_then(Value::as_str) {
            s.transport = "http".into(); s.url = u.into();
            s.headers = kvs(&id, t.get("http_headers"), &[], &mut f.plain);
            if let Some(var) = t.get("bearer_token_env_var").and_then(Value::as_str) { s.headers.push(Kv::sec("Authorization", &vault_name(&id, var), "Bearer ")) }
            if let Some(eh) = t.get("env_http_headers").and_then(Value::as_object) { for (h, var) in eh { s.headers.push(Kv::sec(h, &vault_name(&id, var.as_str().unwrap_or(h)), "")) } }
        } else if let Some(c) = t.get("command").and_then(Value::as_str) {
            s.transport = "stdio".into(); s.command = c.into(); s.args = strs(t.get("args"));
            s.env = kvs(&id, t.get("env"), &[], &mut f.plain);
            for var in strs(t.get("env_vars")) { if !s.env.iter().any(|k| k.k == var) { s.env.push(if secretish(&var) { Kv::sec(&var, &vault_name(&id, &var), "") } else { Kv::lit(&var, &format!("${{{var}}}")) }) } }
            s.cwd = t.get("cwd").and_then(Value::as_str).unwrap_or("").into();
        } else { f.notes.push(format!("{name}: no command or url")); continue }
        s.disabled_tools = strs(t.get("disabled_tools"));
        match s.problem() { None => { ids.push(id); f.servers.push(s) } Some(e) => f.notes.push(format!("{name}: {e}")) }
    }
    f
}

/// Words split as a shell would (quotes and backslashes), no expansion.
pub fn shell_words(s: &str) -> Result<Vec<String>, String> {
    let mut out = vec![]; let mut cur = String::new(); let mut have = false; let mut q: Option<char> = None; let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match (q, c) {
            (Some(x), c) if c == x => q = None,
            (Some('"'), '\\') => { if let Some(n) = it.next() { cur.push(n) } }
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => { q = Some(c); have = true }
            (None, '\\') => { match it.next() { Some('\n') => {} Some(n) => { cur.push(n); have = true } None => {} } }
            (None, c) if c.is_whitespace() => { if have || !cur.is_empty() { out.push(std::mem::take(&mut cur)); have = false } }
            (None, c) => { cur.push(c); have = true }
        }
    }
    if q.is_some() { return Err("a quote is not closed".into()) }
    if have || !cur.is_empty() { out.push(cur) }
    Ok(out)
}

/// `claude mcp add [-s scope] [-t http|sse|stdio] [-e K=V] [-H "K: V"] <name> [--] <command or url> [args]`, `codex mcp add <name> [--env K=V] [--url U] -- <cmd>`,
/// `KEY=v npx -y pkg`, or a bare command.
pub fn command_line(t: &str, taken: &[String]) -> Result<Found, String> {
    let line: String = t.lines().map(|l| l.trim_end().trim_end_matches('\\')).collect::<Vec<_>>().join(" ");
    let mut w = shell_words(line.trim().trim_start_matches("$ "))?;
    if w.is_empty() { return Err("Paste a config snippet or a command".into()) }
    let mut f = Found { format: "command", ..Default::default() };
    let mut env: Vec<(String, String)> = vec![]; let mut headers: Vec<(String, String)> = vec![]; let mut ty = String::new(); let mut name = String::new(); let mut url = String::new();
    let tool = if w.len() >= 3 && w[1] == "mcp" && w[2] == "add" && matches!(w[0].as_str(), "claude" | "codex") { let t = w[0].clone(); w.drain(..3); t } else { String::new() };
    if !tool.is_empty() {
        let mut rest = vec![]; let mut i = 0; let mut dashdash = false;
        while i < w.len() {
            let a = w[i].clone();
            if dashdash { rest.push(a); i += 1; continue }
            let val = |i: &mut usize| -> String { *i += 1; w.get(*i).cloned().unwrap_or_default() };
            match a.as_str() {
                "--" => dashdash = true,
                "-s" | "--scope" => { val(&mut i); }
                "-t" | "--transport" => ty = val(&mut i),
                "-e" | "--env" => { let kv = val(&mut i); if let Some((k, v)) = kv.split_once('=') { env.push((k.into(), v.into())) } }
                "-H" | "--header" => { let h = val(&mut i); if let Some((k, v)) = h.split_once(':') { headers.push((k.trim().into(), v.trim().into())) } }
                "--url" => { url = val(&mut i); }
                "--bearer-token-env-var" => { let var = val(&mut i); headers.push(("Authorization".into(), format!("Bearer ${{{var}}}"))) }
                x if x.starts_with("--transport=") || x.starts_with("-t=") => ty = x.split_once('=').unwrap().1.into(),
                x if x.starts_with("--scope=") => {}
                x if x.starts_with('-') && name.is_empty() => {}
                _ => { if name.is_empty() { name = a } else { rest.push(a) } }
            }
            i += 1;
        }
        if url.is_empty() && rest.len() == 1 && (rest[0].starts_with("http://") || rest[0].starts_with("https://")) { url = rest.remove(0) }
        w = rest;
    } else {
        while w.first().map_or(false, |a| a.contains('=') && !a.starts_with('-') && a.split('=').next().map_or(false, |k| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))) { let a = w.remove(0); let (k, v) = a.split_once('=').unwrap(); env.push((k.into(), v.into())) }
        if w.len() == 1 && (w[0].starts_with("http://") || w[0].starts_with("https://")) { url = w.remove(0) }
    }
    let mut v = serde_json::json!({});
    if !url.is_empty() {
        v["url"] = url.clone().into(); if !ty.is_empty() { v["type"] = ty.clone().into() }
        v["headers"] = Value::Object(headers.into_iter().map(|(k, x)| (k, Value::String(x))).collect());
    } else {
        if w.is_empty() { return Err("No command in it".into()) }
        if !w[0].chars().next().map_or(false, |c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '~' | '_')) { return Err("That does not look like a command, a URL or a config snippet".into()) }
        v["command"] = w[0].clone().into(); v["args"] = w[1..].to_vec().into();
        v["env"] = Value::Object(env.into_iter().map(|(k, x)| (k, Value::String(x))).collect());
    }
    if name.is_empty() { name = name_for(&v) }
    let s = one(&name, &v, taken, &[], &mut f.plain)?;
    f.servers.push(s); Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn p(t: &str) -> Found { paste(t, &[]).unwrap_or_else(|e| panic!("{e}: {t}")) }
    #[test] fn claude_style_mcp_servers_with_every_transport() {
        let f = p(r#"{"mcpServers":{
            "fetch":{"command":"uvx","args":["mcp-server-fetch"]},
            "github":{"type":"http","url":"https://api.githubcopilot.com/mcp/","headers":{"Authorization":"Bearer ${GITHUB_PAT}"}},
            "old":{"type":"sse","url":"https://x.example/sse"},
            "Everything Server":{"command":"npx","args":["-y","@modelcontextprotocol/server-everything"],"env":{"API_KEY":"sk-real-123456","LOG":"debug"}}}}"#);
        assert_eq!(f.format, "mcpServers"); assert_eq!(f.servers.len(), 4);
        let ids: Vec<&str> = f.servers.iter().map(|s| s.id.as_str()).collect(); assert_eq!(ids, ["fetch", "github", "old", "everything-server"]);
        assert_eq!((f.servers[0].transport.as_str(), f.servers[0].command.as_str(), f.servers[0].args.clone()), ("stdio", "uvx", vec!["mcp-server-fetch".to_string()]));
        assert_eq!(f.servers[1].headers, vec![Kv::sec("Authorization", "mcp-github-authorization", "Bearer ")]);
        assert_eq!(f.servers[2].transport, "sse");
        assert_eq!(f.servers[3].env, vec![Kv::sec("API_KEY", "mcp-everything-server-api_key", ""), Kv::lit("LOG", "debug")]);
        assert_eq!(f.plain, vec![("mcp-everything-server-api_key".to_string(), "sk-real-123456".to_string())], "a real key goes to the vault, never into the list");
        assert!(!serde_json::to_string(&f.servers).unwrap().contains("sk-real"));
    }
    #[test] fn vscode_cursor_gemini_opencode_and_bare_shapes() {
        let vs = p(r#"// VS Code mcp.json
            {"inputs":[{"type":"promptString","id":"gh","description":"GitHub PAT","password":true}],
             "servers":{"github":{"command":"docker","args":["run","-i","--rm","-e","GITHUB_PERSONAL_ACCESS_TOKEN","ghcr.io/github/github-mcp-server"],"env":{"GITHUB_PERSONAL_ACCESS_TOKEN":"${input:gh}"}},
                        "remote":{"type":"http","url":"https://h.example/mcp",},}}"#);
        assert_eq!(vs.format, "VS Code"); assert_eq!(vs.servers[0].env, vec![Kv::sec("GITHUB_PERSONAL_ACCESS_TOKEN", "mcp-github-github_personal_access_token", "")]); assert!(vs.plain.is_empty());
        assert_eq!(vs.servers[1].transport, "http");
        let set = p(r#"{"editor.fontSize":12,"mcp":{"servers":{"a":{"command":"node","args":["s.js"]}}}}"#); assert_eq!(set.format, "VS Code settings"); assert_eq!(set.servers[0].id, "a");
        let gem = p(r#"{"mcpServers":{"g":{"httpUrl":"https://g.example/mcp"},"s":{"url":"https://g.example/events/sse"}}}"#); assert_eq!(gem.servers[0].transport, "http"); assert_eq!(gem.servers[1].transport, "sse");
        let oc = p(r#"{"$schema":"https://opencode.ai/config.json","mcp":{"loc":{"type":"local","command":["npx","-y","pkg"],"environment":{"K":"v"}},"rem":{"type":"remote","url":"https://r.example/mcp"}}}"#);
        assert_eq!(oc.format, "OpenCode"); assert_eq!((oc.servers[0].command.as_str(), oc.servers[0].args.clone()), ("npx", vec!["-y".to_string(), "pkg".to_string()])); assert_eq!(oc.servers[1].transport, "http");
        let one = p(r#"{"command":"npx","args":["-y","@modelcontextprotocol/server-everything"]}"#); assert_eq!(one.servers[0].id, "everything");
        let named = p(r#""microchip": {"url": "https://docs.microchip.example/mcp"}"#); assert_eq!(named.servers[0].id, "microchip");
        let bare = p(r#"{"fetch":{"command":"uvx","args":["mcp-server-fetch"]}}"#); assert_eq!(bare.servers[0].id, "fetch");
        let win = p(r#"{"mcpServers":{"w":{"serverUrl":"https://w.example/mcp","disabled":true}}}"#); assert!(!win.servers[0].enabled);
    }
    #[test] fn codex_toml_reads_bearer_and_env_headers_as_vault_refs() {
        let f = p("[mcp_servers.fetch]\ncommand = \"uvx\"\nargs = [\"mcp-server-fetch\"]\nenv = { \"TOKEN\" = \"abc-real-token\" }\n\n[mcp_servers.gh]\nurl = \"https://api.githubcopilot.com/mcp/\"\nbearer_token_env_var = \"GITHUB_PAT\"\nenv_http_headers = { \"X-Key\" = \"X_KEY\" }\ndisabled_tools = [\"delete_repo\"]\n");
        assert_eq!(f.format, "Codex TOML"); assert_eq!(f.servers.len(), 2);
        assert_eq!(f.servers[0].env, vec![Kv::sec("TOKEN", "mcp-fetch-token", "")]); assert_eq!(f.plain, vec![("mcp-fetch-token".to_string(), "abc-real-token".to_string())]);
        assert_eq!(f.servers[1].headers, vec![Kv::sec("Authorization", "mcp-gh-github_pat", "Bearer "), Kv::sec("X-Key", "mcp-gh-x_key", "")]); assert_eq!(f.servers[1].disabled_tools, ["delete_repo"]);
        let dotted = p("mcp_servers.x.command = \"x-server\"\n"); assert_eq!(dotted.servers[0].command, "x-server");
    }
    #[test] fn command_lines_from_docs() {
        let c = p("claude mcp add --transport http stm32 https://mcp.st.example/mcp -H \"Authorization: Bearer <YOUR_TOKEN>\""); assert_eq!((c.servers[0].id.as_str(), c.servers[0].transport.as_str(), c.servers[0].url.as_str()), ("stm32", "http", "https://mcp.st.example/mcp"));
        assert_eq!(c.servers[0].headers, vec![Kv::sec("Authorization", "mcp-stm32-authorization", "Bearer ")]); assert!(c.plain.is_empty());
        let c = p("claude mcp add fetch -s user -e LOG=1 -- uvx mcp-server-fetch --ignore-robots-txt"); assert_eq!(c.servers[0].args, ["mcp-server-fetch", "--ignore-robots-txt"]); assert_eq!(c.servers[0].env, vec![Kv::lit("LOG", "1")]);
        let c = p("codex mcp add ctx --env API_KEY=k-123456789 -- npx -y @upstash/context7-mcp"); assert_eq!((c.servers[0].id.as_str(), c.servers[0].command.as_str()), ("ctx", "npx")); assert_eq!(c.plain.len(), 1);
        let c = p("codex mcp add gh --url https://api.githubcopilot.com/mcp/ --bearer-token-env-var GITHUB_PAT"); assert_eq!(c.servers[0].headers, vec![Kv::sec("Authorization", "mcp-gh-authorization", "Bearer ")]);
        let c = p("npx -y @modelcontextprotocol/server-everything"); assert_eq!(c.servers[0].id, "everything");
        let c = p("BRAVE_API_KEY=abcdefgh123 npx -y @brave/brave-search-mcp-server"); assert_eq!(c.servers[0].id, "brave-search"); assert_eq!(c.plain[0].1, "abcdefgh123");
        let c = p("uvx mcp-server-fetch"); assert_eq!(c.servers[0].id, "fetch");
        let c = p("docker run -i --rm mcp/fetch:latest"); assert_eq!(c.servers[0].id, "fetch");
        let c = p("https://mcp.example.org/sse"); assert_eq!((c.servers[0].transport.as_str(), c.servers[0].id.as_str()), ("sse", "example"));
        assert!(paste("{nope", &[]).unwrap_err().contains("not JSON")); assert!(paste("{\"a\":1}", &[]).is_err()); assert!(paste("   ", &[]).is_err()); assert!(paste("{{ hi", &[]).is_err());
    }
    #[test] fn ids_are_unique_against_whats_there() {
        let f = paste(r#"{"mcpServers":{"fetch":{"command":"a"},"Fetch":{"command":"b"},"backplane":{"command":"c"}}}"#, &["fetch".into()]).unwrap();
        let ids: Vec<&str> = f.servers.iter().map(|s| s.id.as_str()).collect(); assert_eq!(ids, ["fetch-2", "fetch-3", "backplane-2"]);
    }
    #[test] fn jsonc_comments_and_commas_go_strings_stay() {
        assert_eq!(serde_json::from_str::<Value>(&strip_jsonc("{\"a\":\"// not /* a */ comment\", /* x */ \"b\":[1,2,],}")).unwrap(), json!({"a":"// not /* a */ comment","b":[1,2]}));
        assert_eq!(shell_words("a 'b c' \"d \\\" e\" f\\ g").unwrap(), ["a", "b c", "d \" e", "f g"]); assert!(shell_words("'open").is_err());
    }
}

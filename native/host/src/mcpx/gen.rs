//! Each provider's MCP config, from the one list (docs/MCP.md "How agents get them"). Pure: the servers a thread gets, how the hub is reached and
//! the vault's values in, the config out.
//!
//! A stdio server is always started through the hub's own wrapper (`backplane-hub --mcp-run <id>`, run.rs): it asks the hub for the command and
//! its env with the vault's values put in, hides the tools turned off and reports use and failure. So no secret is on a command line or in a
//! file, and every provider gets the same tool filter. A remote server is given to the provider natively; a secret header reaches it the
//! provider's own way: Claude's `headersHelper` (the wrapper prints the headers), Codex's `env_http_headers` (an env variable of the Codex
//! process, named so Codex keeps it from the shell it runs), and the ACP agents' `headers` (on the agent's stdin, never a command line).
use super::model::{Kv, Server};
use serde_json::{json, Map, Value};

pub const PCBPARTS: &str = "https://pcbparts.dev/mcp";

/// How the hub is reached by what it starts: its own program and this agent process's Backplane MCP URL.
#[derive(Clone, Debug, Default)]
pub struct Ctx { pub exe: String, pub url: String, pub pcbparts: bool }

pub type Vault<'a> = &'a dyn Fn(&str) -> Option<String>;
/// A header's value with its secret put in; None when the vault does not hold it.
pub fn value(kv: &Kv, vault: Vault) -> Option<String> { if kv.secret { vault(&kv.v).map(|x| format!("{}{x}", kv.pre)) } else { Some(kv.v.clone()) } }

/// The wrapper an agent starts for a stdio server: (command, args, env).
pub fn wrapper(s: &Server, c: &Ctx) -> (String, Vec<String>, Vec<(String, String)>) {
    (c.exe.clone(), vec!["--mcp-run".into(), s.id.clone()], vec![("BACKPLANE_MCP_URL".into(), c.url.clone())])
}
fn obj(kv: &[(String, String)]) -> Value { Value::Object(kv.iter().map(|(k, v)| (k.clone(), json!(v))).collect()) }
/// A word quoted for `sh -c` (Claude runs `headersHelper` through a shell).
pub fn sh_quote(s: &str) -> String { format!("'{}'", s.replace('\'', "'\\''")) }

/// Claude Code's `--mcp-config` JSON: Backplane's own, pcbparts, then the list.
pub fn claude(servers: &[&Server], c: &Ctx, vault: Vault) -> Value {
    let mut m = Map::new();
    m.insert("backplane".into(), json!({"type":"http","url":c.url}));
    if c.pcbparts { m.insert("pcbparts".into(), json!({"type":"http","url":PCBPARTS})); }
    for s in servers {
        let v = if s.remote() {
            let mut o = json!({"type": s.transport, "url": s.url});
            let plain: Vec<(String, String)> = s.headers.iter().filter(|k| !k.secret).map(|k| (k.k.clone(), k.v.clone())).collect();
            if !plain.is_empty() { o["headers"] = obj(&plain) }
            if s.headers.iter().any(|k| k.secret) { o["headersHelper"] = json!(format!("{} --mcp-headers {} {}", sh_quote(&c.exe), sh_quote(&s.id), sh_quote(&c.url))) }
            let _ = vault; o
        } else { let (cmd, args, env) = wrapper(s, c); json!({"type":"stdio","command":cmd,"args":args,"env":obj(&env)}) };
        m.insert(s.id.clone(), v);
    }
    json!({"mcpServers": m})
}
/// The tools Claude is told not to use: a remote server's turned-off tools (a stdio server's are hidden by the wrapper).
pub fn claude_disallowed(servers: &[&Server]) -> Vec<String> { servers.iter().filter(|s| s.remote()).flat_map(|s| s.disabled_tools.iter().map(move |t| format!("mcp__{}__{t}", s.id))).collect() }

fn toml_str(s: &str) -> String { Value::String(s.to_string()).to_string() }
fn toml_arr(xs: &[String]) -> String { format!("[{}]", xs.iter().map(|x| toml_str(x)).collect::<Vec<_>>().join(",")) }
fn toml_tbl(kv: &[(String, String)]) -> String { format!("{{{}}}", kv.iter().map(|(k, v)| format!("{}={}", toml_str(k), toml_str(v))).collect::<Vec<_>>().join(",")) }
/// The env variable a Codex process carries a server's secret header in. "TOKEN" in it keeps it out of the commands Codex runs (its shell
/// environment policy's default excludes).
pub fn codex_var(id: &str, i: usize) -> String { format!("BACKPLANE_MCP_{}_{i}_TOKEN", id.to_ascii_uppercase().replace('-', "_")) }
/// Codex's `-c` overrides for the list (Backplane's own and pcbparts are set by codex.rs), and the env the Codex process needs for secret headers.
/// A secret the vault does not hold is left out (the server then answers 401, which its card shows).
pub fn codex(servers: &[&Server], c: &Ctx, vault: Vault) -> (Vec<String>, Vec<(String, String)>) {
    let (mut a, mut env) = (vec![], vec![]);
    let mut set = |k: String, v: String| { a.push("-c".to_string()); a.push(format!("{k}={v}")) };
    for s in servers {
        let p = format!("mcp_servers.{}", s.id);
        if s.remote() {
            set(format!("{p}.url"), toml_str(&s.url));
            let plain: Vec<(String, String)> = s.headers.iter().filter(|k| !k.secret).map(|k| (k.k.clone(), k.v.clone())).collect();
            if !plain.is_empty() { set(format!("{p}.http_headers"), toml_tbl(&plain)) }
            let mut envh = vec![];
            for (i, h) in s.headers.iter().filter(|k| k.secret).enumerate() { if let Some(v) = value(h, vault) { let var = codex_var(&s.id, i); envh.push((h.k.clone(), var.clone())); env.push((var, v)) } }
            if !envh.is_empty() { set(format!("{p}.env_http_headers"), toml_tbl(&envh)) }
            if !s.disabled_tools.is_empty() { set(format!("{p}.disabled_tools"), toml_arr(&s.disabled_tools)) }
        } else {
            let (cmd, args, e) = wrapper(s, c);
            set(format!("{p}.command"), toml_str(&cmd)); set(format!("{p}.args"), toml_arr(&args)); set(format!("{p}.env"), toml_tbl(&e));
        }
        set(format!("{p}.startup_timeout_sec"), "60".into());
    }
    (a, env)
}

/// The ACP agents' `mcpServers` (session/new): Backplane's own, then the list; headers with their secrets put in (they go on the agent's stdin).
pub fn acp(servers: &[&Server], c: &Ctx, vault: Vault) -> Value {
    let nv = |kv: &[(String, String)]| Value::Array(kv.iter().map(|(k, v)| json!({"name":k,"value":v})).collect());
    let mut out = vec![json!({"type":"http","name":"backplane","url":c.url,"headers":[]})];
    for s in servers {
        out.push(if s.remote() {
            let hs: Vec<(String, String)> = s.headers.iter().filter_map(|h| value(h, vault).map(|v| (h.k.clone(), v))).collect();
            json!({"type": s.transport, "name": s.id, "url": s.url, "headers": nv(&hs)})
        } else { let (cmd, args, env) = wrapper(s, c); json!({"name": s.id, "command": cmd, "args": args, "env": nv(&env)}) });
    }
    Value::Array(out)
}

/// What the list means for one thread, as a short hash: a long-lived agent (Claude) whose servers changed is started again at its next turn.
pub fn fingerprint(servers: &[&Server], pcbparts: bool) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new(); h.update(if pcbparts { b"p1" } else { b"p0" });
    for s in servers { h.update(serde_json::to_string(s).unwrap_or_default().as_bytes()); h.update(b"\n"); }
    hex::encode(&h.finalize()[..6])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c() -> Ctx { Ctx { exe: "/opt/bp/backplane-hub".into(), url: "http://127.0.0.1:9/mcp/t.1?k=K".into(), pcbparts: true } }
    fn st() -> Server { Server { id: "fetch".into(), transport: "stdio".into(), command: "uvx".into(), args: vec!["mcp-server-fetch".into()], env: vec![Kv::sec("API_KEY", "mcp-fetch-api_key", "")], enabled: true, disabled_tools: vec!["x".into()], ..Default::default() } }
    fn rem() -> Server { Server { id: "gh".into(), transport: "http".into(), url: "https://api.githubcopilot.com/mcp/".into(), headers: vec![Kv::lit("X-Mode", "ro"), Kv::sec("Authorization", "mcp-gh-pat", "Bearer ")], enabled: true, disabled_tools: vec!["delete_repo".into()], ..Default::default() } }
    fn vault(n: &str) -> Option<String> { (n == "mcp-gh-pat").then(|| "ghp_SECRET".to_string()) }
    #[test] fn claude_gets_the_wrapper_for_stdio_and_a_helper_for_secret_headers() {
        let (a, b) = (st(), rem()); let v = claude(&[&a, &b], &c(), &vault);
        let m = v["mcpServers"].as_object().unwrap(); assert_eq!(m.keys().collect::<Vec<_>>(), ["backplane", "pcbparts", "fetch", "gh"]);
        assert_eq!(v["mcpServers"]["fetch"], json!({"type":"stdio","command":"/opt/bp/backplane-hub","args":["--mcp-run","fetch"],"env":{"BACKPLANE_MCP_URL":"http://127.0.0.1:9/mcp/t.1?k=K"}}));
        assert_eq!(v["mcpServers"]["gh"]["headers"], json!({"X-Mode":"ro"}));
        assert_eq!(v["mcpServers"]["gh"]["headersHelper"], "'/opt/bp/backplane-hub' --mcp-headers 'gh' 'http://127.0.0.1:9/mcp/t.1?k=K'");
        assert!(!v.to_string().contains("ghp_SECRET") && !v.to_string().contains("mcp-fetch-api_key"), "no secret and no vault name in Claude's config");
        assert_eq!(claude_disallowed(&[&a, &b]), ["mcp__gh__delete_repo"]);
        let off = claude(&[], &Ctx { pcbparts: false, ..c() }, &vault); assert_eq!(off["mcpServers"].as_object().unwrap().len(), 1);
    }
    #[test] fn codex_gets_toml_overrides_and_its_secret_in_its_own_env() {
        let (a, b) = (st(), rem()); let (args, env) = codex(&[&a, &b], &c(), &vault);
        let kv: Vec<&String> = args.iter().skip(1).step_by(2).collect();
        assert!(args.iter().step_by(2).all(|x| x == "-c"));
        assert_eq!(kv, [
            "mcp_servers.fetch.command=\"/opt/bp/backplane-hub\"", "mcp_servers.fetch.args=[\"--mcp-run\",\"fetch\"]", "mcp_servers.fetch.env={\"BACKPLANE_MCP_URL\"=\"http://127.0.0.1:9/mcp/t.1?k=K\"}", "mcp_servers.fetch.startup_timeout_sec=60",
            "mcp_servers.gh.url=\"https://api.githubcopilot.com/mcp/\"", "mcp_servers.gh.http_headers={\"X-Mode\"=\"ro\"}", "mcp_servers.gh.env_http_headers={\"Authorization\"=\"BACKPLANE_MCP_GH_0_TOKEN\"}", "mcp_servers.gh.disabled_tools=[\"delete_repo\"]", "mcp_servers.gh.startup_timeout_sec=60"]);
        assert_eq!(env, vec![("BACKPLANE_MCP_GH_0_TOKEN".to_string(), "Bearer ghp_SECRET".to_string())]);
        assert!(!args.join(" ").contains("ghp_SECRET"));
        for a in kv { let (_, v) = a.split_once('=').unwrap(); crate::mcpx::toml::parse(&format!("x = {v}\n")).unwrap_or_else(|e| panic!("{v}: {e}")); }
        let (_, env) = codex(&[&rem()], &c(), &|_| None); assert!(env.is_empty(), "a secret the vault lacks is left out");
    }
    #[test] fn acp_gets_name_value_lists() {
        let (a, b) = (st(), rem()); let v = acp(&[&a, &b], &c(), &vault);
        assert_eq!(v[0]["name"], "backplane"); assert_eq!(v[1], json!({"name":"fetch","command":"/opt/bp/backplane-hub","args":["--mcp-run","fetch"],"env":[{"name":"BACKPLANE_MCP_URL","value":"http://127.0.0.1:9/mcp/t.1?k=K"}]}));
        assert_eq!(v[2]["headers"], json!([{"name":"X-Mode","value":"ro"},{"name":"Authorization","value":"Bearer ghp_SECRET"}])); assert_eq!(v[2]["type"], "http");
    }
    #[test] fn the_fingerprint_moves_with_the_list() {
        let (a, mut b) = (st(), rem()); let f = fingerprint(&[&a, &b], true);
        assert_eq!(f, fingerprint(&[&a, &b], true)); assert_ne!(f, fingerprint(&[&a], true)); assert_ne!(f, fingerprint(&[&a, &b], false));
        b.disabled_tools.clear(); assert_ne!(f, fingerprint(&[&a, &b], true)); assert_eq!(f.len(), 12);
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
    }
}

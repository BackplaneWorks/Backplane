//! Discover project MCP from the active checkout. The bytes hashed are the bytes parsed.
use super::{model::{Kv, Server}, parse};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::{Path, PathBuf}};

const LIMIT: u64 = 256 * 1024;

pub struct Config {
    pub path: PathBuf,
    pub hash: String,
    pub key: String,
    pub servers: Vec<Server>,
    pub error: String,
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn base(cwd: &str) -> Vec<PathBuf> { crate::agents::skills::directories(Path::new(cwd)) }

/// These runtimes have native project loaders with no verified per-launch suppression.
pub fn ambient_conflict(cwd: &str, provider: &str, agy_print: bool) -> Option<PathBuf> {
    let files: &[&str] = match provider {
        "grok" => &[".mcp.json", ".cursor/mcp.json", ".grok/config.toml"],
        "muse" => &[".mcp.json"],
        "antigravity" if agy_print => &[".agents/mcp_config.json"],
        "antigravity" => &[".agents/mcp_config.json", ".gemini/settings.json"],
        _ => return None,
    };
    base(cwd).iter().flat_map(|dir| files.iter().map(move |f| dir.join(f))).find(|p| fs::symlink_metadata(p).is_ok())
}

pub fn scan(cwd: &str) -> Vec<Config> {
    let dirs = base(cwd);
    let Some(root) = dirs.first() else { return vec![] };
    let canonical = crate::files::kicad::canonical::main_of(&root.to_string_lossy());
    let mut configs = Vec::new();
    for dir in &dirs {
        for (_, name) in super::import::PROJECT_FILES {
            let path = dir.join(name);
            let Ok(real) = crate::plat::fs::canonicalize(&path) else { continue };
            if !real.starts_with(root) || !real.is_file() { continue }
            let mut bytes = Vec::new();
            let read = fs::File::open(&real).and_then(|f| f.take(LIMIT + 1).read_to_end(&mut bytes));
            let hash = digest(&bytes);
            let key = digest(format!("{canonical}\0{}\0{hash}", path.display()).as_bytes());
            let mut c = Config { path, hash, key, servers: vec![], error: String::new() };
            let parsed = if read.is_err() || bytes.len() as u64 > LIMIT {
                Err("Configuration is unreadable or exceeds 256 KiB".into())
            } else {
                parse_config(&bytes, dir, root, name, &c.hash)
            };
            match parsed {
                Ok(servers) if !servers.is_empty() => c.servers = servers,
                Ok(_) => continue,
                Err(error) => c.error = error,
            }
            configs.push(c);
        }
    }
    configs
}

fn parse_config(bytes: &[u8], dir: &Path, root: &Path, file: &str, hash: &str) -> Result<Vec<Server>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Configuration must be UTF-8")?;
    let value: Value = serde_json::from_str(&parse::strip_jsonc(text)).map_err(|e| e.to_string())?;
    if file == ".gemini/settings.json" && value.get("mcpServers").is_none() { return Ok(vec![]) }
    let found = parse::json_doc(&value, &[])?;
    if !found.notes.is_empty() { return Err(found.notes.join("\n")) }
    let map = value.get("mcpServers").or_else(|| value.get("servers")).and_then(Value::as_object);
    let mut servers = found.servers;
    for server in &mut servers {
        let raw = map.and_then(|m| m.get(&server.name)).ok_or("Expected a named MCP server map")?;
        server.env = bindings(raw.get("env").or_else(|| raw.get("environment")))?;
        server.headers = bindings(raw.get("headers").or_else(|| raw.get("http_headers")))?;
        // Keep executable, arguments and destination fixed to the reviewed configuration.
        // The workspace substitution has no access to ambient credentials.
        let workspace = root.to_str().ok_or("Workspace path must be UTF-8")?;
        for value in std::iter::once(&mut server.command).chain(server.args.iter_mut()).chain([&mut server.url, &mut server.cwd]) {
            *value = value.replace("${workspaceFolder}", workspace);
            if value.contains("${") || value.contains('\0') {
                return Err("Variable references belong in environment values or headers, except ${workspaceFolder}".into())
            }
        }
        if !server.remote() {
            let directory = if server.cwd.is_empty() { dir.to_path_buf() } else { dir.join(&server.cwd) };
            server.cwd = directory.to_string_lossy().into_owned();
        }
        let identity = digest(format!("{}\0{}\0{file}\0{hash}\0{}", root.display(), dir.display(), server.id).as_bytes());
        server.id = format!("project-{}", &identity[..24]);
        if let Some(problem) = server.problem() { return Err(problem) }
    }
    Ok(servers)
}

fn bindings(value: Option<&Value>) -> Result<Vec<Kv>, String> {
    let Some(value) = value else { return Ok(vec![]) };
    let map = value.as_object().ok_or("Environment and headers must be objects")?;
    map.iter().map(|(name, value)| {
        let text = value.as_str().ok_or("Environment and header values must be strings")?;
        if let Some((prefix, name_ref)) = text.split_once("${secret:") {
            let secret = name_ref.strip_suffix('}').ok_or("A vault reference must end the value")?;
            if prefix.contains("${") || !crate::settings::vault::valid_name(secret) { return Err("Invalid vault reference".into()) }
            return Ok(Kv::sec(name, secret, prefix))
        }
        // Keep interpolation unresolved until after trust has been checked.
        Ok(Kv::lit(name, text))
    }).collect()
}

pub fn environment_values(text: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut text = text;
    let mut out = String::new();
    while let Some((prefix, rest)) = text.split_once("${") {
        let (name, tail) = rest.split_once('}')?;
        let name = name.strip_prefix("env:").unwrap_or(name);
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') { return None }
        out.push_str(prefix);
        out.push_str(&env(name)?);
        text = tail;
    }
    out.push_str(text);
    Some(out)
}

fn trust_file(home: &Path, key: &str) -> PathBuf { home.join("mcp/project-trust").join(key) }

impl Config {
    fn state(&self, home: &Path) -> &'static str {
        if !self.error.is_empty() { return "Invalid" }
        match fs::read_to_string(trust_file(home, &self.key)).as_deref() {
            Ok("enabled") => "Enabled",
            Ok("disabled") => "Disabled",
            _ => "Untrusted",
        }
    }

    pub fn view(&self, home: &Path, provider: &str, provider_blocked: bool) -> Value {
        json!({"path": self.path, "sha256": self.hash, "state": self.state(home), "error": self.error,
            "servers": self.servers.iter().map(|s| json!({"name":s.name, "command":s.command,
                "args":s.args, "cwd":s.cwd, "url":s.url, "enabled":s.enabled,
                "status": self.server_status(s, home, provider, provider_blocked)})).collect::<Vec<_>>()})
    }

    fn server_status(&self, s: &Server, home: &Path, provider: &str, provider_blocked: bool) -> &'static str {
        let state = self.state(home);
        if !s.enabled { return "Disabled" }
        if state != "Enabled" { return state }
        if provider_blocked || matches!(provider, "pi" | "omp" | "muse") && s.transport == "sse" { return "Unavailable" }
        if !s.missing(&|n| crate::settings::vault::person_get(n).is_some()).is_empty() { return "Missing vault secret" }
        if s.env.iter().chain(&s.headers).filter(|kv| !kv.secret).any(|kv| environment_values(&kv.v, &|n| std::env::var(n).ok()).is_none()) { return "Missing environment value" }
        state
    }
}

pub fn act(home: &Path, cwd: &str, path: &str, hash: &str, action: &str) -> Result<(), String> {
    let c = scan(cwd).into_iter().find(|c| c.path == Path::new(path) && c.hash == hash)
        .ok_or("Configuration changed. Review it again before trusting it.")?;
    let state = c.state(home);
    let value = match (action, state) {
        ("trust", "Untrusted") | ("enable", "Disabled") => "enabled",
        ("disable", "Enabled") => "disabled",
        _ => return Err("That action does not apply to this configuration".into()),
    };
    let file = trust_file(home, &c.key);
    // A trust marker stays on the hub, outside the repo and out of synced settings.
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(file.parent().unwrap())?;
        let tmp = file.with_extension(crate::agents::proc::unique().to_string());
        fs::write(&tmp, value)?;
        let result = fs::rename(&tmp, &file);
        if result.is_err() { let _ = fs::remove_file(tmp); }
        result
    };
    write().map_err(|e| e.to_string())
}

pub fn selected(home: &Path, cwd: &str) -> Vec<Server> {
    scan(cwd).into_iter().filter(|c| c.state(home) == "Enabled")
        .flat_map(|c| c.servers.into_iter().filter_map(move |mut s| {
            if !s.enabled { return None }
            for kv in s.env.iter_mut().chain(s.headers.iter_mut()).filter(|kv| !kv.secret) {
                let raw = &kv.v;
                // Resolve project values without shell expansion or recursive evaluation.
                let value = environment_values(raw, &|name| std::env::var(name).ok())?;
                if value != *raw {
                    kv.v = value;
                }
            }
            Some(s)
        })).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_is_content_addressed_and_scoped_to_the_repository() {
        let temp = crate::plat::tmp::dir("project-mcp-trust");
        let home = temp.join("hub");
        let repo = temp.join("repo");
        let sub = repo.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir(repo.join(".git")).unwrap();
        let path = repo.join(".mcp.json");
        let config = r#"{"mcpServers":{"index":{"command":"indexer","args":["--quiet"],"env":{"TOKEN":"${secret:inventory-token}"}}}}"#;
        fs::write(&path, config).unwrap();
        let cwd = sub.to_str().unwrap();
        let name = path.to_str().unwrap();
        let first = scan(cwd).remove(0);
        assert!(selected(&home, cwd).is_empty());
        assert_eq!(first.view(&home, "claude", false)["state"], "Untrusted");
        assert_eq!(first.view(&home, "claude", false)["servers"][0]["command"], "indexer");
        assert!(!first.view(&home, "claude", false).to_string().contains("inventory-token"));
        assert!(act(&home, cwd, name, &first.hash, "enable").is_err());
        act(&home, cwd, name, &first.hash, "trust").unwrap();
        let chosen = selected(&home, cwd).remove(0);
        assert_eq!(chosen.env, vec![Kv::sec("TOKEN", "inventory-token", "")]);
        assert_eq!(chosen.cwd, repo.to_string_lossy());
        act(&home, cwd, name, &first.hash, "disable").unwrap();
        assert!(selected(&home, cwd).is_empty());
        act(&home, cwd, name, &first.hash, "enable").unwrap();
        fs::write(&path, config.replace("--quiet", "--strict")).unwrap();
        assert!(selected(&home, cwd).is_empty());
        assert!(act(&home, cwd, name, &first.hash, "trust").is_err());
        let changed = scan(cwd).remove(0);
        act(&home, cwd, name, &changed.hash, "trust").unwrap();
        assert_ne!(selected(&home, cwd)[0].id, chosen.id);
        let nested = sub.join(".mcp.json");
        fs::write(&nested, config.replace("--quiet", "--strict")).unwrap();
        assert_eq!(selected(&home, cwd).len(), 1, "an identical file in another directory has a different effective cwd");
        assert_eq!(scan(cwd).iter().find(|c| c.path == nested).unwrap().view(&home, "claude", false)["state"], "Untrusted");
        let another = temp.join("another");
        fs::create_dir(&another).unwrap();
        fs::write(another.join(".mcp.json"), config).unwrap();
        assert!(selected(&home, another.to_str().unwrap()).is_empty());
    }

    #[test]
    fn native_launch_conflicts_are_visible_even_before_a_file_can_be_reviewed() {
        let d = crate::plat::tmp::dir("project-mcp-provider");
        let cwd = d.to_string_lossy();
        fs::write(d.join(".mcp.json"), "not valid JSON").unwrap();
        assert!(ambient_conflict(&cwd, "claude", false).is_none());
        assert!(ambient_conflict(&cwd, "omp", false).is_none());
        assert_eq!(ambient_conflict(&cwd, "grok", false), Some(d.join(".mcp.json")));
        assert!(ambient_conflict(&cwd, "muse", false).is_some());
        fs::create_dir(d.join(".agents")).unwrap();
        fs::write(d.join(".agents/mcp_config.json"), "{}").unwrap();
        assert!(ambient_conflict(&cwd, "antigravity", true).is_some());
        assert!(ambient_conflict(&cwd, "antigravity", false).is_some());
    }

    #[test]
    fn formats_keep_exact_argv_and_do_not_evaluate_shell_syntax() {
        let dir = crate::plat::tmp::dir("project-mcp-formats");
        for (file, key) in [(".mcp.json", "mcpServers"), (".cursor/mcp.json", "mcpServers"),
            (".vscode/mcp.json", "servers"), (".gemini/settings.json", "mcpServers")] {
            let body = json!({(key): {"x": {"command":"example", "args":["with space", "$(literal)"],
                "env":{"A":"${A} ${env:B}", "B":"Bearer ${secret:example-secret}"}}}});
            let servers = parse_config(body.to_string().as_bytes(), &dir, &dir, file, "sample").unwrap();
            let s = &servers[0];
            assert_eq!(s.args, ["with space", "$(literal)"]);
            assert_eq!(s.env, [Kv::lit("A", "${A} ${env:B}"), Kv::sec("B", "example-secret", "Bearer ")]);
        }
        let resolve = |s: &str| match s { "A" => Some("${env:B}".into()), "B" => Some("value".into()), _ => None };
        assert_eq!(environment_values("${A} ${env:B}", &resolve).as_deref(), Some("${env:B} value"));
        for bad in ["${MISSING}", "${A", "${env:A:-default}", "${secret:A}"] {
            assert!(environment_values(bad, &resolve).is_none(), "{bad}");
        }
    }
}

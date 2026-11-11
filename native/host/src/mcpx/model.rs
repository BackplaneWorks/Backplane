//! The MCP servers every agent Backplane runs is given (docs/MCP.md): one list per hub, kept in `<home>/mcp/servers.json`.
//! A server is stdio (a command), http (streamable HTTP) or sse (the older HTTP + server-sent events). Its env values and headers are
//! plain text or a vault reference (`secret: true`, the value the vault name): a key is never written here in plain text.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// One environment variable or header. `secret`: `v` names a vault secret whose value is put in at launch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Kv { pub k: String, pub v: String, #[serde(default, skip_serializing_if = "is_false")] pub secret: bool,
    /// text put before a secret's value (`Bearer `), so the vault holds only the token
    #[serde(default, skip_serializing_if = "String::is_empty")] pub pre: String }
impl Kv {
    pub fn lit(k: &str, v: &str) -> Kv { Kv { k: k.into(), v: v.into(), ..Default::default() } }
    pub fn sec(k: &str, name: &str, pre: &str) -> Kv { Kv { k: k.into(), v: name.into(), secret: true, pre: pre.into() } }
}
fn is_false(b: &bool) -> bool { !*b }
fn yes() -> bool { true }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Server {
    /// what agents call it: `mcp__<id>__<tool>`; unique, `[a-z0-9_-]`, never `backplane` or `pcbparts`
    pub id: String,
    /// what the person reads
    #[serde(default)] pub name: String,
    /// "stdio", "http" or "sse"
    pub transport: String,
    #[serde(default)] pub command: String,
    #[serde(default)] pub args: Vec<String>,
    #[serde(default)] pub env: Vec<Kv>,
    #[serde(default)] pub cwd: String,
    #[serde(default)] pub url: String,
    #[serde(default)] pub headers: Vec<Kv>,
    #[serde(default = "yes")] pub enabled: bool,
    /// the providers that get it (claude, codex, grok, ...); empty is every one
    #[serde(default)] pub providers: Vec<String>,
    /// the project ids whose threads get it; empty is every project
    #[serde(default)] pub projects: Vec<String>,
    /// the bots that get it: None every bot, Some([]) none
    #[serde(default)] pub bots: Option<Vec<String>>,
    /// tools the agents are not shown (and may not call, for servers the hub runs)
    #[serde(default)] pub disabled_tools: Vec<String>,
    /// where it came from: "manual", "paste", "gallery:<id>", "import:<path>", "agent:<thread>"
    #[serde(default)] pub source: String,
    /// an imported server follows its source file (import.rs `sync`)
    #[serde(default)] pub sync: bool,
    /// the gallery's icon key or "" (the client draws the server's initial)
    #[serde(default)] pub icon: String,
    #[serde(default)] pub added_at: u64,
}

pub const RESERVED: [&str; 2] = ["backplane", "pcbparts"];
pub const TRANSPORTS: [&str; 3] = ["stdio", "http", "sse"];
/// The providers a scope can name (agents/providers): every one Backplane runs.
pub const PROVIDERS: [&str; 9] = ["claude", "codex", "antigravity", "grok", "opencode", "muse", "pi", "omp", "agy"];

pub fn valid_id(id: &str) -> bool { !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_') && !RESERVED.contains(&id) }
/// A name made into an id: lower case, other characters as '-', trimmed; "server" when nothing is left.
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) { if c.is_ascii_alphanumeric() || c == '_' { s.push(c) } else if !s.ends_with('-') { s.push('-') } }
    let s: String = s.trim_matches('-').chars().take(40).collect::<String>().trim_matches('-').to_string();
    if s.is_empty() { "server".into() } else if RESERVED.contains(&s.as_str()) { format!("{s}-2") } else { s }
}
/// `id` made unique among `taken`: id, id-2, id-3...
pub fn unique(id: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == id) && !RESERVED.contains(&id) { return id.to_string() }
    (2..).map(|n| { let base: String = id.chars().take(36).collect(); format!("{base}-{n}") }).find(|c| !taken.iter().any(|t| t == c)).unwrap()
}

/// A key name that holds a secret by its look (API_KEY, GITHUB_TOKEN, Authorization, X-Api-Key, PASSWORD...).
pub fn secretish(k: &str) -> bool {
    const WORDS: [&str; 13] = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "PASS", "CREDENTIAL", "CREDENTIALS", "PAT", "BEARER", "COOKIE", "AUTH", "AUTHORIZATION"];
    k.to_ascii_uppercase().split(|c: char| !c.is_ascii_alphanumeric()).any(|p| WORDS.contains(&p) || p.ends_with("KEY") && p.len() > 3 && !p.ends_with("HOTKEY") || p.ends_with("TOKEN") || p.ends_with("SECRET"))
}
/// A value that is a placeholder, not the real thing: `${VAR}`, `${input:x}`, `<YOUR_KEY>`, `YOUR_API_KEY`, `xxx`, "".
pub fn placeholder(v: &str) -> bool {
    let t = v.trim(); let u = t.to_ascii_uppercase();
    t.is_empty() || (t.starts_with("${") && t.ends_with('}')) || (t.starts_with('<') && t.ends_with('>')) || u.starts_with("YOUR") || u.contains("YOUR_") || u.contains("YOUR-") || u.contains("<YOUR") || t.chars().all(|c| c == 'x' || c == 'X' || c == '*' || c == '.')
        || u == "CHANGEME" || u.starts_with("REPLACE") || u.contains("HERE>") || u.ends_with("_HERE")
}
/// The vault name a server's secret goes in: `mcp-<id>-<key>` in the vault's alphabet.
pub fn vault_name(id: &str, key: &str) -> String {
    let k: String = key.chars().flat_map(char::to_lowercase).map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '.' { c } else { '-' }).collect();
    let n = format!("mcp-{id}-{}", k.trim_matches('-')); n.chars().take(64).collect()
}

impl Server {
    pub fn title(&self) -> &str { if self.name.is_empty() { &self.id } else { &self.name } }
    pub fn remote(&self) -> bool { self.transport != "stdio" }
    /// Why it cannot be kept as it is, in the person's words; None when it can.
    pub fn problem(&self) -> Option<String> {
        if !valid_id(&self.id) { return Some(if RESERVED.contains(&self.id.as_str()) { format!("{} is Backplane's own name: choose another", self.id) } else { "A name of lower-case letters, digits, - and _ (40 at most)".into() }) }
        if !TRANSPORTS.contains(&self.transport.as_str()) { return Some("The transport is stdio, http or sse".into()) }
        if self.transport == "stdio" && self.command.trim().is_empty() { return Some("A stdio server needs a command".into()) }
        if self.remote() && !(self.url.starts_with("https://") || self.url.starts_with("http://")) { return Some("A remote server needs an http or https URL".into()) }
        if self.remote() && self.url.chars().any(|c| c.is_whitespace() || c.is_control()) { return Some("The URL has a space in it".into()) }
        for kv in self.env.iter().chain(self.headers.iter()) {
            if kv.k.trim().is_empty() || kv.k.chars().any(|c| c.is_control() || c == '=') { return Some(format!("{:?} is not a variable or header name", kv.k)) }
            if kv.secret && !crate::settings::vault::valid_name(&kv.v) { return Some(format!("{} names a secret {:?} the vault cannot hold", kv.k, kv.v)) }
        }
        for p in &self.providers { if !PROVIDERS.contains(&p.as_str()) { return Some(format!("{p} is not a provider Backplane runs")) } }
        None
    }
    /// The vault names it needs.
    pub fn secrets(&self) -> Vec<String> { self.env.iter().chain(self.headers.iter()).filter(|k| k.secret).map(|k| k.v.clone()).collect() }
    /// The secrets it needs that the vault does not hold.
    pub fn missing(&self, have: &dyn Fn(&str) -> bool) -> Vec<String> { self.secrets().into_iter().filter(|n| !have(n)).collect() }
    /// One line for the card: the command or the URL.
    pub fn line(&self) -> String {
        if self.remote() { return self.url.clone() }
        let mut s = self.command.clone(); for a in &self.args { s.push(' '); if a.contains(' ') { s.push_str(&format!("\"{a}\"")) } else { s.push_str(a) } } s
    }
    /// Whether a thread of `provider`, project `project` and bot `bot` ("" for none) gets it (on, and in its scope).
    pub fn applies(&self, provider: &str, project: &str, bot: &str) -> bool {
        self.enabled && (self.providers.is_empty() || self.providers.iter().any(|p| p == provider || (p == "antigravity" && provider == "agy") ))
            && (self.projects.is_empty() || self.projects.iter().any(|p| p == project))
            && (bot.is_empty() || self.bots.as_ref().map_or(true, |b| b.iter().any(|x| x == bot)))
    }
    /// The server for an agent: what it reads (no vault values), as JSON for mcp_list.
    pub fn public(&self) -> Value {
        let kvs = |xs: &[Kv]| Value::Object(xs.iter().map(|k| (k.k.clone(), if k.secret { json!({"secret": k.v}) } else { json!(k.v) })).collect());
        let mut o = json!({"id": self.id, "name": self.title(), "transport": self.transport, "enabled": self.enabled});
        if self.remote() { o["url"] = json!(self.url); o["headers"] = kvs(&self.headers) } else { o["command"] = json!(self.command); o["args"] = json!(self.args); o["env"] = kvs(&self.env); if !self.cwd.is_empty() { o["cwd"] = json!(self.cwd) } }
        o["providers"] = json!(self.providers); o["projects"] = json!(self.projects); o["bots"] = json!(self.bots); o["disabled_tools"] = json!(self.disabled_tools); o["source"] = json!(self.source);
        o
    }
    /// What makes two servers the same server (an import or a paste that is already here): the command and its arguments, or the URL.
    pub fn same_as(&self, o: &Server) -> bool { self.transport == o.transport && if self.remote() { self.url.trim_end_matches('/') == o.url.trim_end_matches('/') } else { self.command == o.command && self.args == o.args } }
}

/// The list as it is kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct List { #[serde(default)] pub servers: Vec<Server> }
pub fn path(home: &Path) -> PathBuf { home.join("mcp").join("servers.json") }
impl List {
    pub fn load(home: &Path) -> List {
        match std::fs::read_to_string(path(home)) { Ok(t) => serde_json::from_str::<Value>(&t).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default(), Err(_) => List::default() }
    }
    /// Written whole through a temporary file (0600: it may name a URL with a token in its query).
    pub fn save(&self, home: &Path) -> std::io::Result<()> {
        let p = path(home); if let Some(d) = p.parent() { std::fs::create_dir_all(d)?; }
        let tmp = p.with_extension("tmp");
        let body = serde_json::to_string_pretty(&json!({"v": 1, "servers": self.servers}))?;
        std::fs::write(&tmp, body)?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)); }
        std::fs::rename(&tmp, &p)
    }
    pub fn get(&self, id: &str) -> Option<&Server> { self.servers.iter().find(|s| s.id == id) }
    pub fn ids(&self) -> Vec<String> { self.servers.iter().map(|s| s.id.clone()).collect() }
    /// The servers a thread gets, in the list's order.
    pub fn for_thread(&self, provider: &str, project: &str, bot: &str) -> Vec<&Server> { self.servers.iter().filter(|s| s.applies(provider, project, bot)).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn stdio(id: &str) -> Server { Server { id: id.into(), transport: "stdio".into(), command: "npx".into(), args: vec!["-y".into(), "x".into()], enabled: true, ..Default::default() } }
    #[test] fn ids_are_slugs_never_backplanes_own() {
        assert!(valid_id("fetch") && valid_id("st-mcu_2")); assert!(!valid_id("backplane") && !valid_id("Fetch") && !valid_id("") && !valid_id("a b"));
        assert_eq!(slug("Microchip MCP Server!"), "microchip-mcp-server"); assert_eq!(slug("  "), "server"); assert_eq!(slug("backplane"), "backplane-2"); assert_eq!(slug("Ünï code"), "n-code");
        assert_eq!(unique("fetch", &["fetch".into(), "fetch-2".into()]), "fetch-3"); assert_eq!(unique("new", &[]), "new");
    }
    #[test] fn secrets_are_told_by_name_and_placeholders_by_look() {
        for k in ["GITHUB_PERSONAL_ACCESS_TOKEN", "API_KEY", "Authorization", "X-Api-Key", "OPENAI_API_KEY", "DB_PASSWORD", "CLIENT_SECRET"] { assert!(secretish(k), "{k}") }
        for k in ["PATH", "NODE_ENV", "DEBUG", "HOME", "LOG_LEVEL", "KEYBOARD_LAYOUT", "Accept"] { assert!(!secretish(k), "{k}") }
        for v in ["${GITHUB_TOKEN}", "${input:token}", "<YOUR_TOKEN>", "YOUR_API_KEY", "your-key-here", "xxxx", ""] { assert!(placeholder(v), "{v}") }
        for v in ["ghp_abcdef123456", "sk-live-9", "Bearer abc"] { assert!(!placeholder(v), "{v}") }
        assert_eq!(vault_name("github", "GITHUB_PERSONAL_ACCESS_TOKEN"), "mcp-github-github_personal_access_token"); assert_eq!(vault_name("x", "X-Api-Key"), "mcp-x-x-api-key");
        assert!(crate::settings::vault::valid_name(&vault_name("x", "Authorization")));
    }
    #[test] fn a_server_is_checked_before_it_is_kept() {
        assert_eq!(stdio("fetch").problem(), None);
        assert!(stdio("backplane").problem().unwrap().contains("Backplane's own"));
        let mut s = stdio("a"); s.command = " ".into(); assert!(s.problem().unwrap().contains("command"));
        let r = Server { id: "r".into(), transport: "http".into(), url: "ftp://x".into(), ..Default::default() }; assert!(r.problem().unwrap().contains("URL"));
        let mut s = stdio("a"); s.env.push(Kv::sec("T", "Not A Name", "")); assert!(s.problem().unwrap().contains("vault"));
        let mut s = stdio("a"); s.providers = vec!["gpt".into()]; assert!(s.problem().is_some());
    }
    #[test] fn scope_picks_the_threads_that_get_it() {
        let mut s = stdio("a"); assert!(s.applies("claude", "p1", "") && s.applies("codex", "p2", "bot1"));
        s.providers = vec!["codex".into()]; assert!(!s.applies("claude", "p1", "") && s.applies("codex", "p1", ""));
        s.projects = vec!["p2".into()]; assert!(!s.applies("codex", "p1", "") && s.applies("codex", "p2", ""));
        s.bots = Some(vec![]); assert!(!s.applies("codex", "p2", "b") && s.applies("codex", "p2", ""), "no bot, a person's thread still gets it");
        s.bots = Some(vec!["b".into()]); assert!(s.applies("codex", "p2", "b") && !s.applies("codex", "p2", "c"));
        s.enabled = false; assert!(!s.applies("codex", "p2", ""));
    }
    #[test] fn the_list_round_trips_and_shows_no_secret_value() {
        let d = crate::plat::tmp::uncreated("mcpx-model");
        let mut s = stdio("gh"); s.env = vec![Kv::sec("GITHUB_TOKEN", "mcp-gh-github_token", ""), Kv::lit("LOG", "1")];
        let l = List { servers: vec![s.clone()] }; l.save(&d).unwrap(); assert_eq!(List::load(&d), l);
        let raw = std::fs::read_to_string(path(&d)).unwrap(); assert!(raw.contains("\"secret\": true") && raw.contains("mcp-gh-github_token"));
        assert_eq!(s.public()["env"], json!({"GITHUB_TOKEN": {"secret": "mcp-gh-github_token"}, "LOG": "1"}));
        assert_eq!(s.missing(&|n| n == "other"), vec!["mcp-gh-github_token".to_string()]);
        assert_eq!(List::load(&d.join("nope")), List::default());
    }
}

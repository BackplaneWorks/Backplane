//! The provider-owned account lifecycle: named logins ("profiles") of Claude and Codex, each in a private
//! store of its own under `<home>/providers/accounts-v1/<profile>[/<store>]`. Ops (`request`): list, add, status, login (a sign-in attempt that shows a link, and for
//! Claude takes the pasted code), respond, cancel, rename, disconnect, remove. No inference request is ever made: status asks the CLI who is signed in
//! (`claude auth status`, Codex's `account/read`), the allowance reads are the providers' own read-only usage requests, and nothing but allowlisted metadata
//! leaves this module (raw CLI output is never kept, logged or sent; a profile's credentials never leave the CLI's store).
//! Every child is a provider CLI resolved like the agents' (the stand-in folder in a test hub, else PATH) and made through the process gate (Kind::Agent).
//! Any failure of an op answers the same generic message the JS hub gives; what a sign-in failed on is one of a few fixed sentences.
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration}; use crate::plat::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin}, sync::{mpsc, oneshot}};

pub const GENERIC: &str = "Account operation failed. Check its current status before retrying.";
const FAILURES: &[(&str, &str)] = &[
    ("code_rejected", "Claude did not accept that authentication code. It may be incomplete, expired, already used, or from an earlier sign-in. Choose Connect / Login and copy the new code from that sign-in page."),
    ("network", "Claude could not reach its sign-in service. Check the connection and try Connect / Login again."),
    ("storage", "Claude could not save the sign-in in this account's private store. Check its directory permissions and try again."),
    ("cli_start", "Claude Code could not start. Check the provider CLI installation and try again."),
    ("directory", "Claude did not report this account's private directory. Update Claude Code and try again."),
    ("verification", "The provider sign-in finished, but the saved subscription account could not be verified. Try Connect / Login again."),
];
const SIGNIN_DEFAULT: &str = "Sign-in could not be verified. Retry or check the provider CLI installation.";
const DIRECTORY_ERR: &str = "Claude must report its isolated account directory (Claude Code 2.1.268 or later)";

fn fb_auth(s: &str) -> bool { super::fallback::auth_text(s) }
fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64) }
/// A name shown to the person: no control characters, at most 64, trimmed.
pub fn label(v: &str) -> String { v.chars().filter(|c| !c.is_control()).take(64).collect::<String>().trim().to_string() }
pub fn valid_id(s: &str) -> bool { s.len() == 36 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase() || b == b'-') }
pub fn new_id() -> String {
    let mut b = [0u8; 16]; let _ = crate::plat::fs::random(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40; b[8] = (b[8] & 0x3f) | 0x80; let h = hex::encode(b);
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}
fn fingerprint(profile: &str, identity: &Value) -> String { use sha2::{Digest, Sha256}; hex::encode(Sha256::digest(json!([profile, identity]).to_string().as_bytes())) }
fn failure_text(reason: &str) -> String { FAILURES.iter().find(|(k, _)| *k == reason).map_or(SIGNIN_DEFAULT, |(_, v)| *v).to_string() }

/// `application/x-www-form-urlencoded` decoding of one query component: `+` is a space, `%XX` a byte (what `URL.searchParams` does before keys are compared).
fn form_decode(c: &str) -> String {
    let b = c.as_bytes(); let mut out = Vec::with_capacity(b.len()); let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => { out.push(b' '); i += 1 }
            b'%' if i + 2 < b.len() + 0 && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => { out.push(u8::from_str_radix(&c[i + 1..i + 3], 16).unwrap_or(b'%')); i += 3 }
            x => { out.push(x); i += 1 }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
/// The decoded query pairs of a link (the fragment is not part of the query).
pub fn query_pairs(url: &str) -> Vec<(String, String)> {
    let q = url.split_once('?').map(|(_, q)| q.split('#').next().unwrap_or("")).unwrap_or("");
    q.split('&').filter(|s| !s.is_empty()).map(|kv| { let (k, v) = kv.split_once('=').unwrap_or((kv, "")); (form_decode(k), form_decode(v)) }).collect()
}
/// A sign-in link the person may be sent to: https on the provider's own hosts, no credentials in it (accounts.js `url`).
pub fn sign_in_url(value: &str, provider: &str) -> String {
    let Some(rest) = value.strip_prefix("https://") else { return String::new() };
    let (host_port, path_q) = match rest.find(['/', '?', '#']) { Some(i) => (&rest[..i], &rest[i..]), None => (rest, "") };
    if host_port.contains('@') || host_port.contains(':') || value.len() > 8192 || value.chars().any(|c| c.is_whitespace() || c.is_control()) { return String::new() }
    let hosts: &[&str] = if provider == "codex" { &["auth.openai.com", "chatgpt.com"] } else { &["claude.ai", "claude.com", "platform.claude.com", "console.anthropic.com"] };
    if !hosts.contains(&host_port) { return String::new() }
    let pairs = query_pairs(value);
    if pairs.iter().any(|(k, _)| k == "access_token" || k == "refresh_token") { return String::new() }
    let codes: Vec<&String> = pairs.iter().filter(|(k, _)| k == "code").map(|(_, v)| v).collect();
    let path = path_q.split(['?', '#']).next().unwrap_or("");
    // Claude Code's authorization link uses code=true to request its code entry flow; that boolean is not an OAuth callback's code.
    let claude_flow = provider == "claude" && host_port == "claude.com" && path == "/cai/oauth/authorize" && codes.len() == 1 && codes[0].as_str() == "true";
    if !codes.is_empty() && !claude_flow { return String::new() }
    value.to_string()
}

/// A provider CLI's spawn, again for a moment when exec says ETXTBSY: a CLI written just before it runs (an update in place, a test's stand-in) is "busy" while any
/// fork made in that instant still holds the writer's descriptor, until that fork execs or exits. Under load other threads fork all the time, and the sign-in
/// failed with "could not start" (accounts_io tests in parallel: 3 of 24 runs before, 0 of 88 after, one ETXTBSY retry seen).
async fn spawn(c: &mut tokio::process::Command) -> std::io::Result<Child> {
    let mut n = 0;
    loop {
        match c.spawn() {
            Err(e) if e.raw_os_error() == Some(crate::plat::errno::ETXTBSY) && n < 40 => { n += 1; if cfg!(test) { eprintln!("accounts_io: ETXTBSY on spawn, retry {n}") } tokio::time::sleep(Duration::from_millis(25)).await }
            r => { if let Err(e) = &r { if cfg!(test) { eprintln!("accounts_io: spawn {:?} failed: {e} (os {:?})", c.as_std().get_program(), e.raw_os_error()) } } return r }
        }
    }
}
fn unset_env(k: &str) -> bool {
    const EXACT: &[&str] = &["CODEX_HOME", "CODEX_SQLITE_HOME", "CODEX_ACCESS_TOKEN", "OPENAI_API_KEY", "OPENAI_BASE_URL", "ACCESS_TOKEN", "OPENAI_IDENTITY_TOKEN_FILE", "OPENAI_FEDERATION_RULE_ID", "OPENAI_ORGANIZATION_ID", "CODEX_AUTH_TOKEN",
        "CLAUDE_CONFIG_DIR", "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", "CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CODE_OAUTH_REFRESH_TOKEN", "CLAUDE_CODE_OAUTH_SCOPES", "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "ANTHROPIC_CUSTOM_HEADERS",
        "ANTHROPIC_PROFILE", "ANTHROPIC_CONFIG_DIR", "ANTHROPIC_FEDERATION_RULE_ID", "ANTHROPIC_ORGANIZATION_ID", "ANTHROPIC_IDENTITY_TOKEN_FILE", "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", "CLAUDE_CODE_USE_FOUNDRY",
        "CLAUDE_CODE_USE_ANTHROPIC_AWS", "ANTHROPIC_AWS_API_KEY", "CLAUDE_CODE_API_KEY_HELPER", "CLAUDE_CODE_PROJECT_DIR_NAME",
        // where Claude Code keeps its credentials and the lock its refreshes take; inherited, every profile would share one login and one lock
        "CLAUDE_SECURESTORAGE_CONFIG_DIR"];
    EXACT.contains(&k) || k.starts_with("OPENAI_") || k.starts_with("ANTHROPIC_") || k.starts_with("CLAUDE_CODE_USE_")
        || (k.starts_with("CODEX_") && (k.contains("TOKEN") || k.contains("AUTH") || k.contains("WIF")))
        || ["CLAUDE_CODE_OAUTH", "CLAUDE_CODE_API_KEY", "CLAUDE_CODE_AUTH", "CLAUDE_CODE_PROFILE", "CLAUDE_CODE_SESSION"].iter().any(|p| k.starts_with(p))
}

#[derive(Clone, Debug)]
pub struct Profile { pub id: String, pub provider: String, pub label: String, pub epoch: u64, pub fingerprint: String, pub store: String, pub state: String, pub identity: Value, pub catalog: Vec<Value>, pub allowance: Option<Value>, pub checked_at: u64, pub message: String,
    /// When its sign-in was found dead (0: it was not): kept in the registry, so a restart still knows it must be signed in again.
    pub expired_at: u64,
    /// When the provider last answered a request made with this login (its usage read): the proof a dead sign-in works again. Not kept.
    pub proven_at: u64 }
impl Profile {
    fn public(&self) -> Value {
        let allowance = self.allowance.as_ref().map(|a| { let mut a = a.clone(); if now_s().saturating_sub(a["at"].as_u64().unwrap_or(0)) > 300 { a["state"] = json!("stale") } a });
        json!({"id":self.id,"provider":self.provider,"label":self.label,"epoch":self.epoch,"state":self.state,"identity":self.identity,"checkedAt":self.checked_at,"catalog":self.catalog,"allowance":allowance,"message":self.message,"expiredAt":self.expired_at})
    }
}
// ---- the sign-in state: what a profile's state becomes on each fact ----

/// What was learned of a profile's login.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fact {
    /// A turn's provider said the login is dead (an expired or revoked OAuth session, a 401, "please run /login").
    TurnRefused,
    /// The status check: the CLI says who is signed in, and the provider answered a request made with the login (its usage read).
    Proven,
    /// The CLI says who is signed in, and the provider refused the request with words of a dead login.
    Refused,
    /// The CLI says who is signed in, and nothing proves the login works or is dead (no request was made, or it failed for another reason).
    Unproven,
    /// The CLI says nobody is signed in, or could not say.
    NoLogin,
    /// A sign-in attempt finished and was verified.
    SignedIn,
}
/// The sign-in state machine. A login found dead stays "expired" until a sign-in or a request the provider answered says otherwise: `claude auth status` only reads the
/// credentials file, which still names the account after its session died (on Nov 7 the inventory said "connected" for hours while every turn failed).
pub fn after(state: &str, f: Fact) -> &'static str {
    match (state, f) {
        (_, Fact::SignedIn) => "connected",
        ("connecting", _) => "connecting",
        ("disconnected", Fact::TurnRefused) => "disconnected",
        (_, Fact::TurnRefused) | (_, Fact::Refused) => "expired",
        ("expired", Fact::Unproven) | ("expired", Fact::NoLogin) => "expired",
        (_, Fact::NoLogin) => "disconnected",
        (_, Fact::Proven) | (_, Fact::Unproven) => "connected",
    }
}
/// What the person is told of an expired login, and what a turn that cannot start on it fails with (the failover reads "failed to authenticate" as a dead login).
pub const EXPIRED_SHORT: &str = "Sign-in expired. Sign in again.";
pub const EXPIRED: &str = "Failed to authenticate: this account's sign-in expired. Sign in again in Settings, Accounts.";
/// A checked profile replaces the one in the registry, unless it was found dead while the check ran and the check proved nothing newer.
fn merge_checked(slot: &Profile, mut np: Profile) -> Profile {
    if slot.state == "expired" && np.state != "expired" && np.proven_at <= slot.expired_at { np.state = "expired".into(); np.expired_at = slot.expired_at; np.message = EXPIRED_SHORT.into(); }
    np
}

#[derive(Clone)]
struct Attempt { id: String, profile: String, state: String, url: String, user_code: String, expires_at: u64, message: String, previous_state: String, store: String }

enum Cmd { Respond(String), Cancel }
#[derive(Default)]
struct Inner { profiles: Vec<Profile>, attempts: Vec<Attempt>, jobs: HashMap<String, String>, cmds: HashMap<String, mpsc::UnboundedSender<Cmd>>, revision: u64, load_error: String, default_allowance: Option<Value>, default_identity: String, refresh_at: HashMap<String, u64>, busy: HashMap<String, usize>, launching: HashMap<String, usize>, launch_locks: HashMap<String, Arc<tokio::sync::Mutex<()>>> }

/// A launch waiting or running on a profile; dropped (finished, refused or its future dropped) it leaves the queue, so the profile is never stranded.
struct Queued { me: Arc<Accounts>, id: String }
impl Drop for Queued { fn drop(&mut self) { let mut g = self.me.inner.lock().unwrap(); if let Some(n) = g.launching.get_mut(&self.id) { *n = n.saturating_sub(1); if *n == 0 { g.launching.remove(&self.id); } } } }
pub struct Accounts { root: PathBuf, inner: Mutex<Inner>, resolve: Arc<dyn Fn(&str) -> Result<PathBuf, String> + Send + Sync> }

fn private_dir(dir: &Path) -> std::io::Result<()> {
    if let Some(parent) = dir.parent() { if !parent.exists() { std::fs::DirBuilder::new().recursive(true).mode(0o700).create(parent)?; } }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) { Ok(()) => {} Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {} Err(e) => return Err(e) }
    let m = std::fs::symlink_metadata(dir)?;
    use crate::plat::fs::MetadataExt;
    if !m.is_dir() || m.file_type().is_symlink() || m.uid() != crate::plat::fs::uid() { return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unsafe provider directory")) }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

// ---- child processes ----

/// Codex's app-server over JSON lines on stdio: requests with ids, notifications to a channel. Closed after its timeout.
struct Rpc { stdin: ChildStdin, next: u64, pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, ()>>>>>, notes: mpsc::UnboundedReceiver<Value>, child: Child }
impl Rpc {
    async fn open(prog: &Path, env: &[(String, String)], named: bool) -> Result<Rpc, ()> {
        let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Agent, prog);
        c.args(["-c", "cli_auth_credentials_store=\"file\"", "-c", "features.daemon_auto_start=false", "-c", "analytics.enabled=false", "app-server", "--listen", "stdio://"]);
        apply_env(&mut c, env, named);
        c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
        let mut child = spawn(&mut c).await.map_err(|_| ())?;
        let (stdin, stdout) = (child.stdin.take().ok_or(())?, child.stdout.take().ok_or(())?);
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, ()>>>>> = Default::default();
        let (tx, notes) = mpsc::unbounded_channel();
        let p2 = pending.clone();
        tokio::spawn(async move {
            let mut r = BufReader::new(stdout); let mut line = String::new(); let mut total = 0usize;
            loop {
                line.clear();
                match r.read_line(&mut line).await { Ok(0) | Err(_) => break, Ok(n) => { total += n; if total > 4_000_000 { break } } }
                let Ok(m) = serde_json::from_str::<Value>(line.trim()) else { continue }; // non-protocol output carries no authority
                if let Some(id) = m.get("id").and_then(Value::as_u64) {
                    if let Some(s) = p2.lock().unwrap().remove(&id) { let _ = s.send(if m.get("error").is_some() { Err(()) } else { Ok(m.get("result").cloned().unwrap_or(Value::Null)) }); }
                } else if m.get("method").and_then(Value::as_str).is_some() { let _ = tx.send(m); }
            }
            for (_, s) in p2.lock().unwrap().drain() { let _ = s.send(Err(())); }
        });
        Ok(Rpc { stdin, next: 0, pending, notes, child })
    }
    async fn request(&mut self, method: &str, params: Value, secs: u64) -> Result<Value, ()> {
        self.next += 1; let id = self.next; let (tx, rx) = oneshot::channel(); self.pending.lock().unwrap().insert(id, tx);
        self.stdin.write_all(format!("{}\n", json!({"id":id,"method":method,"params":params})).as_bytes()).await.map_err(|_| ())?;
        tokio::time::timeout(Duration::from_secs(secs), rx).await.map_err(|_| ())?.map_err(|_| ())?
    }
    async fn initialize(&mut self) -> Result<(), ()> {
        self.request("initialize", json!({"clientInfo":{"name":"backplane_accounts","version":"1"},"capabilities":{"experimentalApi":true}}), 12).await?;
        self.stdin.write_all(b"{\"method\":\"initialized\",\"params\":{}}\n").await.map_err(|_| ())
    }
    async fn close(mut self) { let _ = self.child.start_kill(); let _ = self.child.wait().await; }
}
/// The variables that point a CLI at one profile's store (`environment` sets them). The default login never inherits them: a hub started from a shell that
/// ran under a profile (an agent's own Bash, after failover moved it) would otherwise run every default-login thread, and read its usage, under that profile.
pub const STORE_ENV: &[&str] = &["CLAUDE_CONFIG_DIR", "CODEX_HOME"];
/// The host's environment, minus every provider credential variable when the child is a NAMED profile's (its own store is then put in). The CLI's own login
/// (`named` false) keeps the host's environment, as the JS hub's `environment(null)` does, except a profile's store ([`STORE_ENV`]).
fn apply_env(c: &mut tokio::process::Command, env: &[(String, String)], named: bool) {
    if named { for (k, _) in std::env::vars_os() { if let Some(k) = k.to_str() { if unset_env(k) { c.env_remove(k); } } } } else { for k in STORE_ENV { c.env_remove(k); } }
    for (k, v) in env { c.env(k, v); }
}

impl Accounts {
    pub fn open(home: &Path, resolve: Arc<dyn Fn(&str) -> Result<PathBuf, String> + Send + Sync>) -> Accounts {
        let canon = crate::plat::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
        let root = canon.join("providers").join("accounts-v1");
        let mut inner = Inner::default();
        if private_dir(&canon.join("providers")).and_then(|_| private_dir(&root)).is_err() { inner.load_error = "The private account registry could not be verified.".into() }
        else {
            let index = root.join("profiles.json");
            match std::fs::symlink_metadata(&index) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => inner.load_error = "The private account registry could not be verified.".into(),
                Ok(m) => {
                    let ok = m.is_file() && !m.file_type().is_symlink() && m.len() <= 131072 && m.permissions().mode() & 0o077 == 0;
                    let parsed = if ok { std::fs::read_to_string(&index).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) } else { None };
                    let list = parsed.as_ref().filter(|s| s["version"] == 1).and_then(|s| s["profiles"].as_array()).filter(|a| a.len() <= 40);
                    match list {
                        Some(a) if a.iter().all(|p| valid_id(p["id"].as_str().unwrap_or("")) && matches!(p["provider"].as_str(), Some("claude" | "codex")) && p["epoch"].as_u64().map_or(false, |e| e >= 1)) => {
                            for p in a { inner.profiles.push(Profile { id: p["id"].as_str().unwrap().into(), provider: p["provider"].as_str().unwrap().into(), label: label(p["label"].as_str().unwrap_or("")), epoch: p["epoch"].as_u64().unwrap(),
                                fingerprint: p["fingerprint"].as_str().unwrap_or("").into(), store: p["store"].as_str().filter(|s| valid_id(s)).unwrap_or("").into(), state: if p["expiredAt"].as_u64().unwrap_or(0) > 0 { "expired".into() } else { "unchecked".into() }, identity: json!({}), catalog: vec![], allowance: None, checked_at: 0,
                                message: if p["expiredAt"].as_u64().unwrap_or(0) > 0 { EXPIRED_SHORT.into() } else { String::new() }, expired_at: p["expiredAt"].as_u64().unwrap_or(0), proven_at: 0 }) }
                        }
                        _ => inner.load_error = "The private account registry could not be verified.".into(),
                    }
                }
            }
        }
        Accounts { root, inner: Mutex::new(inner), resolve }
    }
    /// The registry's folder (`<home>/providers/accounts-v1`).
    pub fn root(&self) -> &Path { &self.root }
    /// Profile ids in the registry now.
    pub fn ids(&self) -> Vec<String> { self.inner.lock().unwrap().profiles.iter().map(|p| p.id.clone()).collect() }
    /// The 0.14 import: profiles of another home's registry (its `profiles.json` entries) whose folders were copied under `root` first. Each valid
    /// one whose id is not here yet is added; with `signed_in` false its login was not copied, so it reads expired and asks for a sign-in.
    /// Ok(how many were added); Err when the registry is locked or could not be written (nothing changed then).
    pub fn adopt(&self, list: &[Value], signed_in: bool) -> Result<usize, ()> {
        if !self.inner.lock().unwrap().load_error.is_empty() { return Err(()) }
        let now = now_s(); let mut n = 0;
        self.mutate_save(|g| {
            for p in list {
                let (id, provider, epoch) = (p["id"].as_str().unwrap_or(""), p["provider"].as_str().unwrap_or(""), p["epoch"].as_u64().unwrap_or(0));
                // one here already: with its login now copied, a sign-in it was asked for is no longer owed (the next check says how it stands)
                if let Some(x) = g.profiles.iter_mut().find(|x| x.id == id) { if signed_in && x.state == "expired" { x.state = "unchecked".into(); x.expired_at = 0; x.message = String::new(); n += 1; } continue }
                if !valid_id(id) || !matches!(provider, "claude" | "codex") || epoch < 1 || g.profiles.len() >= 40 { continue }
                let store = p["store"].as_str().filter(|s| valid_id(s)).unwrap_or("").to_string();
                let (state, expired_at, message) = if signed_in { ("unchecked".to_string(), 0, String::new()) } else { ("expired".to_string(), now, EXPIRED_SHORT.to_string()) };
                g.profiles.push(Profile { id: id.into(), provider: provider.into(), label: label(p["label"].as_str().unwrap_or("")), epoch, fingerprint: p["fingerprint"].as_str().unwrap_or("").into(), store, state, identity: json!({}), catalog: vec![], allowance: None, checked_at: 0, message, expired_at, proven_at: 0 });
                n += 1;
            }
        })?;
        Ok(n)
    }
    /// Writes the registry (temp file, then rename). The revision moves only when the file is good; a failure is the op's failure.
    fn save(&self, g: &mut Inner) -> Result<(), ()> {
        let body = json!({"version":1,"profiles":g.profiles.iter().map(|p| { let mut o = json!({"id":p.id,"provider":p.provider,"label":p.label,"epoch":p.epoch,"fingerprint":p.fingerprint,"store":p.store}); if p.expired_at > 0 && p.state == "expired" { o["expiredAt"] = json!(p.expired_at) } o }).collect::<Vec<_>>()}).to_string();
        let tmp = self.root.join(format!("profiles.json.{}.tmp", new_id()));
        let w = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp).and_then(|mut f| { std::io::Write::write_all(&mut f, body.as_bytes())?; f.sync_all() }).and_then(|_| crate::plat::fs::rename(&tmp, self.root.join("profiles.json")));
        if w.is_err() { let _ = std::fs::remove_file(&tmp); return Err(()) }
        g.revision += 1; Ok(())
    }
    /// One profile changed and the registry written.
    fn update(&self, id: &str, f: impl FnOnce(&mut Profile)) -> Result<(), ()> { self.mutate_save(|g| { if let Some(p) = g.profiles.iter_mut().find(|p| p.id == id) { f(p) } }) }
    /// Change the profiles and write them; if the write fails the profiles are as they were.
    fn mutate_save(&self, f: impl FnOnce(&mut Inner)) -> Result<(), ()> {
        let mut g = self.inner.lock().unwrap(); let backup = g.profiles.clone(); let (rev, atts, jobs, cmds) = (g.revision, g.attempts.clone(), g.jobs.clone(), g.cmds.clone());
        f(&mut g);
        if self.save(&mut g).is_err() { g.profiles = backup; g.revision = rev; g.attempts = atts; g.jobs = jobs; g.cmds = cmds; return Err(()) }
        Ok(())
    }
    /// The profile's private directory (made 0700): its store inside when it has one.
    pub fn directory(&self, p: &Profile) -> std::io::Result<PathBuf> {
        let base = self.root.join(&p.id); private_dir(&base)?;
        if p.store.is_empty() { return Ok(base) }
        if !valid_id(&p.store) { return Err(std::io::Error::new(std::io::ErrorKind::Other, "Invalid account store")) }
        let d = base.join(&p.store); private_dir(&d)?; Ok(d)
    }
    fn remove_store(&self, id: &str, store: &str) {
        if !valid_id(store) { return }
        let d = self.root.join(id).join(store);
        if let Ok(m) = std::fs::symlink_metadata(&d) { if m.is_dir() && !m.file_type().is_symlink() { let _ = std::fs::remove_dir_all(&d); } }
    }
    /// The environment a profile's CLI runs with: the host's, minus every provider credential variable, plus the profile's own store.
    pub fn environment(&self, p: Option<&Profile>) -> Result<Vec<(String, String)>, ()> {
        let Some(p) = p else { return Ok(vec![]) };
        let mut env: Vec<(String, String)> = vec![("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(), "1".into()), ("DISABLE_AUTOUPDATER".into(), "1".into()), ("MCP_TOOL_TIMEOUT".into(), "86400000".into()), ("CLAUDE_CODE_ENABLE_TODO_TOOLS".into(), "1".into())];
        // a store that cannot be made or verified is a failure of the op: an empty value would let the CLI fall back to the person's own login
        let dir = self.directory(p).map_err(|_| ())?.to_string_lossy().to_string();
        if dir.is_empty() { return Err(()) }
        if p.provider == "codex" { env.push(("CODEX_HOME".into(), dir)); env.push(("RUST_LOG".into(), "error".into())); } else { env.push(("CLAUDE_CONFIG_DIR".into(), dir)); }
        Ok(env)
    }

    /// A test's account: saved, signed in (the stand-in CLI reads `login.json` / `auth.json` from its store) and connected, with no sign-in ceremony.
    #[cfg(test)]
    pub async fn plant(self: &Arc<Self>, provider: &str, label: &str, email: &str, allowance: Value) -> (String, u64) {
        let s = self.request(&json!({"op":"add","provider":provider,"label":label})).await;
        let id = s["profiles"].as_array().and_then(|a| a.last()).and_then(|p| p["id"].as_str()).expect("added").to_string();
        let store = new_id();
        self.update(&id, |p| { p.store = store; p.state = "connected".into(); p.epoch = 2; p.identity = json!({"email":email,"plan":"max","auth":"claude.ai"}); p.checked_at = now_s(); if !allowance.is_null() { p.allowance = Some(allowance) } }).unwrap();
        let dir = self.directory(&self.profile(&id).unwrap()).unwrap(); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(if provider == "codex" { "auth.json" } else { "login.json" }), json!({"email":email,"subscriptionType":"max","accountUuid":email,"orgUuid":"o1"}).to_string()).unwrap();
        (id, 2)
    }

    /// One-shot `<provider> <args>`: stdout JSON (not for logout), 12 s, 128 KB.
    async fn cli(&self, p: Option<&Profile>, program: &str, args: &[&str]) -> Result<(i32, Value), ()> {
        let prog = (self.resolve)(program).map_err(|_| ())?;
        let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Agent, &prog);
        c.args(args); c.current_dir(&self.root); apply_env(&mut c, &self.environment(p)?, p.is_some());
        c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
        let mut child = spawn(&mut c).await.map_err(|_| ())?;
        let mut out = child.stdout.take().ok_or(())?;
        let mut buf = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(12), async { let mut chunk = [0u8; 8192]; loop { let n = out.read(&mut chunk).await.map_err(|_| ())?; if n == 0 { break } buf.extend_from_slice(&chunk[..n]); if buf.len() > 131072 { return Err(()) } } Ok::<(), ()>(()) }).await;
        if !matches!(read, Ok(Ok(()))) { let _ = child.start_kill(); return Err(()) }
        let code = child.wait().await.map_err(|_| ())?.code().unwrap_or(-1);
        let data = if args.get(1) == Some(&"logout") { json!({}) } else { serde_json::from_slice(&buf).map_err(|_| ())? };
        Ok((code, data))
    }

    /// Claude's stream-json control requests (`initialize` -> models; `get_usage`): one child, the answers it gives, 12 s, nothing else read.
    async fn claude_control(&self, p: Option<&Profile>, want: &str) -> Option<Value> { self.claude_read(p, want).await.ok() }
    /// The CLI's own control reply (`models`, or `usage` after `initialize`), or the plain reason there was none.
    async fn claude_read(&self, p: Option<&Profile>, want: &str) -> Result<Value, String> {
        let prog = (self.resolve)("claude").map_err(|_| "The Claude CLI was not found on this machine.".to_string())?;
        let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Agent, &prog);
        c.args(["--setting-sources", "user", "--settings", "{\"forceLoginMethod\":\"claudeai\"}", "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"]);
        c.current_dir(&self.root); apply_env(&mut c, &self.environment(p).map_err(|_| "This account's private folder could not be verified.".to_string())?, p.is_some());
        c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
        let mut child = spawn(&mut c).await.map_err(|_| "The Claude CLI could not be started.".to_string())?;
        let (mut stdin, stdout) = (child.stdin.take().ok_or("The Claude CLI could not be started.")?, child.stdout.take().ok_or("The Claude CLI could not be started.")?);
        let first = json!({"type":"control_request","request_id":if want == "models" { "models" } else { "init" },"request":{"subtype":"initialize"}}).to_string();
        stdin.write_all(format!("{first}\n").as_bytes()).await.map_err(|_| "The Claude CLI closed before answering.".to_string())?;
        let r = tokio::time::timeout(Duration::from_secs(30), async {
            let mut lines = BufReader::new(stdout).lines(); let mut total = 0usize;
            while let Ok(Some(l)) = lines.next_line().await {
                total += l.len(); if total > 524288 { return Err("The Claude CLI's reply was too large.".to_string()) }
                let Ok(r) = serde_json::from_str::<Value>(&l) else { continue };
                if r["type"] != "control_response" { continue }
                let bad = r["response"]["subtype"] == "error";
                let why = || format!("Claude refused the request: {}", label(r["response"]["error"].as_str().unwrap_or("no reason given")));
                match (want, r["response"]["request_id"].as_str()) {
                    ("models", Some("models")) => return if bad { Err(why()) } else { Ok(r["response"]["response"].clone()) },
                    ("usage", Some("init")) => { if bad { return Err(why()) } let q = json!({"type":"control_request","request_id":"usage","request":{"subtype":"get_usage","skip_behaviors":true}}).to_string(); stdin.write_all(format!("{q}\n").as_bytes()).await.map_err(|_| "The Claude CLI closed before answering.".to_string())?; }
                    ("usage", Some("usage")) => return if bad { Err(why()) } else { Ok(r["response"]["response"].clone()) },
                    _ => {}
                }
            }
            Err("The Claude CLI closed before reporting usage.".to_string())
        }).await.unwrap_or_else(|_| Err("The Claude CLI did not answer within 30 seconds.".to_string()));
        let _ = child.start_kill(); r
    }
    /// The windows Claude reports for a profile (or the default login), or why there are none.
    async fn claude_usage(&self, p: Option<&Profile>) -> Result<Vec<Value>, String> {
        let r = self.claude_read(p, "usage").await?;
        if r["rate_limits_available"] != true { return Err("Claude does not report limits for this account (it may not be a subscription plan).".into()) }
        let w = Self::claude_windows(&r);
        if w.is_empty() { Err("Claude reported no limit windows for this account.".into()) } else { Ok(w) }
    }
    async fn claude_models(&self, p: &Profile) -> Vec<Value> {
        let Some(r) = self.claude_control(Some(p), "models").await else { return vec![] };
        r["models"].as_array().cloned().unwrap_or_default().into_iter().take(40).filter(|x| x["value"].is_string()).map(|x| json!({"id":label(x["value"].as_str().unwrap_or("")),"label":label(x["displayName"].as_str().or(x["value"].as_str()).unwrap_or("")),
            "efforts":x["supportedEffortLevels"].as_array().map(|a| a.iter().take(12).map(|y| json!(label(y.as_str().unwrap_or("")))).collect::<Vec<_>>()).unwrap_or_default()})).collect()
    }
    /// accounts.js claudeWindows
    pub fn claude_windows(reading: &Value) -> Vec<Value> {
        if reading["rate_limits_available"] != true { return vec![] }
        let mut out = vec![];
        for (scope, minutes) in [("five_hour", 300u64), ("seven_day", 10080), ("seven_day_opus", 10080), ("seven_day_sonnet", 10080)] {
            let w = &reading["rate_limits"][scope]; let Some(u) = w["utilization"].as_f64().filter(|u| (0.0..=100.0).contains(u)) else { continue };
            let mut o = json!({"name":scope,"scope":scope,"shared":scope == "five_hour" || scope == "seven_day","used_percent":u,"window_minutes":minutes});
            if let Some(r) = w["resets_at"].as_str().and_then(parse_iso).filter(|r| *r > 0) { o["resets_at"] = json!(r) }
            out.push(o);
        }
        out
    }
    /// accounts.js windows (Codex's rate limits)
    pub fn codex_windows(limits: &Value) -> Vec<Value> {
        let buckets: Vec<(String, Value)> = if let Some(o) = limits["rateLimitsByLimitId"].as_object() { o.iter().map(|(k, v)| (k.clone(), v.clone())).collect() } else if limits["rateLimits"].is_object() { vec![("codex".into(), limits["rateLimits"].clone())] } else { vec![] };
        let mut out = vec![];
        for (key, b) in buckets.into_iter().take(20) {
            if !b.is_object() { continue }
            for kind in ["primary", "secondary"] {
                let w = &b[kind]; let Some(u) = w["usedPercent"].as_f64().filter(|u| (0.0..=100.0).contains(u)) else { continue };
                let name = label(b["limitName"].as_str().or(b["limitId"].as_str()).unwrap_or(&key));
                let mut o = json!({"name":format!("{name}/{kind}"),"scope":label(b["limitId"].as_str().unwrap_or(&key)),"shared":true,"used_percent":u});
                if let Some(r) = w["resetsAt"].as_f64() { o["resets_at"] = json!(r) } if let Some(m) = w["windowDurationMins"].as_f64() { o["window_minutes"] = json!(m) }
                out.push(o);
            }
        }
        out
    }

    /// Who is signed in to this profile, from the provider CLI: the profile updated (epoch moves when the identity does), or why not.
    /// `asked`: the person asked for this check (status): the provider is asked too, however fresh the last reading.
    async fn verify(&self, p: &Profile, new_login: bool) -> Result<Profile, String> { self.verify_k(p, new_login, false).await }
    async fn verify_k(&self, p: &Profile, new_login: bool, asked: bool) -> Result<Profile, String> {
        let mut p = p.clone();
        let (identity, fp_identity, mut catalog, mut allowance);
        let mut fact = Fact::Unproven;
        let last = |k: &str| self.inner.lock().unwrap().refresh_at.get(k).copied().unwrap_or(0).max(p.allowance.as_ref().and_then(|a| a["at"].as_u64()).unwrap_or(0));
        if p.provider == "claude" {
            let (code, data) = self.cli(Some(&p), "claude", &["auth", "status"]).await.map_err(|_| "Claude subscription sign-in could not be verified".to_string())?;
            if code != 0 || data["loggedIn"] != true || data["authMethod"] != "claude.ai" { return Err("Claude subscription sign-in could not be verified; Console profiles and API credentials use a different store.".into()) }
            let dir = self.directory(&p).map_err(|e| e.to_string())?;
            if data["configDirectory"].as_str().map(|d| crate::plat::fs::canonicalize(d).unwrap_or_else(|_| PathBuf::from(d))) != Some(crate::plat::fs::canonicalize(&dir).unwrap_or(dir)) { return Err(DIRECTORY_ERR.into()) }
            identity = json!({"email":label(data["email"].as_str().unwrap_or("")),"plan":label(data["subscriptionType"].as_str().unwrap_or("")),"auth":"claude.ai"});
            fp_identity = json!([identity["email"], label(data["accountUuid"].as_str().or(data["userId"].as_str()).unwrap_or("")), label(data["orgUuid"].as_str().or(data["organizationId"].as_str()).unwrap_or(""))]);
            catalog = if p.catalog.is_empty() { self.claude_models(&p).await } else { p.catalog.clone() };
            allowance = p.allowance.clone();
            let at = last(&format!("usage:{}", p.id));
            // a login found dead is read every check: the usage read is the one request that proves it works again (no inference request is made)
            if new_login || asked || p.state == "expired" || at == 0 || now_s().saturating_sub(at) >= 300 {
                self.inner.lock().unwrap().refresh_at.insert(format!("usage:{}", p.id), now_s());
                let read = self.claude_read(Some(&p), "usage").await;
                match &read { Ok(_) => fact = Fact::Proven, Err(reason) if fb_auth(reason) => fact = Fact::Refused, Err(_) => {} }
                let windows = read.and_then(|r| if r["rate_limits_available"] != true { Err("Claude does not report limits for this account (it may not be a subscription plan).".to_string()) } else { let w = Self::claude_windows(&r); if w.is_empty() { Err("Claude reported no limit windows for this account.".into()) } else { Ok(w) } });
                match windows {
                    Ok(windows) => { let n = now_s(); allowance = Some(json!({"provider":"claude","state":"ok","at":n,"attempted_at":n,"auth":"claude.ai","source":"claude-cli-usage","windows":windows})) }
                    Err(reason) => {
                        // a failed read is tried again in a minute, not five; the last good reading stays, else the reason is what the account shows
                        self.inner.lock().unwrap().refresh_at.insert(format!("usage:{}", p.id), now_s().saturating_sub(240));
                        if allowance.as_ref().map_or(true, |a| a["state"] != "ok") { allowance = Some(json!({"provider":"claude","state":"unavailable","at":0,"attempted_at":now_s(),"source":"claude-cli-usage","reason":reason,"windows":[]})) }
                    }
                }
            }
        } else {
            let prog = (self.resolve)("codex")?;
            let mut rpc = Rpc::open(&prog, &self.environment(Some(&p)).map_err(|_| "Codex could not start".to_string())?, true).await.map_err(|_| "Codex could not start".to_string())?;
            let r = async {
                rpc.initialize().await?;
                let a = rpc.request("account/read", json!({"refreshToken":false}), 12).await?;
                if a["account"]["type"] != "chatgpt" { return Err(()) }
                let ident = json!({"email":label(a["account"]["email"].as_str().unwrap_or("")),"plan":label(a["account"]["planType"].as_str().unwrap_or("")),"auth":"chatgpt"});
                let at = last(&format!("usage:{}", p.id));
                let fresh = !new_login && !asked && p.state != "expired" && at != 0 && now_s().saturating_sub(at) < 300;
                if !fresh { self.inner.lock().unwrap().refresh_at.insert(format!("usage:{}", p.id), now_s()); }
                let limits = if fresh { None } else { rpc.request("account/rateLimits/read", json!({}), 12).await.ok() };
                let fp = json!([ident["email"], label(a["workspaceRouting"]["chatgptAccountId"].as_str().unwrap_or(""))]);
                let cat: Vec<Value> = if fresh { p.catalog.clone() } else {
                    rpc.request("model/list", json!({"limit":100,"includeHidden":false}), 12).await.ok().map(|m| m["data"].as_array().cloned().unwrap_or_default().into_iter().take(40).filter(|x| x["model"].is_string()).map(|x| json!({"id":label(x["model"].as_str().unwrap_or("")),"label":label(x["displayName"].as_str().or(x["model"].as_str()).unwrap_or("")),
                        "efforts":x["supportedReasoningEfforts"].as_array().map(|a| a.iter().take(12).map(|y| json!(label(y["reasoningEffort"].as_str().or(y.as_str()).unwrap_or("")))).collect::<Vec<_>>()).unwrap_or_default()})).collect()).unwrap_or_default() };
                let windows = limits.as_ref().map(Self::codex_windows).unwrap_or_default(); let n = now_s();
                let al = if fresh || windows.is_empty() { p.allowance.clone() } else { Some(json!({"provider":"codex","state":"ok","at":n,"attempted_at":n,"auth":"chatgpt","source":"codex-app-server","windows":windows})) };
                Ok::<_, ()>((ident, fp, cat, al, limits.is_some()))
            }.await;
            rpc.close().await;
            let (i, f, c, a, answered) = r.map_err(|_| "Codex ChatGPT sign-in could not be verified".to_string())?;
            identity = i; fp_identity = f; catalog = c; allowance = a; if answered { fact = Fact::Proven }
        }
        if !fp_identity.as_array().map_or(false, |a| a.iter().any(|x| x.as_str().map_or(false, |s| !s.is_empty()))) { return Err("Provider did not report a verifiable account identity".into()) }
        let digest = fingerprint(&p.id, &fp_identity);
        if new_login || (!p.fingerprint.is_empty() && p.fingerprint != digest) { p.epoch += 1; if !new_login { catalog = vec![]; allowance = None; } }
        p.fingerprint = digest; p.identity = identity; p.catalog = catalog; p.allowance = allowance; p.checked_at = now_s();
        if fact == Fact::Proven { p.proven_at = now_s() }
        p.state = after(if new_login { "unchecked" } else { &p.state }, fact).into();
        if p.state == "expired" { if p.expired_at == 0 { p.expired_at = now_s() } p.message = EXPIRED_SHORT.into() } else { p.expired_at = 0; p.message.clear() }
        Ok(p)
    }
}

fn parse_iso(s: &str) -> Option<u64> {
    // 2026-11-05T12:00:00Z or with fractional seconds / +00:00
    let t = s.trim(); if t.len() < 20 { return None }
    let (y, mo, d, h, mi, se): (i64, i64, i64, i64, i64, i64) = (t.get(0..4)?.parse().ok()?, t.get(5..7)?.parse().ok()?, t.get(8..10)?.parse().ok()?, t.get(11..13)?.parse().ok()?, t.get(14..16)?.parse().ok()?, t.get(17..19)?.parse().ok()?);
    let (y2, m2) = if mo <= 2 { (y - 1, mo + 9) } else { (y, mo - 3) };
    let days = 365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 + (153 * m2 + 2) / 5 + d - 719469 + 0;
    u64::try_from(days * 86400 + h * 3600 + mi * 60 + se).ok()
}

// ---- the snapshot and the ops ----

#[derive(Debug)]
pub struct LaunchPlan { pub program: PathBuf, pub env: Vec<(String, String)>, pub args: Vec<String> }

impl Accounts {
    pub fn snapshot(&self) -> Value {
        let g = self.inner.lock().unwrap();
        json!({"version":1,"revision":g.revision,"default_allowance":g.default_allowance,"error":g.load_error,"profiles":g.profiles.iter().map(Profile::public).collect::<Vec<_>>(),
            "attempts":g.attempts.iter().map(|a| json!({"id":a.id,"profile":a.profile,"state":a.state,"url":a.url,"userCode":a.user_code,"expiresAt":a.expires_at,"message":a.message})).collect::<Vec<_>>()})
    }
    fn profile(&self, id: &str) -> Result<Profile, ()> { let g = self.inner.lock().unwrap(); if !g.load_error.is_empty() { return Err(()) } g.profiles.iter().find(|p| p.id == id).cloned().ok_or(()) }
    fn with_profile<T>(&self, id: &str, f: impl FnOnce(&mut Profile, &mut Inner) -> T) -> Option<T> {
        let mut g = self.inner.lock().unwrap(); let i = g.profiles.iter().position(|p| p.id == id)?;
        let mut p = g.profiles[i].clone(); let r = f(&mut p, &mut g); if let Some(slot) = g.profiles.iter_mut().find(|x| x.id == id) { *slot = p } Some(r)
    }
    fn busy(&self, id: &str) -> bool { self.inner.lock().unwrap().busy.get(id).copied().unwrap_or(0) > 0 }
    /// A provider child of this profile is running (a turn): the agents say so, and a disconnect or remove waits.
    pub fn child_started(&self, id: &str) { *self.inner.lock().unwrap().busy.entry(id.to_string()).or_insert(0) += 1 }
    /// A turn's provider said this login is dead (fallback.rs `auth_failure`): the profile is to be signed in again, and says so until a sign-in or a request the provider
    /// answers proves otherwise. Only at the epoch the turn ran on. True when this changed it.
    pub fn expire(&self, id: &str, epoch: u64) -> bool {
        let mut changed = false;
        let apply = |g: &mut Inner, changed: &mut bool| {
            let Some(p) = g.profiles.iter_mut().find(|p| p.id == id && p.epoch == epoch) else { return };
            let next = after(&p.state, Fact::TurnRefused);
            if p.state == "connecting" { for a in g.attempts.iter_mut().filter(|a| a.profile == id && a.previous_state != "disconnected") { a.previous_state = "expired".into() } return }
            if next != p.state { p.state = next.into(); p.expired_at = now_s(); p.message = EXPIRED_SHORT.into(); *changed = true }
        };
        if self.mutate_save(|g| apply(g, &mut changed)).is_err() { let mut g = self.inner.lock().unwrap(); apply(&mut g, &mut changed); g.revision += 1; }
        changed
    }
    /// The profile's state now ("" when there is none).
    pub fn state_of(&self, id: &str) -> String { self.inner.lock().unwrap().profiles.iter().find(|p| p.id == id).map(|p| p.state.clone()).unwrap_or_default() }
    pub fn child_ended(&self, id: &str) { let mut g = self.inner.lock().unwrap(); if let Some(n) = g.busy.get_mut(id) { *n = n.saturating_sub(1); if *n == 0 { g.busy.remove(id); } } }

    /// A request on a profile: refused while another is in progress, or a launch is queued (accounts.js `jobs` / `launchQueues`). `check_busy`: also while a provider
    /// child (a turn) runs on it; refresh asks that (login checks `busy` and `launching` itself in `start`, disconnect and remove check it themselves), a status and a rename do not.
    fn begin(&self, id: &str, what: &str, check_busy: bool) -> Result<(), ()> {
        let mut g = self.inner.lock().unwrap();
        if g.jobs.contains_key(id) || g.launching.get(id).copied().unwrap_or(0) > 0 || (check_busy && g.busy.get(id).copied().unwrap_or(0) > 0) { return Err(()) }
        g.jobs.insert(id.to_string(), what.to_string()); Ok(())
    }
    fn end(&self, id: &str, what: &str) { let mut g = self.inner.lock().unwrap(); if g.jobs.get(id).map(String::as_str) == Some(what) { g.jobs.remove(id); } }

    /// One account request (accounts.js `request`): the snapshot after it; a failure is the snapshot with the generic error.
    pub async fn request(self: &Arc<Self>, r: &Value) -> Value {
        match self.request_inner(r).await {
            Ok(v) => v,
            Err(Some(msg)) => { let mut s = self.snapshot(); s["error"] = json!(msg); s }
            Err(None) => { let mut s = self.snapshot(); s["error"] = json!(GENERIC); s }
        }
    }
    async fn request_inner(self: &Arc<Self>, r: &Value) -> Result<Value, Option<String>> {
        let op = r["op"].as_str().ok_or(None)?;
        { if !self.inner.lock().unwrap().load_error.is_empty() { return Err(None) } }
        if op == "list" {
            // reverify saved profiles when the picker opens after a restart: initialization/status only
            let todo: Vec<String> = { let g = self.inner.lock().unwrap(); g.profiles.iter().filter(|p| (p.state == "unchecked" || p.state == "expired" && p.checked_at == 0) && !g.jobs.contains_key(&p.id)).map(|p| p.id.clone()).collect() };
            for id in todo { let _ = Box::pin(self.request_inner(&json!({"op":"status","profile":id}))).await; }
            return Ok(self.snapshot());
        }
        if op == "add" {
            let (provider, lab) = (r["provider"].as_str().unwrap_or(""), label(r["label"].as_str().unwrap_or("")));
            let mut g = self.inner.lock().unwrap();
            if !matches!(provider, "claude" | "codex") || lab.is_empty() || g.profiles.len() >= 12 { return Err(None) }
            let p = Profile { id: new_id(), provider: provider.into(), label: lab, epoch: 1, fingerprint: String::new(), store: String::new(), state: "disconnected".into(), identity: json!({}), catalog: vec![], allowance: None, checked_at: 0, message: String::new(), expired_at: 0, proven_at: 0 };
            self.directory(&p).map_err(|_| None)?;
            g.profiles.push(p); if self.save(&mut g).is_err() { g.profiles.pop(); return Err(None) } drop(g); return Ok(self.snapshot());
        }
        let pid = r["profile"].as_str().unwrap_or("").to_string();
        let p = self.profile(&pid).map_err(|_| None)?;
        match op {
            "cancel" => {
                let aid = r["attempt"].as_str().unwrap_or("");
                let tx = { let g = self.inner.lock().unwrap(); if !g.attempts.iter().any(|a| a.id == aid && a.profile == pid) { return Err(None) } g.cmds.get(aid).cloned() };
                self.cancel(aid, "cancelled", ""); if let Some(t) = tx { let _ = t.send(Cmd::Cancel); } return Ok(self.snapshot());
            }
            "respond" => {
                let aid = r["attempt"].as_str().unwrap_or(""); let code = r["code"].as_str().unwrap_or("");
                let (att, tx) = { let g = self.inner.lock().unwrap(); (g.attempts.iter().find(|a| a.id == aid).cloned(), g.cmds.get(aid).cloned()) };
                let ok = att.as_ref().map_or(false, |a| g_job_is(&self.inner, &pid, aid) && a.state == "waiting" && now_ms() < a.expires_at) && p.provider == "claude" && code.len() <= 4096 && !code.contains(['\r', '\n', '\0']);
                if !ok { return Err(None) }
                let att = att.unwrap(); let parts: Vec<&str> = code.trim().split('#').collect();
                let state = query_pairs(&att.url).into_iter().find(|(k, _)| k == "state").map(|(_, v)| v).unwrap_or_default();
                let error = if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() { "Paste the complete authentication code from Claude's sign-in page, including # and everything after it. API keys and setup-tokens cannot complete this sign-in." }
                    else if state.is_empty() || parts[1] != state { "That code belongs to a different sign-in attempt. Open this attempt's provider sign-in link and copy its new authentication code." } else { "" };
                if !error.is_empty() { { let mut g = self.inner.lock().unwrap(); if let Some(a) = g.attempts.iter_mut().find(|a| a.id == aid) { a.message = error.into() } g.revision += 1; } let mut s = self.snapshot(); s["error"] = json!(error); return Ok(s) }
                { let mut g = self.inner.lock().unwrap(); if let Some(a) = g.attempts.iter_mut().find(|a| a.id == aid) { a.message.clear() } g.revision += 1; }
                // Claude validates its own OAuth state and code; no callback is fetched here
                if let Some(t) = tx { let _ = t.send(Cmd::Respond(code.trim().to_string())); }
                return Ok(self.snapshot());
            }
            "login" => { self.start(&p, r["mode"].as_str().unwrap_or("device")).await.map_err(|_| None)?; return Ok(self.snapshot()) }
            _ => {}
        }
        // Delete is the way out of a stuck sign-in: its attempt is ended first (its child is told to stop), and a job left behind by an attempt that is gone is dropped
        if op == "remove" {
            let attempts: Vec<(String, Option<mpsc::UnboundedSender<Cmd>>)> = { let g = self.inner.lock().unwrap(); g.attempts.iter().filter(|a| a.profile == pid).map(|a| (a.id.clone(), g.cmds.get(&a.id).cloned())).collect() };
            for (aid, tx) in attempts { self.cancel(&aid, "cancelled", ""); if let Some(t) = tx { let _ = t.send(Cmd::Cancel); } }
            { let mut g = self.inner.lock().unwrap(); if g.jobs.get(&pid).map_or(false, |j| j.starts_with("attempt:")) { g.jobs.remove(&pid); } }
        }
        self.begin(&pid, "status", false).map_err(|_| None)?;
        let res: Result<(), ()> = async {
            match op {
                "status" => {
                    match self.verify_k(&p, false, r["asked"] == true).await {
                        Ok(np) => { self.update(&pid, |slot| *slot = merge_checked(slot, np))?; }
                        // a dead login stays one to sign in again: the CLI no longer naming anyone does not make it a disconnect
                        Err(_) => { let _ = self.update(&pid, |slot| { if after(&slot.state, Fact::NoLogin) == "expired" { return } slot.state = "disconnected".into(); slot.identity = json!({}); slot.catalog = vec![]; slot.allowance = None; slot.message = "Account status could not be verified.".into(); slot.expired_at = 0; }); }
                    }
                    Ok(())
                }
                "disconnect" => {
                    if self.busy(&pid) { return Err(()) }
                    if p.provider == "claude" { let (code, _) = self.cli(Some(&p), "claude", &["auth", "logout"]).await?; if code != 0 { return Err(()) } }
                    else { let prog = (self.resolve)("codex").map_err(|_| ())?; let mut rpc = Rpc::open(&prog, &self.environment(Some(&p))?, true).await?; let r = async { rpc.initialize().await?; rpc.request("account/logout", json!({}), 12).await.map(|_| ()) }.await; rpc.close().await; r?; }
                    // the registry is written first: a store is removed only once the file no longer names it
                    self.update(&pid, |slot| { slot.epoch += 1; slot.state = "disconnected".into(); slot.fingerprint.clear(); slot.identity = json!({}); slot.catalog = vec![]; slot.allowance = None; })?;
                    self.remove_store(&pid, &p.store); Ok(())
                }
                "rename" => { let l = label(r["label"].as_str().unwrap_or("")); if l.is_empty() { return Err(()) } self.update(&pid, |slot| slot.label = l) }
                "remove" => {
                    if self.busy(&pid) { return Err(()) }
                    let base = self.root.join(&pid); private_dir(&base).map_err(|_| ())?;
                    self.mutate_save(|g| { g.profiles.retain(|x| x.id != pid); g.attempts.retain(|a| a.profile != pid); })?;
                    let _ = std::fs::remove_dir_all(&base); Ok(())
                }
                _ => Err(()),
            }
        }.await;
        self.end(&pid, "status");
        res.map(|_| self.snapshot()).map_err(|_| None)
    }
    // ---- sign-in attempts ----
    async fn start(self: &Arc<Self>, p: &Profile, mode: &str) -> Result<(), ()> {
        let aid = new_id();
        let (rx, cand) = {
            let mut g = self.inner.lock().unwrap();
            let live: Vec<String> = g.jobs.iter().filter(|(_, v)| v.starts_with("attempt:")).map(|(k, _)| k.clone()).collect();
            g.attempts.retain(|a| live.contains(&a.profile));
            // a dead login is signed in again even while a process of it lives: that process can do nothing with it, and the new sign-in has a store of its own
            let dead = g.profiles.iter().any(|x| x.id == p.id && x.state == "expired");
            if g.attempts.len() >= 8 || g.jobs.contains_key(&p.id) || (!dead && g.busy.get(&p.id).copied().unwrap_or(0) > 0) || g.launching.get(&p.id).copied().unwrap_or(0) > 0 { return Err(()) }
            let cand = Profile { store: aid.clone(), ..p.clone() };
            g.attempts.push(Attempt { id: aid.clone(), profile: p.id.clone(), state: "starting".into(), url: String::new(), user_code: String::new(), expires_at: now_ms() + 600_000, message: String::new(), previous_state: p.state.clone(), store: aid.clone() });
            g.jobs.insert(p.id.clone(), format!("attempt:{aid}"));
            if let Some(slot) = g.profiles.iter_mut().find(|x| x.id == p.id) { slot.state = "connecting".into() }
            g.revision += 1;
            let (tx, rx) = mpsc::unbounded_channel(); g.cmds.insert(aid.clone(), tx); (rx, cand)
        };
        let me = self.clone(); let mode = mode.to_string();
        tokio::spawn(async move { me.run_attempt(aid, cand, mode, rx).await });
        Ok(())
    }
    fn current(&self, aid: &str) -> Option<Attempt> { let g = self.inner.lock().unwrap(); let a = g.attempts.iter().find(|a| a.id == aid)?.clone(); if g.jobs.get(&a.profile).map(String::as_str) == Some(format!("attempt:{aid}").as_str()) { Some(a) } else { None } }
    fn set_waiting(&self, aid: &str, url: String, code: String) { let mut g = self.inner.lock().unwrap(); if let Some(a) = g.attempts.iter_mut().find(|a| a.id == aid) { a.url = url; a.user_code = code; a.state = "waiting".into(); } g.revision += 1; }
    fn cancel(&self, aid: &str, state: &str, reason: &str) {
        let mut g = self.inner.lock().unwrap();
        let Some(i) = g.attempts.iter().position(|a| a.id == aid) else { return };
        let prof = g.attempts[i].profile.clone();
        if g.jobs.get(&prof).map(String::as_str) != Some(format!("attempt:{aid}").as_str()) { return }
        g.jobs.remove(&prof);
        let (store, prev) = (g.attempts[i].store.clone(), g.attempts[i].previous_state.clone());
        { let a = &mut g.attempts[i]; a.state = state.into(); a.url.clear(); a.user_code.clear(); a.message = if state == "failed" { failure_text(reason) } else { String::new() }; }
        let active = g.profiles.iter().find(|p| p.id == prof).map(|p| p.store.clone()).unwrap_or_default();
        if let Some(slot) = g.profiles.iter_mut().find(|p| p.id == prof) { slot.state = prev }
        g.revision += 1; g.cmds.remove(aid); let _ = (store, active);
        // the candidate store is removed when the attempt's child is gone (`discard`): a CLI finishing a sign-in must not recreate credentials after the delete
    }
    /// The attempt ended and its provider child with it: its candidate store goes unless it became the profile's.
    fn discard(&self, aid: &str) {
        let (prof, store, active, done) = { let g = self.inner.lock().unwrap(); let Some(a) = g.attempts.iter().find(|a| a.id == aid) else { return };
            (a.profile.clone(), a.store.clone(), g.profiles.iter().find(|p| p.id == a.profile).map(|p| p.store.clone()).unwrap_or_default(), a.state == "done") };
        if !done && store != active { self.remove_store(&prof, &store); }
    }
    async fn finish(self: &Arc<Self>, aid: &str, cand: &Profile, success: bool, reason: &str) {
        let Some(a) = self.current(aid) else { return };
        if now_ms() >= a.expires_at { return self.cancel(aid, "expired", "") }
        if !success { return self.cancel(aid, "failed", reason) }
        match self.verify(cand, true).await {
            Ok(np) => {
                if self.current(aid).is_none() { return }
                let old_store = self.profile(&a.profile).map(|p| p.store).unwrap_or_default();
                // one lock: the profile takes the new store and the attempt is done in the same step, so a cancel cannot land between and put the old state back over it
                let saved = self.mutate_save(|g| {
                    if let Some(slot) = g.profiles.iter_mut().find(|p| p.id == a.profile) { *slot = np.clone(); slot.store = a.store.clone(); }
                    g.jobs.remove(&a.profile); g.cmds.remove(aid);
                    if let Some(att) = g.attempts.iter_mut().find(|x| x.id == aid) { att.state = "done".into(); att.url.clear(); att.user_code.clear(); }
                });
                if saved.is_err() { return self.cancel(aid, "failed", "storage") }
                if !old_store.is_empty() { self.remove_store(&a.profile, &old_store); }
            }
            Err(e) => self.cancel(aid, "failed", if e == DIRECTORY_ERR { "directory" } else { "verification" }),
        }
    }
    async fn run_attempt(self: Arc<Self>, aid: String, cand: Profile, mode: String, cmds: mpsc::UnboundedReceiver<Cmd>) {
        self.run_attempt_inner(aid.clone(), cand, mode, cmds).await;
        self.discard(&aid);
    }
    async fn run_attempt_inner(self: &Arc<Self>, aid: String, cand: Profile, mode: String, mut cmds: mpsc::UnboundedReceiver<Cmd>) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
        if cand.provider == "codex" {
            let Ok(prog) = (self.resolve)("codex") else { return self.cancel(&aid, "failed", "") };
            let Ok(env) = self.environment(Some(&cand)) else { return self.cancel(&aid, "failed", "") };
            let Ok(mut rpc) = Rpc::open(&prog, &env, true).await else { return self.cancel(&aid, "failed", "") };
            let started = async {
                rpc.initialize().await?;
                let r = rpc.request("account/login/start", json!({"type": if mode == "browser" { "chatgpt" } else { "chatgptDeviceCode" }}), 30).await?;
                let id = r["loginId"].as_str().filter(|s| !s.is_empty() && s.len() <= 256).ok_or(())?.to_string();
                let url = sign_in_url(r["verificationUrl"].as_str().or(r["authUrl"].as_str()).unwrap_or(""), "codex"); if url.is_empty() { return Err(()) }
                Ok::<_, ()>((id, url, label(r["userCode"].as_str().unwrap_or(""))))
            }.await;
            let Ok((login_id, url, code)) = started else { rpc.close().await; return self.cancel(&aid, "failed", "") };
            if self.current(&aid).is_none() { rpc.close().await; return }
            self.set_waiting(&aid, url, code);
            let outcome = loop {
                tokio::select! {
                    n = rpc.notes.recv() => match n { Some(m) if m["method"] == "account/login/completed" && m["params"]["loginId"] == login_id => break Some(m["params"]["success"] == true), Some(_) => {}, None => break Some(false) },
                    c = cmds.recv() => match c { Some(Cmd::Cancel) | None => { let _ = rpc.request("account/login/cancel", json!({"loginId":login_id}), 5).await; rpc.close().await; return }, Some(Cmd::Respond(_)) => {} },
                    _ = tokio::time::sleep_until(deadline) => break None,
                }
            };
            rpc.close().await;
            match outcome { Some(ok) => self.finish(&aid, &cand, ok, "").await, None => self.cancel(&aid, "expired", "") }
        } else {
            let Ok(prog) = (self.resolve)("claude") else { return self.cancel(&aid, "failed", "cli_start") };
            let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Agent, &prog);
            c.args(["auth", "login", "--claudeai"]);
            let Ok(mut env) = self.environment(Some(&cand)) else { return self.cancel(&aid, "failed", "cli_start") }; env.push(("BROWSER".into(), "/bin/true".into())); // the CLI prints its own authorization link; no browser is opened here
            apply_env(&mut c, &env, true);
            c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
            let Ok(mut child) = spawn(&mut c).await else { return self.cancel(&aid, "failed", "cli_start") };
            let (mut stdin, so, se) = (child.stdin.take().unwrap(), child.stdout.take().unwrap(), child.stderr.take().unwrap());
            let (otx, mut orx) = mpsc::unbounded_channel::<Vec<u8>>();
            let readers: [Box<dyn tokio::io::AsyncRead + Unpin + Send>; 2] = [Box::new(so), Box::new(se)];
            for mut r in readers { let tx = otx.clone(); tokio::spawn(async move { let mut b = [0u8; 4096]; loop { match r.read(&mut b).await { Ok(0) | Err(_) => break, Ok(n) => { if tx.send(b[..n].to_vec()).is_err() { break } } } } }); }
            drop(otx);
            let (mut output, mut reason, mut open) = (String::new(), String::new(), true);
            // what the CLI says is read for its link and for why a code was refused; no transcript is kept or sent
            let mut scan = |output: &str, reason: &mut String| -> Option<String> {
                let low = output.to_lowercase();
                if low.contains("that code wasn't accepted") || low.contains("invalid code.") || low.contains("invalid_grant") || (low.contains("code") && (low.contains("expired") || low.contains("already used"))) { *reason = "code_rejected".into() }
                else if low.contains("could not save") || low.contains("failed to save") || low.contains("credential store could not be") { *reason = "storage".into() }
                else if ["enotfound", "econnrefused", "etimedout", "unable to connect", "fetch failed"].iter().any(|w| low.contains(w)) { *reason = "network".into() }
                let mut rest = output;
                while let Some(i) = rest.find("https://") {
                    let tail = &rest[i..]; let end = tail.find(|c: char| c.is_whitespace() || c == '\u{1b}' || c == '<' || c == '>').unwrap_or(tail.len());
                    let u = sign_in_url(&tail[..end], "claude"); if !u.is_empty() { return Some(u) } rest = &tail[8..];
                }
                None
            };
            let outcome: Result<bool, &str> = loop {
                tokio::select! {
                    chunk = orx.recv(), if open => match chunk {
                        Some(b) => { output.push_str(&String::from_utf8_lossy(&b)); if output.len() > 131072 { break Err("failed") } if let Some(u) = scan(&output, &mut reason) { if self.current(&aid).is_some() { self.set_waiting(&aid, u, String::new()); } } }
                        None => open = false,
                    },
                    cmd = cmds.recv() => match cmd { Some(Cmd::Respond(code)) => { let _ = stdin.write_all(format!("{code}\n").as_bytes()).await; } Some(Cmd::Cancel) | None => { let _ = child.start_kill(); let _ = child.wait().await; return } },
                    st = child.wait() => {
                        // whatever it printed last still counts (why a code was refused)
                        while let Ok(Some(b)) = tokio::time::timeout(Duration::from_millis(200), orx.recv()).await { output.push_str(&String::from_utf8_lossy(&b)); let _ = scan(&output, &mut reason); }
                        break Ok(st.map(|s| s.success()).unwrap_or(false))
                    },
                    _ = tokio::time::sleep_until(deadline) => break Err("expired"),
                }
            };
            let _ = child.start_kill(); let _ = child.wait().await;
            match outcome { Ok(ok) => self.finish(&aid, &cand, ok, &reason).await, Err("expired") => self.cancel(&aid, "expired", ""), Err(_) => self.cancel(&aid, "failed", "") }
        }
    }
}
fn g_job_is(inner: &Mutex<Inner>, profile: &str, aid: &str) -> bool { inner.lock().unwrap().jobs.get(profile).map(String::as_str) == Some(format!("attempt:{aid}").as_str()) }

// ---- launching a turn under an account, and the background refresh ----
/// How long a launch waits for a routine check of its profile to end: longer than a check can take (its CLI calls time out at 12 and 30 s).
const CHECK_WAIT: Duration = Duration::from_secs(90);
impl Accounts {
    /// Rechecks the selected identity before a new process (accounts.js `launchOne`): no model turn. The environment and the arguments to put before the
    /// provider's own, or why the turn must not start under this account.
    pub async fn launch_plan(self: &Arc<Self>, profile: &str, epoch: &str, cmd: &str) -> Result<LaunchPlan, String> {
        const NO: &str = "Reconnect and select this account before starting a turn";
        // launches on one profile queue one behind another; any other request on it is refused while one is queued or running
        // a routine check on it (the 5-minute refresh, the status the picker asks for) is waited out, not a refusal: it ends in seconds, and the queued launch
        // below then keeps the next one off until this turn has its plan. A sign-in in progress is refused at once (it waits on the person), as is a missing profile.
        let started = std::time::Instant::now();
        let (lock, _queued) = loop {
            {
                let mut g = self.inner.lock().unwrap();
                if !g.profiles.iter().any(|p| p.id == profile) { return Err(NO.to_string()) }
                match g.jobs.get(profile) {
                    None => {
                        *g.launching.entry(profile.to_string()).or_insert(0) += 1;
                        break (g.launch_locks.entry(profile.to_string()).or_default().clone(), Queued { me: self.clone(), id: profile.to_string() });
                    }
                    Some(j) if j.starts_with("attempt:") => return Err(NO.to_string()),
                    Some(_) if started.elapsed() >= CHECK_WAIT => return Err("This account's status check did not finish; start the turn again".to_string()),
                    Some(_) => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let _turn = lock.lock().await;
        // the profile as it is now, after the launches ahead of this one: what they changed (identity, epoch, state) counts
        let p = self.profile(profile).map_err(|_| NO.to_string())?;
        if p.state == "expired" && p.epoch.to_string() == epoch && cmd == p.provider { return Err(EXPIRED.into()) }
        if !matches!(p.state.as_str(), "connected" | "unchecked") || p.epoch.to_string() != epoch || cmd != p.provider { return Err(NO.into()) }
        let r = async {
            let np = self.verify(&p, false).await.map_err(|_| NO.to_string())?;
            // the verified identity is kept even when the turn is refused: the epoch it moved to is what the person must select again
            let np = { let mut kept = np.clone(); self.update(profile, |slot| { *slot = merge_checked(slot, np); kept = slot.clone(); }).map_err(|_| "Account storage failed".to_string())?; kept };
            if np.state == "expired" { return Err(EXPIRED.into()) }
            if np.epoch.to_string() != epoch || np.state != "connected" { return Err("Account identity changed; select this account again".to_string()) }
            let program = (self.resolve)(cmd)?;
            let args: Vec<String> = if np.provider == "codex" { vec!["-c".into(), "cli_auth_credentials_store=\"file\"".into(), "-c".into(), "model_provider=\"openai\"".into()] }
                else { vec!["--setting-sources".into(), "user".into(), "--settings".into(), "{\"forceLoginMethod\":\"claudeai\"}".into()] };
            Ok(LaunchPlan { program, env: self.environment(Some(&np)).map_err(|_| NO.to_string())?, args })
        }.await;
        r
    }
    /// Whether the provider CLI's own login (no profile) can start a turn now: the last route a start on an account has (agents `account_route`). Claude is asked
    /// (`auth status`, a few hundred ms, only when a start falls back); an API key in the environment is a login; Codex's own login is taken as there.
    pub async fn default_login(self: &Arc<Self>, provider: &str) -> bool {
        if provider != "claude" || std::env::var_os("ANTHROPIC_API_KEY").is_some() { return true }
        matches!(self.cli(None, "claude", &["auth", "status"]).await, Ok((0, d)) if d["loggedIn"] == true)
    }
    /// A current Claude child may publish its passive usage windows (accounts.js `observe`); anything else changes nothing.
    pub fn observe(&self, profile: &str, epoch: &str, raw: &str) -> Value {
        let Ok(p) = self.profile(profile) else { return self.snapshot() };
        if p.provider != "claude" || p.epoch.to_string() != epoch || p.state != "connected" { return self.snapshot() }
        let Ok(data) = serde_json::from_str::<Value>(raw) else { return self.snapshot() };
        let mut windows = vec![];
        for key in ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet"] {
            let w = &data[key]; if !w.is_object() { continue }
            let mut o = json!({"name":format!("{key} (provider pool)"),"scope":key,"shared":true});
            if let Some(u) = w["utilization"].as_f64().filter(|u| (0.0..=1.0).contains(u)) { o["used_percent"] = json!(u * 100.0) }
            if let Some(r) = w["resetsAt"].as_f64() { o["resets_at"] = json!(r) }
            windows.push(o);
        }
        if windows.is_empty() { return self.snapshot() }
        let n = now_s();
        if p.allowance.as_ref().map_or(false, |a| n.saturating_sub(a["at"].as_u64().unwrap_or(0)) < 60 && a["windows"] == json!(windows)) { return self.snapshot() }
        self.with_profile(profile, |slot, g| { slot.allowance = Some(json!({"provider":"claude","state":"ok","at":n,"source":"claude-stream","auth":"claude.ai","windows":windows})); g.revision += 1; });
        self.snapshot()
    }
    fn refresh_at_retry(&self, g: &mut Inner) { g.refresh_at.insert("default-claude".into(), now_s().saturating_sub(240)); }
    /// The 5-minute recheck of connected profiles (accounts.js `refresh`); a test hub and a copy never run it.
    pub async fn refresh(self: &Arc<Self>) {
        let now = now_s();
        // an expired login is checked too, busy or not (a process of it can do nothing): a sign-in made elsewhere, or a provider that recovered, brings it back
        let ids: Vec<String> = { let g = self.inner.lock().unwrap(); g.profiles.iter().filter(|p| (matches!(p.state.as_str(), "connected" | "unchecked") && g.busy.get(&p.id).copied().unwrap_or(0) == 0 || p.state == "expired") && !g.jobs.contains_key(&p.id))
            .filter(|p| { let at = g.refresh_at.get(&p.id).copied().unwrap_or(0).max(p.allowance.as_ref().and_then(|a| a["at"].as_u64()).unwrap_or(0)); at == 0 || now.saturating_sub(at) >= 300 }).map(|p| p.id.clone()).collect() };
        // a profile with a turn running is not verified again (its sign-in is in use), but its usage is a read-only request and is kept current
        let busy: Vec<String> = { let g = self.inner.lock().unwrap(); g.profiles.iter().filter(|p| p.provider == "claude" && p.state == "connected" && !g.jobs.contains_key(&p.id) && g.busy.get(&p.id).copied().unwrap_or(0) > 0)
            .filter(|p| { let at = g.refresh_at.get(&format!("usage:{}", p.id)).copied().unwrap_or(0).max(p.allowance.as_ref().and_then(|a| a["at"].as_u64()).unwrap_or(0)); at == 0 || now.saturating_sub(at) >= 300 }).map(|p| p.id.clone()).collect() };
        for id in busy {
            self.inner.lock().unwrap().refresh_at.insert(format!("usage:{id}"), now);
            let Ok(p) = self.profile(&id) else { continue };
            let r = self.claude_usage(Some(&p)).await; let n = now_s();
            // the read was refused as a dead login: said now, before the next turn of it finds out
            if matches!(&r, Err(reason) if fb_auth(reason)) { self.expire(&id, p.epoch); continue }
            let _ = self.with_profile(&id, |slot, g| { match r {
                Ok(windows) => slot.allowance = Some(json!({"provider":"claude","state":"ok","at":n,"attempted_at":n,"auth":"claude.ai","source":"claude-cli-usage","windows":windows})),
                Err(reason) => if slot.allowance.as_ref().map_or(true, |a| a["state"] != "ok") { slot.allowance = Some(json!({"provider":"claude","state":"unavailable","at":0,"attempted_at":n,"source":"claude-cli-usage","reason":reason,"windows":[]})) },
            } g.revision += 1; });
        }
        for id in ids {
            self.inner.lock().unwrap().refresh_at.insert(id.clone(), now);
            let dead = self.profile(&id).map_or(false, |p| p.state == "expired");
            if self.begin(&id, "refresh", !dead).is_err() { continue }
            if let Ok(p) = self.profile(&id) { if let Ok(np) = self.verify(&p, false).await { let _ = self.update(&id, |slot| *slot = merge_checked(slot, np)); } }
            self.end(&id, "refresh");
        }
        let due = { let g = self.inner.lock().unwrap(); let at = g.refresh_at.get("default-claude").copied().unwrap_or(0); at == 0 || now.saturating_sub(at) >= 300 };
        if due && std::env::var_os("ANTHROPIC_API_KEY").is_none() {
            self.inner.lock().unwrap().refresh_at.insert("default-claude".into(), now);
            if let Ok((0, data)) = self.cli(None, "claude", &["auth", "status"]).await {
                if data["loggedIn"] == true && data["authMethod"] == "claude.ai" {
                    use sha2::{Digest, Sha256};
                    let ident = hex::encode(Sha256::digest(json!([data["email"], data["accountUuid"].as_str().or(data["userId"].as_str()), data["orgUuid"].as_str().or(data["organizationId"].as_str())]).to_string().as_bytes()));
                    let changed = { let mut g = self.inner.lock().unwrap(); let c = g.default_identity != ident; if c { g.default_allowance = None } g.default_identity = ident; c }; let _ = changed;
                    match self.claude_usage(None).await {
                        Ok(windows) => { let n = now_s(); let mut g = self.inner.lock().unwrap(); g.default_allowance = Some(json!({"provider":"claude","state":"ok","auth":"claude.ai","at":n,"attempted_at":n,"source":"claude-cli-usage","windows":windows})); g.revision += 1; }
                        Err(reason) => { let mut g = self.inner.lock().unwrap(); self.refresh_at_retry(&mut g); if g.default_allowance.as_ref().map_or(true, |a| a["state"] != "ok") { g.default_allowance = Some(json!({"provider":"claude","state":"unavailable","at":0,"attempted_at":now,"source":"claude-cli-usage","reason":reason,"windows":[]})); g.revision += 1; } }
                    }
                } else { let mut g = self.inner.lock().unwrap(); g.default_identity.clear(); g.default_allowance = Some(json!({"provider":"claude","state":"unavailable","at":0,"attempted_at":now,"source":"claude-cli-usage","windows":[]})); g.revision += 1; }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_sign_in_state_machine_keeps_a_dead_login_dead_until_something_proves_it_works() {
        use Fact::*;
        // a turn's provider says the login is dead: from any usable state it is expired; a sign-in under way and a disconnect stay what they are
        for st in ["connected", "unchecked", "expired"] { assert_eq!(after(st, TurnRefused), "expired", "{st}") }
        assert_eq!(after("connecting", TurnRefused), "connecting"); assert_eq!(after("disconnected", TurnRefused), "disconnected");
        // Nov 7: `claude auth status` still named the account; that alone never makes a dead login connected again
        assert_eq!(after("expired", Unproven), "expired"); assert_eq!(after("expired", NoLogin), "expired", "nobody signed in is still a sign-in to make, not a disconnect");
        assert_eq!(after("expired", Refused), "expired"); assert_eq!(after("expired", Proven), "connected", "the provider answered a request made with it");
        assert_eq!(after("expired", SignedIn), "connected");
        // a usable login: a refused request finds it dead before any turn does; an unproven check keeps it as it was
        assert_eq!(after("connected", Refused), "expired"); assert_eq!(after("connected", Unproven), "connected"); assert_eq!(after("unchecked", Unproven), "connected");
        assert_eq!(after("connected", NoLogin), "disconnected"); assert_eq!(after("unchecked", NoLogin), "disconnected");
        assert_eq!(after("connecting", Proven), "connecting", "a check never ends a sign-in");
    }
    #[test] fn a_check_that_started_before_the_login_died_does_not_bring_it_back() {
        let base = Profile { id: "p".into(), provider: "claude".into(), label: "W".into(), epoch: 2, fingerprint: String::new(), store: String::new(), state: "connected".into(), identity: json!({}), catalog: vec![], allowance: None, checked_at: 0, message: String::new(), expired_at: 0, proven_at: 0 };
        let dead = Profile { state: "expired".into(), expired_at: 100, message: EXPIRED_SHORT.into(), ..base.clone() };
        let stale = Profile { checked_at: 120, proven_at: 50, ..base.clone() };
        assert_eq!(merge_checked(&dead, stale.clone()).state, "expired", "its proof is older than the failure");
        assert_eq!(merge_checked(&dead, Profile { proven_at: 130, ..stale.clone() }).state, "connected", "a request answered after it");
        assert_eq!(merge_checked(&base, stale).state, "connected");
    }
    #[tokio::test] async fn an_expired_login_is_kept_across_a_restart_and_refuses_a_launch_in_words_the_failover_reads() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let home = std::env::temp_dir().join(format!("bp-acc-exp-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let a = Arc::new(Accounts::open(&home, Arc::new(|p: &str| Ok(PathBuf::from(p)))));
        let (id, epoch) = a.plant("claude", "Work", "w@x.com", Value::Null).await;
        assert!(!a.expire(&id, epoch + 1), "another epoch is another login"); assert!(a.expire(&id, epoch)); assert!(!a.expire(&id, epoch), "said once");
        assert_eq!(a.state_of(&id), "expired");
        let e = a.launch_plan(&id, &epoch.to_string(), "claude").await.unwrap_err(); assert_eq!(e, EXPIRED);
        assert!(super::super::fallback::from_stderr("claude", "m", &e).map_or(false, |l| l.auth), "a start it refuses fails over as a failed login");
        let again = Accounts::open(&home, Arc::new(|p: &str| Ok(PathBuf::from(p))));
        let snap = again.snapshot(); let p = &snap["profiles"][0];
        assert_eq!((p["state"].as_str().unwrap(), p["message"].as_str().unwrap()), ("expired", EXPIRED_SHORT), "a restart still knows");
        assert!(p["expiredAt"].as_u64().unwrap() > 0);
        let _ = std::fs::remove_dir_all(&home);
    }
    static ENV: Mutex<()> = Mutex::new(());
    const CLAUDE: &str = r#"#!/usr/bin/env python3
import sys, os, json, time
d = os.environ.get("CLAUDE_CONFIG_DIR", "")
a = sys.argv[1:]
def login(): return os.path.join(d, "login.json")
if a[:2] == ["auth", "status"]:
    json.dump({"leak": bool(os.environ.get("ANTHROPIC_API_KEY")), "dir": d}, open(os.path.join(d, "env.json"), "w")) if d else None
    if d and os.path.exists(login()):
        j = json.load(open(login())); j.update({"loggedIn": True, "authMethod": "claude.ai", "configDirectory": d}); print(json.dumps(j))
    else: print(json.dumps({"loggedIn": False}))
elif a[:2] == ["auth", "logout"]:
    if os.path.exists(login()): os.remove(login())
elif a[:2] == ["auth", "login"]:
    if d: open(d.rstrip("/") + ".pid", "w").write(str(os.getpid()))
    print("Open https://claude.com/cai/oauth/authorize?code=true&client_id=x&state=STATE123 to sign in", flush=True)
    line = sys.stdin.readline().strip()
    if line == "CODE#STATE123":
        json.dump({"email": os.environ.get("STANDIN_EMAIL", "a@example.com"), "subscriptionType": "max", "accountUuid": "u1", "orgUuid": "o1"}, open(login(), "w"))
    else:
        print("That code wasn't accepted.", flush=True); sys.exit(1)
elif "-p" in a:
    for l in sys.stdin:
        try: r = json.loads(l)
        except Exception: continue
        rid = r.get("request_id"); sub = r.get("request", {}).get("subtype")
        resp = {}
        if rid == "models": resp = {"models": [{"value": "opus", "displayName": "Opus", "supportedEffortLevels": ["low", "high"]}]}
        if sub == "get_usage": resp = {"rate_limits_available": True, "rate_limits": {"five_hour": {"utilization": 42.5, "resets_at": "2026-11-05T12:00:00Z"}}}
        print(json.dumps({"type": "control_response", "response": {"request_id": rid, "subtype": "success", "response": resp}}), flush=True)
"#;
    const CODEX: &str = r#"#!/usr/bin/env python3
import sys, os, json, threading
d = os.environ.get("CODEX_HOME", "")
auth = os.path.join(d, "auth.json")
lock = threading.Lock()
def send(o):
    with lock: print(json.dumps(o), flush=True)
for l in sys.stdin:
    try: m = json.loads(l)
    except Exception: continue
    if "id" not in m: continue
    i, meth = m["id"], m.get("method")
    if meth == "account/read":
        if os.path.exists(auth): send({"id": i, "result": {"account": {"type": "chatgpt", "email": json.load(open(auth))["email"], "planType": "plus"}, "workspaceRouting": {"chatgptAccountId": "acc1"}}})
        else: send({"id": i, "result": {"account": None}})
    elif meth == "account/rateLimits/read": send({"id": i, "result": {"rateLimits": {"limitId": "codex", "primary": {"usedPercent": 10, "resetsAt": 1800000000, "windowDurationMins": 300}}}})
    elif meth == "model/list": send({"id": i, "result": {"data": [{"model": "gpt-x", "displayName": "GPT X", "supportedReasoningEfforts": [{"reasoningEffort": "low"}]}]}})
    elif meth == "account/login/start":
        send({"id": i, "result": {"loginId": "L1", "verificationUrl": "https://auth.openai.com/codex/device", "userCode": "ABCD-1234"}})
        def done():
            import time; time.sleep(0.4); json.dump({"email": os.environ.get("STANDIN_EMAIL", "c@example.com")}, open(auth, "w"))
            send({"method": "account/login/completed", "params": {"loginId": "L1", "success": True}})
        threading.Thread(target=done, daemon=True).start()
    elif meth == "account/logout":
        if os.path.exists(auth): os.remove(auth)
        send({"id": i, "result": {}})
    else: send({"id": i, "result": {}})
"#;
    fn dirs(tag: &str) -> (PathBuf, PathBuf) {
        let h = std::env::temp_dir().join(format!("bp-acc-{}-{tag}", std::process::id())); let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap();
        let sd = h.join("bin"); std::fs::create_dir_all(&sd).unwrap();
        for (n, t) in [("claude", CLAUDE), ("codex", CODEX)] { let p = sd.join(n); std::fs::write(&p, t).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        (h, sd)
    }
    fn mgr(home: &Path, sd: &Path) -> Arc<Accounts> { let cli = crate::agents::proc::Cli::standin(sd); Arc::new(Accounts::open(home, Arc::new(move |p: &str| cli.resolve(p)))) }
    async fn add(a: &Arc<Accounts>, provider: &str, lab: &str) -> String { let s = a.request(&json!({"op":"add","provider":provider,"label":lab})).await; assert_eq!(s["error"], "", "{s}"); s["profiles"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap().to_string() }
    /// Waits on the manager's state, never on a time: the deadline is only there so a real hang fails, far above what a stand-in CLI takes on a loaded machine
    /// (a full gate at load 32 outlasted the old 10 s while a python stand-in started).
    const PATIENCE: Duration = Duration::from_secs(120);
    /// `f` holds, waited for on the state up to [`PATIENCE`]; the caller's own assert says what it wanted.
    async fn soon(f: impl Fn() -> bool) { let end = std::time::Instant::now() + PATIENCE; while !f() && std::time::Instant::now() < end { tokio::time::sleep(Duration::from_millis(20)).await } }
    async fn until(a: &Arc<Accounts>, f: impl Fn(&Value) -> bool) -> Value { let end = std::time::Instant::now() + PATIENCE; loop { let s = a.snapshot(); if f(&s) { return s } if std::time::Instant::now() >= end { panic!("timed out: {}", a.snapshot()) } tokio::time::sleep(Duration::from_millis(50)).await } }
    fn prof<'a>(s: &'a Value, id: &str) -> &'a Value { s["profiles"].as_array().unwrap().iter().find(|p| p["id"] == id).unwrap() }

    #[test] fn sign_in_links_are_https_on_the_providers_hosts_without_credentials() {
        assert!(!sign_in_url("https://claude.com/cai/oauth/authorize?code=true&state=s", "claude").is_empty());
        assert_eq!(sign_in_url("https://claude.com/cai/oauth/authorize?code=abc&state=s", "claude"), "", "an OAuth code is not the code-entry flag");
        for bad in ["http://claude.ai/x", "https://evil.example/x", "https://claude.ai.evil.com/x", "https://user:pw@claude.ai/x", "https://claude.ai:8443/x", "https://claude.ai/x?access_token=1", "https://claude.ai/x y", "https://auth.openai.com/x"] { assert_eq!(sign_in_url(bad, "claude"), "", "{bad}") }
        assert!(!sign_in_url("https://auth.openai.com/codex/device", "codex").is_empty()); assert_eq!(sign_in_url("https://claude.ai/x", "codex"), "");
    }
    #[test] fn allowance_windows_are_read_as_the_js_hub_reads_them() {
        let w = Accounts::claude_windows(&json!({"rate_limits_available":true,"rate_limits":{"five_hour":{"utilization":42.5,"resets_at":"2026-11-05T12:00:00Z"},"seven_day":{"utilization":120},"seven_day_opus":{"utilization":3}}}));
        assert_eq!(w.len(), 2); assert_eq!((w[0]["name"].clone(), w[0]["shared"].clone(), w[0]["window_minutes"].clone(), w[0]["resets_at"].clone()), (json!("five_hour"), json!(true), json!(300), json!(1_793_880_000u64)));
        assert_eq!(w[1]["scope"], "seven_day_opus"); assert_eq!(w[1]["shared"], false); assert!(Accounts::claude_windows(&json!({"rate_limits_available":false})).is_empty());
        let c = Accounts::codex_windows(&json!({"rateLimits":{"limitId":"codex","primary":{"usedPercent":10,"resetsAt":5,"windowDurationMins":300},"secondary":{"usedPercent":-1}}}));
        assert_eq!(c.len(), 1); assert_eq!((c[0]["name"].clone(), c[0]["used_percent"].clone(), c[0]["window_minutes"].clone()), (json!("codex/primary"), json!(10.0), json!(300.0)));
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_usage_read_that_fails_says_why() {
        let _l = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (h, sd) = dirs("usage"); let a = mgr(&h, &sd);
        let script = CLAUDE.replace("resp = {\"rate_limits_available\": True,", "resp = {\"rate_limits_available\": os.environ.get(\"STANDIN_USAGE\") != \"none\",");
        let path = sd.join("claude"); std::fs::write(&path, script).unwrap();
        std::env::set_var("STANDIN_USAGE", "ok"); let w = a.claude_usage(None).await.unwrap(); assert_eq!(w[0]["scope"], "five_hour");
        std::env::set_var("STANDIN_USAGE", "none"); let e = a.claude_usage(None).await.unwrap_err(); assert!(e.contains("does not report limits"), "{e}");
        std::env::remove_var("STANDIN_USAGE"); let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn the_registry_adds_renames_removes_and_survives_a_restart_and_a_bad_file_is_not_trusted() {
        use crate::plat::fs::PermissionsExt as _;
        let (h, sd) = dirs("reg"); let a = mgr(&h, &sd);
        assert_eq!(a.request(&json!({"op":"add","provider":"grok","label":"x"})).await["error"], GENERIC); assert_eq!(a.request(&json!({"op":"add","provider":"claude","label":"  "})).await["error"], GENERIC);
        let id = add(&a, "claude", "Work\u{7}").await; let s = a.snapshot(); assert_eq!((prof(&s, &id)["label"].clone(), prof(&s, &id)["state"].clone(), prof(&s, &id)["epoch"].clone()), (json!("Work"), json!("disconnected"), json!(1)));
        let root = h.join("providers/accounts-v1");
        assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(root.join("profiles.json")).unwrap().permissions().mode() & 0o777, 0o600), 0o600); assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(root.join(&id)).unwrap().permissions().mode() & 0o777, 0o700), 0o700);
        assert_eq!(a.request(&json!({"op":"rename","profile":id,"label":"Home"})).await["profiles"][0]["label"], "Home"); assert_eq!(a.request(&json!({"op":"rename","profile":id,"label":""})).await["error"], GENERIC);
        let b = mgr(&h, &sd); assert_eq!(prof(&b.snapshot(), &id)["state"], "unchecked", "after a restart a profile is unchecked until asked"); assert_eq!(prof(&b.snapshot(), &id)["label"], "Home");
        assert_eq!(b.request(&json!({"op":"remove","profile":id})).await["profiles"], json!([])); assert!(!root.join(&id).exists());
        for _ in 0..12 { add(&b, "codex", "c").await; } assert_eq!(b.request(&json!({"op":"add","provider":"codex","label":"one more"})).await["error"], GENERIC, "at most 12");
        std::fs::set_permissions(root.join("profiles.json"), std::fs::Permissions::from_mode(0o644)).unwrap();
        let c = mgr(&h, &sd); assert_eq!(c.snapshot()["error"], "The private account registry could not be verified."); assert_eq!(c.request(&json!({"op":"list"})).await["error"], GENERIC);
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_claude_sign_in_shows_its_link_takes_the_pasted_code_and_connects_a_verified_account() {
        let _l = ENV.lock().unwrap_or_else(|e| e.into_inner()); std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-should-not-leak");
        let (h, sd) = dirs("cl"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await;
        assert_eq!(a.request(&json!({"op":"status","profile":id})).await["profiles"][0]["state"], "disconnected", "nothing signed in");
        let s = a.request(&json!({"op":"login","profile":id,"mode":"device"})).await; assert_eq!(s["error"], "");
        let s = until(&a, |s| s["attempts"][0]["state"] == "waiting").await; let aid = s["attempts"][0]["id"].as_str().unwrap().to_string();
        assert_eq!(s["attempts"][0]["url"], "https://claude.com/cai/oauth/authorize?code=true&client_id=x&state=STATE123"); assert_eq!(prof(&s, &id)["state"], "connecting");
        assert_eq!(a.request(&json!({"op":"login","profile":id})).await["error"], GENERIC, "one request at a time");
        let r = a.request(&json!({"op":"respond","profile":id,"attempt":aid,"code":"justacode"})).await; assert!(r["error"].as_str().unwrap().starts_with("Paste the complete authentication code"));
        let r = a.request(&json!({"op":"respond","profile":id,"attempt":aid,"code":"CODE#OTHER"})).await; assert!(r["error"].as_str().unwrap().starts_with("That code belongs to a different sign-in attempt"));
        assert_eq!(a.request(&json!({"op":"respond","profile":id,"attempt":aid,"code":"a\nb#c"})).await["error"], GENERIC);
        assert_eq!(a.request(&json!({"op":"respond","profile":id,"attempt":aid,"code":"CODE#STATE123"})).await["error"], "");
        let s = until(&a, |s| s["attempts"][0]["state"] == "done").await; let p = prof(&s, &id);
        assert_eq!((p["state"].clone(), p["epoch"].clone(), p["identity"]["email"].clone(), p["identity"]["plan"].clone()), (json!("connected"), json!(2), json!("a@example.com"), json!("max")), "{p}");
        assert_eq!(p["catalog"][0]["id"], "opus"); assert_eq!(p["allowance"]["windows"][0]["used_percent"], 42.5); assert_eq!(s["attempts"][0]["url"], "");
        // the account's own store, and the CLI never saw the host's credentials
        let store = h.join("providers/accounts-v1").join(&id); let stores: Vec<_> = std::fs::read_dir(&store).unwrap().flatten().filter(|e| e.path().is_dir()).collect(); assert_eq!(stores.len(), 1, "one store: the sign-in's");
        let env: Value = serde_json::from_str(&std::fs::read_to_string(stores[0].path().join("env.json")).unwrap()).unwrap(); assert_eq!(env["leak"], false, "ANTHROPIC_API_KEY is not passed to a profile's CLI");
        // status keeps it; a launch under the account is planned with its store; another epoch is refused
        assert_eq!(a.request(&json!({"op":"status","profile":id})).await["profiles"][0]["state"], "connected");
        let plan = a.launch_plan(&id, "2", "claude").await.unwrap(); assert!(plan.env.iter().any(|(k, v)| k == "CLAUDE_CONFIG_DIR" && v.contains(&id))); assert_eq!(&plan.args[..2], ["--setting-sources", "user"]);
        assert!(a.launch_plan(&id, "1", "claude").await.is_err()); assert!(a.launch_plan(&id, "2", "codex").await.is_err());
        // a different person signing in behind the profile's back moves the epoch: the old selection must be made again
        std::fs::write(stores[0].path().join("login.json"), json!({"email":"other@example.com","accountUuid":"u2","orgUuid":"o1"}).to_string()).unwrap();
        assert!(a.launch_plan(&id, "2", "claude").await.unwrap_err().contains("changed")); assert_eq!(prof(&a.snapshot(), &id)["epoch"], 3);
        // disconnect: logged out, epoch moves, the store is gone
        a.child_started(&id); assert_eq!(a.request(&json!({"op":"disconnect","profile":id})).await["error"], GENERIC, "not while a turn runs"); a.child_ended(&id);
        let s = a.request(&json!({"op":"disconnect","profile":id})).await; assert_eq!((prof(&s, &id)["state"].clone(), prof(&s, &id)["epoch"].clone()), (json!("disconnected"), json!(4)));
        assert!(std::fs::read_dir(&store).unwrap().flatten().all(|e| !e.path().is_dir()), "the store is forgotten");
        std::env::remove_var("ANTHROPIC_API_KEY"); let _ = std::fs::remove_dir_all(&h);
    }
    async fn connect(a: &Arc<Accounts>, id: &str) {
        a.request(&json!({"op":"login","profile":id})).await; let s = until(a, |s| s["attempts"][0]["state"] == "waiting").await; let aid = s["attempts"][0]["id"].as_str().unwrap().to_string();
        assert_eq!(a.request(&json!({"op":"respond","profile":id,"attempt":aid,"code":"CODE#STATE123"})).await["error"], "");
        until(a, |s| s["attempts"][0]["state"] == "done").await;
    }
    fn launch_gate(a: &Arc<Accounts>, id: &str) -> Arc<tokio::sync::Mutex<()>> { a.inner.lock().unwrap().launch_locks.entry(id.to_string()).or_default().clone() }
    fn queued(a: &Arc<Accounts>, id: &str) -> usize { a.inner.lock().unwrap().launching.get(id).copied().unwrap_or(0) }
    #[tokio::test(flavor = "multi_thread")] async fn launches_on_one_profile_queue_and_every_other_request_waits_for_them() {
        let (h, sd) = dirs("lq"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await; connect(&a, &id).await;
        let gate = launch_gate(&a, &id); let hold = gate.lock().await; // a launch is running
        let (a1, i1) = (a.clone(), id.clone()); let one = tokio::spawn(async move { a1.launch_plan(&i1, "2", "claude").await });
        let (a2, i2) = (a.clone(), id.clone()); let two = tokio::spawn(async move { a2.launch_plan(&i2, "2", "claude").await });
        soon(|| queued(&a, &id) == 2).await;
        assert_eq!(queued(&a, &id), 2, "both are queued behind the one running");
        for op in ["status", "login", "disconnect", "remove"] { assert_eq!(a.request(&json!({"op":op,"profile":id})).await["error"], GENERIC, "{op} is refused while launches are queued"); }
        assert_eq!(prof(&a.snapshot(), &id)["state"], "connected", "and none of them changed it");
        drop(hold);
        assert!(one.await.unwrap().is_ok() && two.await.unwrap().is_ok(), "they run one after the other");
        assert_eq!(queued(&a, &id), 0); assert_eq!(a.request(&json!({"op":"status","profile":id})).await["error"], "", "the profile is free again");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_profile_with_a_stuck_sign_in_can_still_be_deleted() {
        let (h, sd) = dirs("rm-stuck"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await;
        a.request(&json!({"op":"login","profile":id})).await; let s = until(&a, |s| s["attempts"][0]["state"] == "waiting").await; assert_eq!(prof(&s, &id)["state"], "connecting");
        let s = a.request(&json!({"op":"remove","profile":id})).await; assert_eq!(s["error"], "", "{s}"); assert_eq!(s["profiles"], json!([]), "the profile is gone with its sign-in");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_launch_rechecks_the_profile_after_the_ones_ahead_and_a_dropped_one_leaves_the_queue() {
        let (h, sd) = dirs("lr"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await; connect(&a, &id).await;
        let gate = launch_gate(&a, &id); let hold = gate.lock().await;
        let (a1, i1) = (a.clone(), id.clone()); let waits = tokio::spawn(async move { a1.launch_plan(&i1, "2", "claude").await });
        soon(|| queued(&a, &id) == 1).await;
        // while it waits, the identity behind the profile moves (what the launch ahead of it would have found)
        a.update(&id, |p| p.epoch = 3).unwrap();
        drop(hold);
        assert!(waits.await.unwrap().is_err(), "the epoch it was asked for is gone: refused on the profile as it is now");
        let hold = gate.lock().await; let (a2, i2) = (a.clone(), id.clone()); let dropped = tokio::spawn(async move { a2.launch_plan(&i2, "3", "claude").await });
        soon(|| queued(&a, &id) == 1).await;
        dropped.abort(); let _ = dropped.await; assert_eq!(queued(&a, &id), 0, "a dropped launch does not strand the profile"); drop(hold);
        assert_eq!(a.request(&json!({"op":"status","profile":id})).await["error"], "");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_launch_during_a_routine_check_waits_for_it_and_starts_but_not_during_a_sign_in() {
        let (h, sd) = dirs("lw"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await; connect(&a, &id).await;
        for job in ["refresh", "status"] {
            a.begin(&id, job, false).unwrap(); // the 5-minute recheck (or the picker's status) is running on the profile
            let (a1, i1) = (a.clone(), id.clone()); let t0 = std::time::Instant::now(); let launch = tokio::spawn(async move { a1.launch_plan(&i1, "2", "claude").await });
            tokio::time::sleep(Duration::from_millis(500)).await;
            assert!(!launch.is_finished(), "{job}: the launch waits for the check, it is not refused");
            a.end(&id, job);
            let r = launch.await.unwrap(); assert!(r.is_ok(), "{job}: then it starts: {:?}", r.err()); assert!(t0.elapsed() >= Duration::from_millis(500));
            assert_eq!(queued(&a, &id), 0, "{job}: and leaves no queued launch");
        }
        // a check that found the account gone: the waiting launch is refused on the profile as the check left it, never run on another login
        a.begin(&id, "status", false).unwrap();
        let (a1, i1) = (a.clone(), id.clone()); let launch = tokio::spawn(async move { a1.launch_plan(&i1, "2", "claude").await });
        tokio::time::sleep(Duration::from_millis(300)).await; a.update(&id, |p| p.state = "disconnected".into()).unwrap(); a.end(&id, "status");
        assert!(launch.await.unwrap().unwrap_err().contains("Reconnect"), "refused once the check says it is disconnected");
        a.update(&id, |p| p.state = "connected".into()).unwrap();
        // a sign-in in progress waits on the person: refused at once
        a.begin(&id, "attempt:x", false).unwrap(); let t0 = std::time::Instant::now();
        assert!(a.launch_plan(&id, "2", "claude").await.is_err()); assert!(t0.elapsed() < Duration::from_secs(1), "not held for the sign-in");
        a.end(&id, "attempt:x"); let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_launch_and_a_status_go_on_while_a_turn_runs_but_not_a_sign_in() {
        let (h, sd) = dirs("lc"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await; connect(&a, &id).await;
        a.child_started(&id);
        assert!(a.launch_plan(&id, "2", "claude").await.is_ok(), "a second turn may start under the account");
        assert_eq!(a.request(&json!({"op":"status","profile":id})).await["error"], "", "status is not refused by a running turn");
        assert_eq!(a.request(&json!({"op":"login","profile":id})).await["error"], GENERIC, "a sign-in is");
        a.child_ended(&id); let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_failed_save_leaves_the_profile_as_it_was_and_says_so() {
        let (h, sd) = dirs("sv"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await;
        let reg = h.join("providers/accounts-v1/profiles.json"); std::fs::remove_file(&reg).unwrap(); std::fs::create_dir_all(reg.join("x")).unwrap(); // the rename onto it cannot succeed
        let rev = a.snapshot()["revision"].clone();
        assert_eq!(a.request(&json!({"op":"rename","profile":id,"label":"New"})).await["error"], GENERIC);
        let s = a.snapshot(); assert_eq!((prof(&s, &id)["label"].clone(), s["revision"].clone()), (json!("Work"), rev), "label and revision are as they were");
        assert_eq!(a.request(&json!({"op":"remove","profile":id})).await["error"], GENERIC); assert!(a.snapshot()["profiles"].as_array().unwrap().iter().any(|p| p["id"] == id.as_str()), "a profile whose removal was not saved stays");
        assert!(h.join("providers/accounts-v1").join(&id).exists(), "and so does its directory");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_candidate_store_survives_a_cancel_until_the_child_has_exited() {
        let (h, sd) = dirs("cs"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await;
        a.request(&json!({"op":"login","profile":id})).await; let s = until(&a, |s| s["attempts"][0]["state"] == "waiting").await; let aid = s["attempts"][0]["id"].as_str().unwrap().to_string();
        let store = h.join("providers/accounts-v1").join(&id).join(&aid); assert!(store.exists(), "the candidate store exists while the CLI runs");
        a.cancel(&aid, "cancelled", ""); // the attempt is over for the person; the child is only now told to go (the command channel closes)
        // the invariant, whatever the timing: the store is never gone while its child still runs (read the store first: it is removed only after the exit)
        let pidf = PathBuf::from(format!("{}.pid", store.display())); soon(|| pidf.exists()).await;
        let pid: u32 = std::fs::read_to_string(&pidf).unwrap().trim().parse().unwrap();
        let end = std::time::Instant::now() + PATIENCE;
        loop {
            let gone = !store.exists();
            if gone { assert!(!crate::agents::proc::alive(pid), "the store went while its child ran"); break }
            assert!(std::time::Instant::now() < end, "the store was never removed"); tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!store.exists(), "removed once the child is gone");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_refused_code_fails_the_attempt_with_its_reason_and_leaves_the_profile_as_it_was() {
        let (h, sd) = dirs("bad"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await;
        a.request(&json!({"op":"login","profile":id})).await; let s = until(&a, |s| s["attempts"][0]["state"] == "waiting").await; let aid = s["attempts"][0]["id"].as_str().unwrap().to_string();
        // the stand-in takes exactly one line; a line that is not its code is refused
        a.inner.lock().unwrap().cmds.get(&aid).unwrap().send(Cmd::Respond("WRONG#STATE123".into())).unwrap();
        let s = until(&a, |s| s["attempts"][0]["state"] == "failed").await;
        assert!(s["attempts"][0]["message"].as_str().unwrap().starts_with("Claude did not accept that authentication code"), "{s}"); assert_eq!(prof(&s, &id)["state"], "disconnected"); assert_eq!(prof(&s, &id)["epoch"], 1);
        assert!(std::fs::read_dir(h.join("providers/accounts-v1").join(&id)).unwrap().flatten().all(|e| !e.path().is_dir()), "the candidate store is discarded");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_sign_in_can_be_cancelled_and_started_again() {
        let (h, sd) = dirs("cancel"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await;
        a.request(&json!({"op":"login","profile":id})).await; let s = until(&a, |s| s["attempts"][0]["state"] == "waiting").await; let aid = s["attempts"][0]["id"].as_str().unwrap().to_string();
        assert_eq!(a.request(&json!({"op":"cancel","profile":id,"attempt":"nope"})).await["error"], GENERIC);
        let s = a.request(&json!({"op":"cancel","profile":id,"attempt":aid})).await; assert_eq!((s["attempts"][0]["state"].clone(), s["attempts"][0]["url"].clone(), prof(&s, &id)["state"].clone()), (json!("cancelled"), json!(""), json!("disconnected")));
        assert_eq!(a.request(&json!({"op":"login","profile":id})).await["error"], ""); until(&a, |s| s["attempts"].as_array().unwrap().iter().any(|x| x["state"] == "waiting")).await;
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_codex_device_sign_in_connects_and_disconnects() {
        let (h, sd) = dirs("cx"); let a = mgr(&h, &sd); let id = add(&a, "codex", "Team").await;
        assert_eq!(a.request(&json!({"op":"login","profile":id,"mode":"device"})).await["error"], "");
        let s = until(&a, |s| s["attempts"][0]["state"] == "waiting").await; assert_eq!((s["attempts"][0]["url"].clone(), s["attempts"][0]["userCode"].clone()), (json!("https://auth.openai.com/codex/device"), json!("ABCD-1234")));
        let s = until(&a, |s| s["attempts"][0]["state"] == "done").await; let p = prof(&s, &id);
        assert_eq!((p["state"].clone(), p["epoch"].clone(), p["identity"]["email"].clone(), p["identity"]["auth"].clone()), (json!("connected"), json!(2), json!("c@example.com"), json!("chatgpt")), "{p}");
        assert_eq!((p["catalog"][0]["id"].clone(), p["allowance"]["windows"][0]["name"].clone()), (json!("gpt-x"), json!("codex/primary")));
        let plan = a.launch_plan(&id, "2", "codex").await.unwrap(); assert!(plan.env.iter().any(|(k, _)| k == "CODEX_HOME")); assert!(plan.args.iter().any(|x| x.contains("cli_auth_credentials_store")));
        let s = a.request(&json!({"op":"disconnect","profile":id})).await; assert_eq!((prof(&s, &id)["state"].clone(), prof(&s, &id)["epoch"].clone()), (json!("disconnected"), json!(3)));
        assert_eq!(a.request(&json!({"op":"bogus","profile":id})).await["error"], GENERIC);
        let _ = std::fs::remove_dir_all(&h);
    }
}

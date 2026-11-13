//! The provider-owned account lifecycle: named logins ("profiles") of Claude and Codex, each in a private
//! store of its own under `<home>/providers/accounts-v1/<profile>[/<store>]`. Ops (`request`): list, add, status, login (a sign-in attempt that shows a link, and for
//! Claude takes the pasted code), respond, cancel, rename, disconnect, remove. No inference request is ever made: status asks the CLI who is signed in
//! (`claude auth status`, Codex's `account/read`), the allowance reads are the providers' own read-only usage requests, and nothing but allowlisted metadata
//! leaves this module (raw CLI output is never kept, logged or sent, except the one line a failed sync sign-in's CLI showed, never a token: `said_line`; a profile's credentials never leave the CLI's store, except its access token read for the
//! usage request: claude_usage.rs).
//! Every child is a provider CLI resolved like the agents' (the stand-in folder in a test hub, else PATH) and made through the process gate (Kind::Agent).
//! Any failure of an op answers the same generic message the JS hub gives; what a sign-in failed on is one of a few fixed sentences.
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration}; use crate::plat::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin}, sync::{mpsc, oneshot}};

pub const GENERIC: &str = "Account operation failed. Check its current status before retrying.";
/// Why an op was refused because something else runs on the account (#1108), instead of GENERIC.
pub const SIGNING_IN: &str = "A sign-in is already in progress on this account. Finish or cancel it first.";
pub const CHECKING: &str = "A verification is already running on this account. Try again in a moment.";
pub const STARTING: &str = "A turn is starting on this account. Try again in a moment.";
pub const TURN_RUNNING: &str = "A turn is running on this account. Try again when it ends.";
/// The whole of a one-shot CLI call (`Accounts::cli`): its output and its exit.
const CLI_LIMIT: Duration = Duration::from_secs(12);
const FAILURES: &[(&str, &str)] = &[
    ("code_rejected", "Claude did not accept that authentication code. It may be incomplete, expired, already used, or from an earlier sign-in. Choose Connect / Login and copy the new code from that sign-in page."),
    ("network", "Claude could not reach its sign-in service. Check the connection and try Connect / Login again."),
    ("storage", "Claude could not save the sign-in in this account's private store. Check its directory permissions and try again."),
    ("cli_start", "Claude Code could not start. Check the provider CLI installation and try again."),
    ("directory", "Claude did not report this account's private directory. Update Claude Code and try again."),
    ("verification", "The provider sign-in finished, but the saved subscription account could not be verified. Try Connect / Login again."),
];
/// Why a usage read has no windows: the CLI says the account has no plan limits, or the answer had none.
const NO_LIMITS: &str = "Claude does not report limits for this account (it may not be a subscription plan).";
const NO_WINDOWS: &str = "Claude reported no limit windows for this account.";
const SIGNIN_DEFAULT: &str = "Sign-in could not be verified. Retry or check the provider CLI installation.";
const DIRECTORY_ERR: &str = "Claude must report its isolated account directory (Claude Code 2.1.268 or later)";

fn fb_auth(s: &str) -> bool { super::fallback::auth_text(s) }
fn last_of(inner: &Mutex<Inner>, key: &str) -> u64 { inner.lock().unwrap().refresh_at.get(key).copied().unwrap_or(0) }
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

/// One usage reading of a Claude login.
#[derive(Debug)]
pub struct Usage { pub windows: Vec<Value>, pub extra: Option<Value>, pub source: &'static str }
impl Usage {
    /// The allowance a reading makes (`email`: the login it was read with, for the CLI's own login whose email is not in a profile).
    pub fn allowance(self, at: u64, email: &str) -> Value {
        let mut a = json!({"provider":"claude","state":"ok","at":at,"attempted_at":at,"auth":"claude.ai","source":self.source,"windows":self.windows});
        if let Some(x) = self.extra { a["extra"] = x }
        if !email.is_empty() { a["email"] = json!(email) }
        a
    }
}
#[derive(Clone, Debug, Default)]
pub struct Profile { pub id: String, pub provider: String, pub label: String, pub epoch: u64, pub fingerprint: String, pub store: String, pub state: String, pub identity: Value, pub catalog: Vec<Value>, pub allowance: Option<Value>, pub checked_at: u64, pub message: String,
    /// When its sign-in was found dead (0: it was not): kept in the registry, so a restart still knows it must be signed in again.
    pub expired_at: u64,
    /// When the provider last answered a request made with this login (its usage read): the proof a dead sign-in works again. Not kept.
    pub proven_at: u64,
    /// "" a CLI login in its own store; [`SYNC`] Claude Code's long-lived token (`claude setup-token`), kept in the vault ([`token_name`]) and shared with the owner's linked machines.
    pub kind: String,
    /// A sync account another machine made: that machine's link id here ("" made here). Only the machine that made one announces it (accounts_sync.rs).
    pub origin: String,
    /// A sync token's issue and end (seconds; 0 unknown): its end raises the renewal alert a month ahead (accounts_hub.rs `renew_alert`).
    pub made_at: u64, pub expires_at: u64 }
/// The account kind of a Claude Code long-lived token.
pub const SYNC: &str = "sync";
/// The vault name of a sync account's token (`hub.`: the hub's own, never listed, run or set through the secret box).
pub fn token_name(id: &str) -> String { format!("hub.account.{id}.token") }
/// How long `claude setup-token` says its token lasts: one year.
pub const TOKEN_LIFE: u64 = 365 * 86400;
/// A token's life in the CLI's words ("Your OAuth token (valid for 1 year):", "valid for 364 days"), in seconds; None when it said none.
pub fn valid_for(text: &str) -> Option<u64> {
    let i = text.find("valid for")?; let rest = text[i + 9..].trim_start();
    let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n: u64 = if n.is_empty() { if rest.starts_with("a ") || rest.starts_with("an ") { 1 } else { return None } } else { n.parse().ok()? };
    let unit = rest.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start().trim_start_matches("an ").trim_start_matches("a ");
    let per = [("year", 365 * 86400), ("month", 30 * 86400), ("week", 7 * 86400), ("day", 86400), ("hour", 3600)].iter().find(|(u, _)| unit.starts_with(u)).map(|(_, s)| *s)?;
    Some(n * per)
}
/// The terminal's escape sequences (colour, cursor moves) out of what a CLI on a pty printed.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len()); let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\u{1b}' { out.push(c); continue }
        match it.peek() {
            // Claude Code's Ink places words by column (`Paste\e[8Gcode`): a column move is a gap between words
            Some('[') => { it.next(); while let Some(&d) = it.peek() { it.next(); if ('@'..='~').contains(&d) { if matches!(d, 'G' | 'C') && !out.ends_with([' ', '\n', '\r']) && !out.is_empty() { out.push(' ') } break } } }
            Some(']') => { it.next(); while let Some(d) = it.next() { if d == '\u{7}' { break } if d == '\u{1b}' { it.next(); break } } }
            Some(_) => { it.next(); }
            None => {}
        }
    }
    out
}
/// The long-lived token in what `claude setup-token` printed (`sk-ant-oat01-...`), or None.
pub fn catch_token(text: &str) -> Option<String> {
    let i = text.find("sk-ant-oat")?;
    let t: String = text[i..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    (t.len() >= 40 && t.len() <= 512).then_some(t)
}
/// Where a sync account's token is kept. The hub's is its vault; a test's is memory.
pub trait Tokens: Send + Sync {
    fn get(&self, id: &str) -> Option<String>;
    /// false when it could not be kept
    fn put(&self, id: &str, label: &str, token: &str) -> bool;
    fn del(&self, id: &str);
    /// whether the store can be read now (a locked vault cannot: nothing is announced from it then)
    fn open(&self) -> bool;
}
pub struct VaultTokens;
impl Tokens for VaultTokens {
    fn get(&self, id: &str) -> Option<String> { super::vault::own_get(&token_name(id)) }
    fn put(&self, id: &str, label: &str, token: &str) -> bool { !super::vault::own_put(&token_name(id), token, &format!("Claude sync token: {label}")).starts_with("fail") }
    fn del(&self, id: &str) { if super::vault::own_get(&token_name(id)).is_some() { let _ = super::vault::own_del(&token_name(id)); } }
    fn open(&self) -> bool { super::vault::status()["locked"] != true }
}
#[derive(Default)]
pub struct MemTokens(pub Mutex<HashMap<String, String>>);
impl Tokens for MemTokens {
    fn get(&self, id: &str) -> Option<String> { self.0.lock().unwrap().get(id).cloned() }
    fn put(&self, id: &str, _label: &str, token: &str) -> bool { self.0.lock().unwrap().insert(id.into(), token.into()); true }
    fn del(&self, id: &str) { self.0.lock().unwrap().remove(id); }
    fn open(&self) -> bool { true }
}
impl Profile {
    fn public(&self) -> Value {
        let allowance = self.allowance.as_ref().map(|a| { let mut a = a.clone(); if now_s().saturating_sub(a["at"].as_u64().unwrap_or(0)) > 300 { a["state"] = json!("stale") } a });
        json!({"id":self.id,"provider":self.provider,"label":self.label,"epoch":self.epoch,"state":self.state,"identity":self.identity,"checkedAt":self.checked_at,"catalog":self.catalog,"allowance":allowance,"message":self.message,"expiredAt":self.expired_at,
            "kind":self.kind,"origin":self.origin,"madeAt":self.made_at,"tokenExpiresAt":self.expires_at})
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
struct Inner { profiles: Vec<Profile>,
    /// a sync token a sign-in caught, by profile, while it is checked (never kept here longer, never in the registry)
    caught: HashMap<String, String>, attempts: Vec<Attempt>, jobs: HashMap<String, String>, cmds: HashMap<String, mpsc::UnboundedSender<Cmd>>, revision: u64, load_error: String, default_allowance: Option<Value>, default_identity: String, refresh_at: HashMap<String, u64>, busy: HashMap<String, usize>, launching: HashMap<String, usize>, launch_locks: HashMap<String, Arc<tokio::sync::Mutex<()>>>, runs_on: HashMap<String, (String, u64)>, runs_seq: u64 }

/// A launch waiting or running on a profile; dropped (finished, refused or its future dropped) it leaves the queue, so the profile is never stranded.
struct Queued { me: Arc<Accounts>, id: String }
impl Drop for Queued { fn drop(&mut self) { let mut g = self.me.inner.lock().unwrap(); if let Some(n) = g.launching.get_mut(&self.id) { *n = n.saturating_sub(1); if *n == 0 { g.launching.remove(&self.id); } } } }
pub struct Accounts { root: PathBuf, inner: Mutex<Inner>, resolve: Arc<dyn Fn(&str) -> Result<PathBuf, String> + Send + Sync>,
    /// Claude's usage read with each login's own token (claude_usage.rs): the request, and the CLI's own store. None in a test hub, a stand-in hub and a copy:
    /// the stand-in CLI's `get_usage` is read instead.
    usage: Mutex<Option<(super::claude_usage::Get, Option<PathBuf>)>>,
    /// The models-list request (the real hub's only: a test hub asks no API).
    models_get: Mutex<Option<super::claude_usage::Get>>,
    /// One usage read (and the token refresh it may need) at a time per login ("" is the CLI's own).
    usage_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// Where sync accounts' tokens are kept (the vault; memory in a test).
    tokens: Mutex<Arc<dyn Tokens>> }

fn private_dir(dir: &Path) -> std::io::Result<()> {
    if let Some(parent) = dir.parent() { if !parent.exists() { std::fs::DirBuilder::new().recursive(true).mode(0o700).create(parent)?; } }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) { Ok(()) => {} Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {} Err(e) => return Err(e) }
    let m = std::fs::symlink_metadata(dir)?;
    use crate::plat::fs::MetadataExt;
    if !m.is_dir() || m.file_type().is_symlink() || m.uid() != crate::plat::fs::uid() { return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unsafe provider directory")) }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

/// What goes before a Claude child's own arguments: the person's settings only, and a CLI login must be a claude.ai one. A sync account's token is a claude.ai
/// subscription's too, but comes from the environment, which that setting is not about.
fn claude_pre(p: Option<&Profile>) -> &'static [&'static str] {
    if p.map_or(false, |p| p.kind == SYNC) { &["--setting-sources", "user"] } else { &["--setting-sources", "user", "--settings", "{\"forceLoginMethod\":\"claudeai\"}"] }
}
/// A registry entry's kind is one this hub knows: a CLI login, or a Claude sync token.
fn kind_ok(p: &Value) -> bool { match p["kind"].as_str().unwrap_or("") { "" => true, SYNC => p["provider"] == "claude" && p["origin"].as_str().map_or(true, |o| o.len() <= 128), _ => false } }
/// What a registry entry keeps of its kind (the rest of a profile is the caller's).
fn saved_kind(p: &Value) -> Profile {
    if p["kind"] != SYNC { return Profile { identity: json!({}), ..Profile::default() } }
    Profile { kind: SYNC.into(), origin: p["origin"].as_str().unwrap_or("").into(), made_at: p["madeAt"].as_u64().unwrap_or(0), expires_at: p["expiresAt"].as_u64().unwrap_or(0),
        identity: json!({"email":label(p["email"].as_str().unwrap_or("")),"plan":label(p["plan"].as_str().unwrap_or("")),"auth":"oauth_token"}), ..Profile::default() }
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
    /// Killed, and its exit waited for a moment only: a missed exit (#1108) must not hold the op that closes it.
    async fn close(mut self) { let _ = self.child.start_kill(); let _ = tokio::time::timeout(Duration::from_secs(3), self.child.wait()).await; }
}
/// The variables that point a CLI at one profile's store (`environment` sets them). The default login never inherits them: a hub started from a shell that
/// ran under a profile (an agent's own Bash, after failover moved it) would otherwise run every default-login thread, and read its usage, under that profile.
pub const STORE_ENV: &[&str] = &["CLAUDE_CONFIG_DIR", "CODEX_HOME"];
/// The host's environment, minus every provider credential variable when the child is a NAMED profile's (its own store is then put in). The CLI's own login
/// (`named` false) keeps the host's environment, as the JS hub's `environment(null)` does, except a profile's store ([`STORE_ENV`]).
fn apply_env(c: &mut tokio::process::Command, env: &[(String, String)], named: bool) {
    let (remove, set) = env_plan(std::env::vars_os().filter_map(|(k, _)| k.into_string().ok()), env, named);
    for k in remove { c.env_remove(k); } for (k, v) in set { c.env(k, v); }
}
/// `apply_env` as what to remove of the host's variables (`host`) and what to set.
pub fn env_plan(host: impl Iterator<Item = String>, env: &[(String, String)], named: bool) -> (Vec<String>, Vec<(String, String)>) {
    let remove = if named { host.filter(|k| unset_env(k)).collect() } else { STORE_ENV.iter().map(|k| k.to_string()).collect() };
    (remove, env.to_vec())
}
/// A provider child's own variables name a profile's store (a named account's child): every provider credential it would inherit from the hub goes, so only
/// its own login reaches it (agents/proc.rs `spawn_with`: a turn, a title). A sync account's token is set again after, from its own environment.
pub fn scrub_for(env: &[(String, String)]) -> Vec<String> {
    if !env.iter().any(|(k, _)| STORE_ENV.contains(&k.as_str())) { return vec![] }
    std::env::vars_os().filter_map(|(k, _)| k.into_string().ok()).filter(|k| unset_env(k)).collect()
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
                        Some(a) if a.iter().all(|p| valid_id(p["id"].as_str().unwrap_or("")) && matches!(p["provider"].as_str(), Some("claude" | "codex")) && p["epoch"].as_u64().map_or(false, |e| e >= 1) && kind_ok(p)) => {
                            for p in a { inner.profiles.push(Profile { id: p["id"].as_str().unwrap().into(), provider: p["provider"].as_str().unwrap().into(), label: label(p["label"].as_str().unwrap_or("")), epoch: p["epoch"].as_u64().unwrap(),
                                fingerprint: p["fingerprint"].as_str().unwrap_or("").into(), store: p["store"].as_str().filter(|s| valid_id(s)).unwrap_or("").into(), state: if p["expiredAt"].as_u64().unwrap_or(0) > 0 { "expired".into() } else { "unchecked".into() }, identity: saved_kind(p).identity, catalog: vec![], allowance: None, checked_at: 0,
                                message: if p["expiredAt"].as_u64().unwrap_or(0) > 0 { EXPIRED_SHORT.into() } else { String::new() }, expired_at: p["expiredAt"].as_u64().unwrap_or(0), proven_at: 0, ..saved_kind(p) }) }
                        }
                        _ => inner.load_error = "The private account registry could not be verified.".into(),
                    }
                }
            }
        }
        // the usage read with the login's own token, on a real hub only (a test's, a stand-in's and a copy's read the CLI)
        // (BACKPLANE_USAGE_READ=1: a rig with stand-in agents reads the logins copied into its temp home for real; a copy never does)
        let real = !cfg!(test) && (std::env::var_os("BACKPLANE_AGENTS").map_or(true, |v| v.is_empty()) || std::env::var("BACKPLANE_USAGE_READ").map_or(false, |v| v == "1")) && std::env::var("BACKPLANE_COPY").map_or(true, |v| v != "1");
        let usage = real.then(|| (super::claude_usage::curl(), super::claude_usage::default_dir()));
        Accounts { root, inner: Mutex::new(inner), resolve, models_get: Mutex::new(real.then(super::claude_usage::curl_models)), usage: Mutex::new(usage), usage_locks: Mutex::new(HashMap::new()), tokens: Mutex::new(Arc::new(VaultTokens)) }
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
                if !kind_ok(p) { continue }
                g.profiles.push(Profile { id: id.into(), provider: provider.into(), label: label(p["label"].as_str().unwrap_or("")), epoch, fingerprint: p["fingerprint"].as_str().unwrap_or("").into(), store, state, identity: saved_kind(p).identity, catalog: vec![], allowance: None, checked_at: 0, message, expired_at, proven_at: 0, ..saved_kind(p) });
                n += 1;
            }
        })?;
        Ok(n)
    }
    /// Writes the registry (temp file, then rename). The revision moves only when the file is good; a failure is the op's failure.
    fn save(&self, g: &mut Inner) -> Result<(), ()> {
        let body = json!({"version":1,"profiles":g.profiles.iter().map(|p| { let mut o = json!({"id":p.id,"provider":p.provider,"label":p.label,"epoch":p.epoch,"fingerprint":p.fingerprint,"store":p.store}); if p.expired_at > 0 && p.state == "expired" { o["expiredAt"] = json!(p.expired_at) }
            // a sync account's identity is what it was made with (its token cannot ask who it is): kept, as are where it came from and its token's dates. Never the token.
            if p.kind == SYNC { o["kind"] = json!(SYNC); o["origin"] = json!(p.origin); o["madeAt"] = json!(p.made_at); o["expiresAt"] = json!(p.expires_at); o["email"] = p.identity["email"].clone(); o["plan"] = p.identity["plan"].clone(); }
            o }).collect::<Vec<_>>()}).to_string();
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
    /// A test's token store (the hub's is the vault).
    pub fn set_tokens(&self, t: Arc<dyn Tokens>) { *self.tokens.lock().unwrap() = t }
    /// A sync account's token: the one a sign-in just caught while it is checked, else the kept one.
    pub fn token(&self, id: &str) -> Option<String> {
        if let Some(t) = self.inner.lock().unwrap().caught.get(id).cloned() { return Some(t) }
        let t = self.tokens.lock().unwrap().clone(); t.get(id)
    }
    /// The environment a profile's CLI runs with: the host's, minus every provider credential variable, plus the profile's own store; a sync account's
    /// child also gets its token (`CLAUDE_CODE_OAUTH_TOKEN`), and no other child ever does (`unset_env`).
    pub fn environment(&self, p: Option<&Profile>) -> Result<Vec<(String, String)>, ()> {
        let Some(p) = p else { return Ok(vec![]) };
        let mut env = self.store_environment(p)?;
        if p.kind == SYNC { env.push(("CLAUDE_CODE_OAUTH_TOKEN".into(), self.token(&p.id).filter(|t| !t.is_empty()).ok_or(())?)); }
        Ok(env)
    }
    /// `environment` without the token (the sign-in that makes one).
    fn store_environment(&self, p: &Profile) -> Result<Vec<(String, String)>, ()> {
        let mut env: Vec<(String, String)> = vec![("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".into(), "1".into()), ("DISABLE_AUTOUPDATER".into(), "1".into()), ("MCP_TOOL_TIMEOUT".into(), "86400000".into()), ("CLAUDE_CODE_ENABLE_TODO_TOOLS".into(), "1".into())];
        // a store that cannot be made or verified is a failure of the op: an empty value would let the CLI fall back to the person's own login
        let dir = self.directory(p).map_err(|_| ())?.to_string_lossy().to_string();
        if dir.is_empty() { return Err(()) }
        if p.provider == "codex" { env.push(("CODEX_HOME".into(), dir)); env.push(("RUST_LOG".into(), "error".into())); } else { env.push(("CLAUDE_CONFIG_DIR".into(), dir)); }
        Ok(env)
    }

    /// A test's account: saved, signed in (the stand-in CLI reads `login.json` / `auth.json` from its store) and connected, with no sign-in ceremony.
    #[cfg(test)]
    /// A test's catalog for a planted profile (what the CLI lists: aliases such as "opus").
    #[cfg(test)] pub fn set_catalog(&self, id: &str, ids: &[&str]) { let _ = self.update(id, |p| p.catalog = ids.iter().map(|i| serde_json::json!({"id":i})).collect()); }
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
    async fn cli(&self, p: Option<&Profile>, program: &str, args: &[&str]) -> Result<(i32, Value), ()> { self.cli_within(CLI_LIMIT, p, program, args).await }
    /// `cli` with its own time limit: the read AND the wait for the exit are under it (#1108: the wait had none, and a child exit the hub missed held the
    /// account's job for ever). On expiry the child is killed and this is an error; the child is killed on drop too, so a cancelled caller leaves none.
    async fn cli_within(&self, limit: Duration, p: Option<&Profile>, program: &str, args: &[&str]) -> Result<(i32, Value), ()> {
        let prog = (self.resolve)(program).map_err(|_| ())?;
        let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Agent, &prog);
        c.args(args); c.current_dir(&self.root); apply_env(&mut c, &self.environment(p)?, p.is_some());
        c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
        let mut child = spawn(&mut c).await.map_err(|_| ())?;
        let mut out = child.stdout.take().ok_or(())?;
        let mut buf = Vec::new();
        let ran = tokio::time::timeout(limit, async {
            let mut chunk = [0u8; 8192];
            loop { let n = out.read(&mut chunk).await.map_err(|_| ())?; if n == 0 { break } buf.extend_from_slice(&chunk[..n]); if buf.len() > 131072 { return Err(()) } }
            Ok::<i32, ()>(child.wait().await.map_err(|_| ())?.code().unwrap_or(-1))
        }).await;
        let code = match ran { Ok(Ok(code)) => code, _ => { let _ = child.start_kill(); return Err(()) } };
        let data = if args.get(1) == Some(&"logout") { json!({}) } else { serde_json::from_slice(&buf).map_err(|_| ())? };
        Ok((code, data))
    }

    /// Claude's stream-json control requests (`initialize` -> models; `get_usage`): one child, the answers it gives, 12 s, nothing else read.
    async fn claude_control(&self, p: Option<&Profile>, want: &str) -> Option<Value> { self.claude_read(p, want).await.ok() }
    /// The CLI's own control reply (`models`, or `usage` after `initialize`), or the plain reason there was none.
    async fn claude_read(&self, p: Option<&Profile>, want: &str) -> Result<Value, String> {
        let prog = (self.resolve)("claude").map_err(|_| "The Claude CLI was not found on this machine.".to_string())?;
        let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Agent, &prog);
        c.args(claude_pre(p)); c.args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"]);
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
    /// A test's usage request and CLI store (the real hub's are set in `open`).
    #[cfg(test)]
    pub fn set_usage(&self, get: super::claude_usage::Get, default_dir: Option<PathBuf>) { *self.usage.lock().unwrap() = Some((get, default_dir)) }
    /// The usage endpoint asked with the login's own token (claude_usage.rs), or None when this login has no token the hub can read (no store file: a keychain
    /// login, an API key) and the CLI is asked instead. An expired token is refreshed by the CLI itself first: one start of it in that store, whose startup
    /// refreshes it under the CLI's own lock; a 401 gets the same once, in case the token was replaced under the read.
    async fn usage_direct(&self, p: Option<&Profile>) -> Option<Result<Value, String>> { self.oauth_read(p, false).await }
    /// `usage_direct`, or the models list (`models`) asked the same way.
    async fn oauth_read(&self, p: Option<&Profile>, models: bool) -> Option<Result<Value, String>> {
        use super::claude_usage as cu;
        let (get, default_dir) = self.usage.lock().unwrap().clone()?;
        let get = if models { self.models_get.lock().unwrap().clone()? } else { get };
        // a sync token is the provider's to refresh (it is never): asked as it is
        if let Some(p) = p.filter(|p| p.kind == SYNC) {
            let tok = self.token(&p.id)?;
            return Some(match get(tok).await { Ok((200, body)) if body.is_object() => Ok(body), Ok((200, _)) => Err("Claude's usage service answered with something else than usage.".into()), Ok((code, _)) => Err(cu::refusal(code)), Err(e) => Err(e) })
        }
        let dir = match p { Some(p) => self.directory(p).ok()?, None => default_dir? };
        let lock = self.usage_locks.lock().unwrap().entry(p.map_or(String::new(), |p| p.id.clone())).or_default().clone();
        let _one = lock.lock().await;
        let mut login = cu::read_login(&dir)?;
        if !login.fresh(now_ms()) {
            let _ = self.claude_read(p, "models").await;
            login = cu::read_login(&dir)?;
            if !login.fresh(now_ms()) { return None }
        }
        let mut r = get(login.token.clone()).await;
        if matches!(r, Ok((401, _))) {
            let _ = self.claude_read(p, "models").await;
            if let Some(again) = cu::read_login(&dir).filter(|l| l.token != login.token && l.fresh(now_ms())) { r = get(again.token).await }
        }
        Some(match r { Ok((200, body)) if body.is_object() => Ok(body), Ok((200, _)) => Err("Claude's usage service answered with something else than usage.".into()), Ok((code, _)) => Err(cu::refusal(code)), Err(e) => Err(e) })
    }
    /// The windows Claude reports for a profile (or the default login), its extra usage, and where they were read; or why there are none.
    async fn claude_usage(&self, p: Option<&Profile>) -> Result<Usage, String> {
        use super::claude_usage as cu;
        if let Some(r) = self.usage_direct(p).await {
            let body = r?; let w = cu::windows(&body);
            return if w.is_empty() { Err(NO_WINDOWS.into()) } else { Ok(Usage { windows: w, extra: cu::extra(&body), source: "claude-oauth-usage" }) }
        }
        let r = self.claude_read(p, "usage").await?;
        if r["rate_limits_available"] != true { return Err(NO_LIMITS.into()) }
        let w = Self::claude_windows(&r);
        if w.is_empty() { Err(NO_WINDOWS.into()) } else { Ok(Usage { windows: w, extra: cu::extra(&r["rate_limits"]), source: "claude-cli-usage" }) }
    }
    async fn claude_models(&self, p: &Profile) -> Vec<Value> {
        let Some(r) = self.claude_control(Some(p), "models").await else { return vec![] };
        // the CLI's list lags a model the account already has: the API's list (the login's own token, read-only) adds the newest of each family
        let api = match self.oauth_read(Some(p), true).await { Some(Ok(b)) => b, _ => Value::Null };
        super::models_probe::claude_extend(r["models"].as_array().cloned().unwrap_or_default(), &api).into_iter().take(40).filter(|x| x["value"].is_string()).map(|x| json!({"id":label(x["value"].as_str().unwrap_or("")),"label":label(x["displayName"].as_str().or(x["value"].as_str()).unwrap_or("")),
            "efforts":x["supportedEffortLevels"].as_array().map(|a| a.iter().take(12).map(|y| json!(label(y.as_str().unwrap_or("")))).collect::<Vec<_>>()).unwrap_or_default()})).collect()
    }
    /// accounts.js claudeWindows: the windows of Claude Code's `get_usage` answer (its `rate_limits` is the usage endpoint's body).
    pub fn claude_windows(reading: &Value) -> Vec<Value> {
        if reading["rate_limits_available"] != true { return vec![] }
        super::claude_usage::windows(&reading["rate_limits"])
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
        if p.kind == SYNC {
            // the CLI says it runs on the token it was given (`authMethod: oauth_token`; it cannot say whose: the identity is the one the account was made with),
            // and the usage read with it is the provider's answer. No model prompt.
            let (code, data) = self.cli(Some(&p), "claude", &["auth", "status"]).await.map_err(|_| "Claude sync token could not be checked".to_string())?;
            if code != 0 || data["loggedIn"] != true || data["authMethod"] != "oauth_token" { return Err("Claude Code did not take this account's sync token.".into()) }
            let dir = self.directory(&p).map_err(|e| e.to_string())?;
            if data["configDirectory"].as_str().map(|d| crate::plat::fs::canonicalize(d).unwrap_or_else(|_| PathBuf::from(d))) != Some(crate::plat::fs::canonicalize(&dir).unwrap_or(dir)) { return Err(DIRECTORY_ERR.into()) }
            identity = json!({"email":p.identity["email"].as_str().unwrap_or(""),"plan":p.identity["plan"].as_str().unwrap_or(""),"auth":"oauth_token"});
            fp_identity = json!([identity["email"], format!("sync:{}", p.id)]);
            let ck = format!("catalog:{}", p.id);
            catalog = if p.catalog.is_empty() || now_s().saturating_sub(last_of(&self.inner, &ck)) >= 3600 {
                self.inner.lock().unwrap().refresh_at.insert(ck, now_s());
                let c = self.claude_models(&p).await; if c.is_empty() { p.catalog.clone() } else { c }
            } else { p.catalog.clone() };
            allowance = p.allowance.clone();
            let at = last(&format!("usage:{}", p.id));
            if new_login || asked || p.state == "expired" || at == 0 || now_s().saturating_sub(at) >= 300 {
                self.inner.lock().unwrap().refresh_at.insert(format!("usage:{}", p.id), now_s());
                match self.claude_usage(Some(&p)).await {
                    Ok(u) => { fact = Fact::Proven; allowance = Some(u.allowance(now_s(), "")) }
                    Err(reason) => {
                        if fb_auth(&reason) { fact = Fact::Refused } else if reason == NO_WINDOWS || reason == NO_LIMITS { fact = Fact::Proven }
                        self.inner.lock().unwrap().refresh_at.insert(format!("usage:{}", p.id), now_s().saturating_sub(240));
                        if allowance.as_ref().map_or(true, |a| a["state"] != "ok") { allowance = Some(json!({"provider":"claude","state":"unavailable","at":0,"attempted_at":now_s(),"source":"claude-cli-usage","reason":reason,"windows":[]})) }
                    }
                }
            }
            // past its end the token is dead whatever the CLI says
            if p.expires_at > 0 && now_s() >= p.expires_at { fact = Fact::Refused }
        } else if p.provider == "claude" {
            let (code, data) = self.cli(Some(&p), "claude", &["auth", "status"]).await.map_err(|_| "Claude subscription sign-in could not be verified".to_string())?;
            if code != 0 || data["loggedIn"] != true || data["authMethod"] != "claude.ai" { return Err("Claude subscription sign-in could not be verified; Console profiles and API credentials use a different store.".into()) }
            let dir = self.directory(&p).map_err(|e| e.to_string())?;
            if data["configDirectory"].as_str().map(|d| crate::plat::fs::canonicalize(d).unwrap_or_else(|_| PathBuf::from(d))) != Some(crate::plat::fs::canonicalize(&dir).unwrap_or(dir)) { return Err(DIRECTORY_ERR.into()) }
            identity = json!({"email":label(data["email"].as_str().unwrap_or("")),"plan":label(data["subscriptionType"].as_str().unwrap_or("")),"auth":"claude.ai"});
            fp_identity = json!([identity["email"], label(data["accountUuid"].as_str().or(data["userId"].as_str()).unwrap_or("")), label(data["orgUuid"].as_str().or(data["organizationId"].as_str()).unwrap_or(""))]);
            // the catalog is read again each hour (a model the provider adds shows without a restart), and the old one stays when the read gives none
            let ck = format!("catalog:{}", p.id);
            catalog = if p.catalog.is_empty() || now_s().saturating_sub(last_of(&self.inner, &ck)) >= 3600 {
                self.inner.lock().unwrap().refresh_at.insert(ck, now_s());
                let c = self.claude_models(&p).await; if c.is_empty() { p.catalog.clone() } else { c }
            } else { p.catalog.clone() };
            allowance = p.allowance.clone();
            let at = last(&format!("usage:{}", p.id));
            // a login found dead is read every check: the usage read is the one request that proves it works again (no inference request is made)
            if new_login || asked || p.state == "expired" || at == 0 || now_s().saturating_sub(at) >= 300 {
                self.inner.lock().unwrap().refresh_at.insert(format!("usage:{}", p.id), now_s());
                let read = self.claude_usage(Some(&p)).await;
                // an answer without windows is still an answer the provider gave with this login
                match &read { Ok(_) => fact = Fact::Proven, Err(reason) if fb_auth(reason) => fact = Fact::Refused, Err(reason) if reason == NO_WINDOWS || reason == NO_LIMITS => fact = Fact::Proven, Err(_) => {} }
                match read {
                    Ok(u) => { allowance = Some(u.allowance(now_s(), "")) }
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
        // a sync account's identity is fixed when it is made: a new token (a renewal) is the same account, and threads on it stay on it
        if p.kind != SYNC && (new_login || (!p.fingerprint.is_empty() && p.fingerprint != digest)) { p.epoch += 1; if !new_login { catalog = vec![]; allowance = None; } }
        p.fingerprint = digest; p.identity = identity; p.catalog = catalog; p.allowance = allowance; p.checked_at = now_s();
        if fact == Fact::Proven { p.proven_at = now_s() }
        p.state = after(if new_login { "unchecked" } else { &p.state }, fact).into();
        if p.state == "expired" { if p.expired_at == 0 { p.expired_at = now_s() } p.message = EXPIRED_SHORT.into() } else { p.expired_at = 0; p.message.clear() }
        Ok(p)
    }
}

pub fn parse_iso(s: &str) -> Option<u64> {
    // 2026-11-05T12:00:00Z or with fractional seconds / +00:00
    let t = s.trim(); if t.len() < 20 { return None }
    let (y, mo, d, h, mi, se): (i64, i64, i64, i64, i64, i64) = (t.get(0..4)?.parse().ok()?, t.get(5..7)?.parse().ok()?, t.get(8..10)?.parse().ok()?, t.get(11..13)?.parse().ok()?, t.get(14..16)?.parse().ok()?, t.get(17..19)?.parse().ok()?);
    let (y2, m2) = if mo <= 2 { (y - 1, mo + 9) } else { (y, mo - 3) };
    let days = 365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 + (153 * m2 + 2) / 5 + d - 719469 + 0;
    u64::try_from(days * 86400 + h * 3600 + mi * 60 + se).ok()
}

/// Why profile `id` takes no new op now (None: it may start). `check_busy`: a turn running on it is a refusal too.
fn busy_refusal(g: &Inner, id: &str, check_busy: bool) -> Option<&'static str> {
    match g.jobs.get(id) { Some(j) if j.starts_with("attempt:") => return Some(SIGNING_IN), Some(_) => return Some(CHECKING), None => {} }
    if g.launching.get(id).copied().unwrap_or(0) > 0 { return Some(STARTING) }
    if check_busy && g.busy.get(id).copied().unwrap_or(0) > 0 { return Some(TURN_RUNNING) }
    None
}
/// An op in progress on a profile (`Accounts::hold`): its job is ended when this drops, however the op ends.
struct Job<'a> { a: &'a Accounts, id: String, what: String }
impl Drop for Job<'_> { fn drop(&mut self) { self.a.end(&self.id, &self.what) } }

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
    /// Thread `th`'s provider process runs on profile `id` from now on (accounts_hub::Busy::for_thread): the token ends it in `run_off`, and only it, so the guard of a process
    /// that was replaced cannot clear its successor's record.
    pub fn run_on(&self, th: &str, id: &str) -> u64 { let mut g = self.inner.lock().unwrap(); g.runs_seq += 1; let t = g.runs_seq; g.runs_on.insert(th.to_string(), (id.to_string(), t)); t }
    pub fn run_off(&self, th: &str, token: u64) { let mut g = self.inner.lock().unwrap(); if g.runs_on.get(th).map_or(false, |(_, t)| *t == token) { g.runs_on.remove(th); } }
    /// Tests: a routine check (the 5-minute refresh) holds profile `id` until `release_check`, so a start on it waits in launch_plan, running and on no login. Taken once the
    /// profile is free: the hub's own refresh checks a planted profile on its first tick, and under load that check can still be running here (it failed a full gate).
    #[cfg(test)] pub async fn hold_check(&self, id: &str) -> bool {
        let end = std::time::Instant::now() + Duration::from_secs(120);
        loop { if self.begin(id, "refresh", false).is_ok() { return true } if std::time::Instant::now() >= end { return false } tokio::time::sleep(Duration::from_millis(20)).await }
    }
    #[cfg(test)] pub fn release_check(&self, id: &str) { self.end(id, "refresh") }
    /// The profile thread `th`'s live provider process runs on; None while none runs on a saved profile (a turn still choosing its login, or one on the CLI's own).
    pub fn runs_on(&self, th: &str) -> Option<String> { self.inner.lock().unwrap().runs_on.get(th).map(|(id, _)| id.clone()) }
    pub fn child_ended(&self, id: &str) { let mut g = self.inner.lock().unwrap(); if let Some(n) = g.busy.get_mut(id) { *n = n.saturating_sub(1); if *n == 0 { g.busy.remove(id); } } }

    /// A request on a profile: refused while another is in progress, or a launch is queued (accounts.js `jobs` / `launchQueues`). `check_busy`: also while a provider
    /// child (a turn) runs on it; refresh asks that (login checks `busy` and `launching` itself in `start`, disconnect and remove check it themselves), a status and a rename do not.
    /// Err: why, in the words Settings shows (#1108: a refusal said only "Account operation failed").
    fn begin(&self, id: &str, what: &str, check_busy: bool) -> Result<(), &'static str> {
        let mut g = self.inner.lock().unwrap();
        if let Some(why) = busy_refusal(&g, id, check_busy) { return Err(why) }
        g.jobs.insert(id.to_string(), what.to_string()); Ok(())
    }
    fn end(&self, id: &str, what: &str) { let mut g = self.inner.lock().unwrap(); if g.jobs.get(id).map(String::as_str) == Some(what) { g.jobs.remove(id); } }
    /// `begin`, ended when the guard drops: an op that fails, times out or whose future is dropped never leaves the account marked in progress (#1108).
    fn hold(&self, id: &str, what: &str, check_busy: bool) -> Result<Job<'_>, &'static str> { self.begin(id, what, check_busy)?; Ok(Job { a: self, id: id.to_string(), what: what.to_string() }) }

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
        if op == "add" || op == "synctoken" {
            // synctoken: a sync account made from a connected Claude account (its name and who it is), and its `claude setup-token` started at once
            let from = if op == "synctoken" { let f = self.profile(r["profile"].as_str().unwrap_or("")).map_err(|_| None)?; if f.provider != "claude" || f.kind == SYNC || f.state != "connected" { return Err(None) } Some(f) } else { None };
            let sync = from.is_some() || r["kind"] == SYNC;
            let provider = if from.is_some() { "claude" } else { r["provider"].as_str().unwrap_or("") };
            let lab = match &from { Some(f) => label(&format!("{} sync", f.label)), None => label(r["label"].as_str().unwrap_or("")) };
            let identity = match &from { Some(f) => json!({"email":f.identity["email"],"plan":f.identity["plan"],"auth":"oauth_token"}), None if sync => json!({"email":label(r["email"].as_str().unwrap_or("")),"plan":"","auth":"oauth_token"}), None => json!({}) };
            let p = {
                let mut g = self.inner.lock().unwrap();
                if !matches!(provider, "claude" | "codex") || (sync && provider != "claude") || lab.is_empty() || g.profiles.len() >= 12 { return Err(None) }
                let p = Profile { id: new_id(), provider: provider.into(), label: lab, epoch: 1, state: "disconnected".into(), identity, kind: if sync { SYNC.into() } else { String::new() }, ..Profile::default() };
                self.directory(&p).map_err(|_| None)?;
                g.profiles.push(p.clone()); if self.save(&mut g).is_err() { g.profiles.pop(); return Err(None) } p
            };
            // synctoken, or an add that asks (`start`, Settings' Add Claude (sync)): its sign-in starts at once
            if from.is_some() || (sync && r["start"] == true) { self.start(&p, "sync").await.map_err(|why| why.map(str::to_string))?; }
            let mut s = self.snapshot(); s["made"] = json!(p.id); return Ok(s);
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
            "login" => { self.start(&p, if p.kind == SYNC { "sync" } else { r["mode"].as_str().unwrap_or("device") }).await.map_err(|why| why.map(str::to_string))?; return Ok(self.snapshot()) }
            _ => {}
        }
        // Delete is the way out of a stuck sign-in: its attempt is ended first (its child is told to stop), and a job left behind by an attempt that is gone is dropped
        if op == "remove" {
            let attempts: Vec<(String, Option<mpsc::UnboundedSender<Cmd>>)> = { let g = self.inner.lock().unwrap(); g.attempts.iter().filter(|a| a.profile == pid).map(|a| (a.id.clone(), g.cmds.get(&a.id).cloned())).collect() };
            for (aid, tx) in attempts { self.cancel(&aid, "cancelled", ""); if let Some(t) = tx { let _ = t.send(Cmd::Cancel); } }
            { let mut g = self.inner.lock().unwrap(); if g.jobs.get(&pid).map_or(false, |j| j.starts_with("attempt:")) { g.jobs.remove(&pid); } }
        }
        let job = self.hold(&pid, "status", false).map_err(|why| Some(why.to_string()))?;
        // why an op inside was refused (a turn running on the account), said instead of the generic failure
        let refused = std::sync::atomic::AtomicBool::new(false);
        let turn_runs = || { refused.store(true, std::sync::atomic::Ordering::Relaxed); Err(()) };
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
                "disconnect" if p.kind == SYNC => {
                    // a sync token is not a login the CLI can sign out of: it is forgotten here (on a machine that made it, its linked machines forget it too)
                    if self.busy(&pid) { return turn_runs() }
                    self.update(&pid, |slot| { slot.state = "disconnected".into(); slot.catalog = vec![]; slot.allowance = None; slot.made_at = 0; slot.expires_at = 0; })?;
                    let t = self.tokens.lock().unwrap().clone(); t.del(&pid); Ok(())
                }
                "disconnect" => {
                    if self.busy(&pid) { return turn_runs() }
                    if p.provider == "claude" { let (code, _) = self.cli(Some(&p), "claude", &["auth", "logout"]).await?; if code != 0 { return Err(()) } }
                    else { let prog = (self.resolve)("codex").map_err(|_| ())?; let mut rpc = Rpc::open(&prog, &self.environment(Some(&p))?, true).await?; let r = async { rpc.initialize().await?; rpc.request("account/logout", json!({}), 12).await.map(|_| ()) }.await; rpc.close().await; r?; }
                    // the registry is written first: a store is removed only once the file no longer names it
                    self.update(&pid, |slot| { slot.epoch += 1; slot.state = "disconnected".into(); slot.fingerprint.clear(); slot.identity = json!({}); slot.catalog = vec![]; slot.allowance = None; })?;
                    self.remove_store(&pid, &p.store); Ok(())
                }
                "rename" => { let l = label(r["label"].as_str().unwrap_or("")); if l.is_empty() { return Err(()) } self.update(&pid, |slot| slot.label = l) }
                "remove" => {
                    if self.busy(&pid) { return turn_runs() }
                    let base = self.root.join(&pid); private_dir(&base).map_err(|_| ())?;
                    self.mutate_save(|g| { g.profiles.retain(|x| x.id != pid); g.attempts.retain(|a| a.profile != pid); })?;
                    if p.kind == SYNC { let t = self.tokens.lock().unwrap().clone(); t.del(&pid); }
                    let _ = std::fs::remove_dir_all(&base); Ok(())
                }
                _ => Err(()),
            }
        }.await;
        drop(job);
        res.map(|_| self.snapshot()).map_err(|_| refused.load(std::sync::atomic::Ordering::Relaxed).then(|| TURN_RUNNING.to_string()))
    }
    // ---- sign-in attempts ----
    /// Err(Some(why)) when refused because something else is running on the account, Err(None) for any other failure.
    async fn start(self: &Arc<Self>, p: &Profile, mode: &str) -> Result<(), Option<&'static str>> {
        let aid = new_id();
        let (rx, cand) = {
            let mut g = self.inner.lock().unwrap();
            let live: Vec<String> = g.jobs.iter().filter(|(_, v)| v.starts_with("attempt:")).map(|(k, _)| k.clone()).collect();
            g.attempts.retain(|a| live.contains(&a.profile));
            // a dead login is signed in again even while a process of it lives: that process can do nothing with it, and the new sign-in has a store of its own
            let dead = g.profiles.iter().any(|x| x.id == p.id && x.state == "expired");
            if let Some(why) = busy_refusal(&g, &p.id, !dead) { return Err(Some(why)) }
            if g.attempts.len() >= 8 { return Err(None) }
            // a sync account's store holds no credentials (its sessions only): a new token keeps it
            let store = if p.kind == SYNC { p.store.clone() } else { aid.clone() };
            let cand = Profile { store: store.clone(), ..p.clone() };
            g.attempts.push(Attempt { id: aid.clone(), profile: p.id.clone(), state: "starting".into(), url: String::new(), user_code: String::new(), expires_at: now_ms() + 600_000, message: String::new(), previous_state: p.state.clone(), store });
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
    fn cancel(&self, aid: &str, state: &str, reason: &str) { self.end_as(aid, state, if state == "failed" { failure_text(reason) } else { String::new() }) }
    /// A sync sign-in that failed: why, in this page's words, then the line the CLI itself showed (no token is ever in it: [`said_line`]).
    fn cancel_said(&self, aid: &str, reason: &str, said: &str) {
        let why = match reason {
            "code_rejected" => "Claude did not accept that code. It may be incomplete, expired, already used, or from an earlier sign-in. Press Make the sync token and paste the code from that new sign-in.",
            "no_answer" => "Claude Code did not finish the sign-in after the code was sent. Press Make the sync token to try again.",
            "" => "The sync token could not be made. Press Make the sync token to try again.",
            r => return self.end_as(aid, "failed", with_said(&failure_text(r), said)),
        };
        self.end_as(aid, "failed", with_said(why, said))
    }
    fn end_as(&self, aid: &str, state: &str, message: String) {
        let mut g = self.inner.lock().unwrap();
        let Some(i) = g.attempts.iter().position(|a| a.id == aid) else { return };
        let prof = g.attempts[i].profile.clone();
        if g.jobs.get(&prof).map(String::as_str) != Some(format!("attempt:{aid}").as_str()) { return }
        g.jobs.remove(&prof);
        let (store, prev) = (g.attempts[i].store.clone(), g.attempts[i].previous_state.clone());
        { let a = &mut g.attempts[i]; a.state = state.into(); a.url.clear(); a.user_code.clear(); a.message = message; }
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
                // a sync token is kept once it is checked: in the vault, under the account's id (a renewal replaces the old one)
                if cand.kind == SYNC {
                    let Some(tok) = self.inner.lock().unwrap().caught.get(&cand.id).cloned() else { return self.cancel(aid, "failed", "verification") };
                    let t = self.tokens.lock().unwrap().clone();
                    if !t.put(&cand.id, &cand.label, &tok) { return self.cancel(aid, "failed", "storage") }
                }
                let old_store = self.profile(&a.profile).map(|p| p.store).unwrap_or_default();
                // one lock: the profile takes the new store and the attempt is done in the same step, so a cancel cannot land between and put the old state back over it
                let saved = self.mutate_save(|g| {
                    if let Some(slot) = g.profiles.iter_mut().find(|p| p.id == a.profile) { *slot = np.clone(); slot.store = a.store.clone(); }
                    g.jobs.remove(&a.profile); g.cmds.remove(aid);
                    if let Some(att) = g.attempts.iter_mut().find(|x| x.id == aid) { att.state = "done".into(); att.url.clear(); att.user_code.clear(); }
                });
                if saved.is_err() { return self.cancel(aid, "failed", "storage") }
                if !old_store.is_empty() && old_store != a.store { self.remove_store(&a.profile, &old_store); }
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
        } else if mode == "sync" {
            self.run_setup_token(&aid, cand, cmds, deadline).await
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
            let scan = scan_claude;
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
/// What a Claude sign-in printed so far: why it failed if it said (into `reason`), and its authorization link once it shows one.
fn scan_claude(output: &str, reason: &mut String) -> Option<String> {
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
}
/// How long after the pasted code its Enter is typed: a key of its own, never read in one go with the code.
const ENTER_AFTER: Duration = Duration::from_millis(150);
/// How long a sent code's answer (the token, or the CLI's error) is waited for: the exchange is a request or two.
const ANSWER_WAIT: Duration = if cfg!(test) { Duration::from_secs(4) } else { Duration::from_secs(90) };
/// One line a terminal program showed, as words, at most 240 characters; None for a line that holds a token or the masked code, or no words.
pub fn said_line(line: &str) -> Option<String> {
    let l: String = line.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    // nor the pasted code, which the CLI echoes masked (`*****…` and its last characters)
    if l.contains("sk-ant") || l.contains("****") || l.chars().filter(|c| c.is_alphabetic()).count() < 3 { return None }
    Some(if l.chars().count() > 240 { format!("{}…", l.chars().take(240).collect::<String>()) } else { l })
}
fn lines(text: &str) -> impl DoubleEndedIterator<Item = &str> { text.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()) }
/// The CLI's last line on its screen, for a failure the page names.
pub fn last_said(text: &str) -> String { lines(text).rev().find_map(said_line).unwrap_or_default() }
/// `claude setup-token`'s error screen after a code ("OAuth error: … Press Enter to retry."): its error line. It waits there for Enter; it never exits.
pub fn refusal(text: &str) -> Option<String> {
    if !text.to_lowercase().contains("press enter to retry") { return None }
    Some(lines(text).rev().filter(|l| l.to_lowercase().contains("error")).find_map(said_line).unwrap_or_else(|| last_said(text)))
}
fn with_said(why: &str, said: &str) -> String { if said.is_empty() { why.to_string() } else { format!("{why} Claude Code said: \u{201c}{said}\u{201d}") } }
impl Accounts {
    /// A sync account's sign-in: `claude setup-token` on a terminal of its own (it draws its screen and reads the pasted code as a terminal program), its link
    /// shown, the code the person pastes typed into it, and the token it prints caught straight into the attempt: never logged, never in the snapshot, the
    /// timeline or a client. It is checked (`finish`) and only then kept, in the vault.
    async fn run_setup_token(self: &Arc<Self>, aid: &str, mut cand: Profile, mut cmds: mpsc::UnboundedReceiver<Cmd>, deadline: tokio::time::Instant) {
        let Ok(prog) = (self.resolve)("claude") else { return self.cancel(aid, "failed", "cli_start") };
        let Ok(mut env) = self.store_environment(&cand) else { return self.cancel(aid, "failed", "cli_start") };
        env.push(("BROWSER".into(), "/bin/true".into()));
        let (remove, set) = env_plan(std::env::vars_os().filter_map(|(k, _)| k.into_string().ok()), &env, true);
        // wide, so neither its link nor its token is wrapped across lines
        let sp = crate::term::pty::Pty::spawn_as(crate::proc::Kind::Agent, &prog.to_string_lossy(), &["setup-token".to_string()], &self.root.to_string_lossy(), 2000, 50,
            &mut |c| { for k in &remove { c.env_remove(k); } for (k, v) in &set { c.env(k, v); } });
        let Ok(mut sp) = sp else { return self.cancel(aid, "failed", "cli_start") };
        let (mut output, mut reason, mut open) = (String::new(), String::new(), true);
        let mut shown = false;
        // where the output stood when the code was typed, and until when an answer to it is waited for
        let mut sent: Option<(usize, tokio::time::Instant)> = None;
        let far = deadline;
        let caught: Result<(String, u64), String> = loop {
            let answer_by = sent.map_or(far, |(_, t)| t.min(far));
            tokio::select! {
                chunk = sp.output.recv(), if open => match chunk {
                    Some(b) => {
                        output.push_str(&String::from_utf8_lossy(&b)); if output.len() > 262144 { break Err(String::new()) }
                        let clean = strip_ansi(&output);
                        if let Some(t) = catch_token(&clean) {
                            // the whole token: what follows it on its line has been drawn too
                            if clean[clean.find(&t).unwrap_or(0) + t.len()..].contains(['\n', '\r']) { break Ok((t, valid_for(&clean).unwrap_or(TOKEN_LIFE))) }
                        }
                        if !shown { if let Some(u) = scan_claude(&clean, &mut reason) { if self.current(aid).is_some() { self.set_waiting(aid, u, String::new()); shown = true } } } else { let _ = scan_claude(&clean, &mut reason); }
                        // a refused code: the CLI draws its error and "Press Enter to retry", then waits there for good (it does not exit)
                        if let Some((at, _)) = sent { if let Some(said) = refusal(&strip_ansi(&output[at..])) { break Err(said) } }
                    }
                    None => open = false,
                },
                cmd = cmds.recv() => match cmd {
                    Some(Cmd::Respond(code)) => {
                        // as a bracketed paste when the CLI asked for those, then Enter on its own: text and Enter in one write is pasted text to Claude Code's
                        // input, and a code that long never submits (Nov 11: hung at "Checking the code…")
                        let paste = output.contains("\u{1b}[?2004h");
                        let at = output.len();
                        let _ = sp.pty.write(if paste { format!("\u{1b}[200~{code}\u{1b}[201~") } else { code }.into_bytes());
                        tokio::time::sleep(ENTER_AFTER).await;
                        let _ = sp.pty.write(b"\r".to_vec());
                        sent = Some((at, tokio::time::Instant::now() + ANSWER_WAIT));
                    }
                    Some(Cmd::Cancel) | None => { sp.pty.force(); return }
                },
                _ = &mut sp.exit => {
                    while let Ok(Some(b)) = tokio::time::timeout(Duration::from_millis(200), sp.output.recv()).await { output.push_str(&String::from_utf8_lossy(&b)); }
                    let clean = strip_ansi(&output); let _ = scan_claude(&clean, &mut reason);
                    break catch_token(&clean).map(|t| (t, valid_for(&clean).unwrap_or(TOKEN_LIFE))).ok_or_else(|| last_said(&strip_ansi(&output[sent.map_or(0, |(at, _)| at)..])))
                },
                _ = tokio::time::sleep_until(answer_by) => {
                    sp.pty.force();
                    if sent.is_none() || answer_by >= far { return self.cancel(aid, "expired", "") }
                    if reason.is_empty() { reason = "no_answer".into() }
                    break Err(last_said(&strip_ansi(&output[sent.map_or(0, |(at, _)| at)..])))
                },
            }
        };
        sp.pty.force(); drop(output);
        let (token, life) = match caught { Ok(t) => t, Err(said) => return self.cancel_said(aid, &reason, &said) };
        let now = now_s(); cand.made_at = now; cand.expires_at = now + life;
        self.inner.lock().unwrap().caught.insert(cand.id.clone(), token);
        self.finish(aid, &cand, true, "").await;
        self.inner.lock().unwrap().caught.remove(&cand.id);
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
                else { claude_pre(Some(&np)).iter().map(|a| a.to_string()).collect() };
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
                Ok(u) => slot.allowance = Some(u.allowance(n, "")),
                Err(reason) => if slot.allowance.as_ref().map_or(true, |a| a["state"] != "ok") { slot.allowance = Some(json!({"provider":"claude","state":"unavailable","at":0,"attempted_at":n,"source":"claude-cli-usage","reason":reason,"windows":[]})) },
            } g.revision += 1; });
        }
        for id in ids {
            self.inner.lock().unwrap().refresh_at.insert(id.clone(), now);
            let dead = self.profile(&id).map_or(false, |p| p.state == "expired");
            let Ok(_job) = self.hold(&id, "refresh", !dead) else { continue };
            if let Ok(p) = self.profile(&id) { if let Ok(np) = self.verify(&p, false).await { let _ = self.update(&id, |slot| *slot = merge_checked(slot, np)); } }
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
                        Ok(u) => { let n = now_s(); let mut g = self.inner.lock().unwrap(); g.default_allowance = Some(u.allowance(n, &label(data["email"].as_str().unwrap_or("")))); g.revision += 1; }
                        Err(reason) => { let mut g = self.inner.lock().unwrap(); self.refresh_at_retry(&mut g); if g.default_allowance.as_ref().map_or(true, |a| a["state"] != "ok") { g.default_allowance = Some(json!({"provider":"claude","state":"unavailable","at":0,"attempted_at":now,"source":"claude-cli-usage","reason":reason,"windows":[]})); g.revision += 1; } }
                    }
                } else { let mut g = self.inner.lock().unwrap(); g.default_identity.clear(); g.default_allowance = Some(json!({"provider":"claude","state":"unavailable","at":0,"attempted_at":now,"source":"claude-cli-usage","windows":[]})); g.revision += 1; }
            }
        }
    }
}

// ---- sync accounts between the owner's machines (accounts_sync.rs carries them) ----

/// A token as one machine tells another: Claude Code's long-lived one, or "" (the account was signed out there).
fn token_ok(t: &str) -> bool { t.is_empty() || (t.starts_with("sk-ant-") && (20..=512).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')) }
/// What a linked machine's announcement does to this machine's profiles.
#[derive(Debug, Default)]
pub struct Merge { pub profiles: Vec<Profile>, pub put: Vec<(String, String, String)>, pub del: Vec<String>, pub removed: Vec<String>, pub changed: usize,
    /// what changed, one word each (`new`, `token`, `name`, `who`, `dates`, `signed out`, `removed`): for the hub's log, never a value
    pub why: Vec<&'static str> }
/// The sync accounts machine `origin` made, as it says they are now (`items`: `{id, label, email, plan, token, madeAt, expiresAt}`, every one it has): each is
/// added here with its id, or brought up to date (a renewed token, a new name), and the ones of `origin` it no longer names are removed. A profile of this machine's
/// own, or of another machine, is never touched. `have`: the token kept here now.
pub fn sync_merge(cur: &[Profile], origin: &str, items: &[Value], have: &dyn Fn(&str) -> Option<String>) -> Merge {
    let mut m = Merge { profiles: cur.to_vec(), ..Merge::default() };
    let mut named = vec![];
    for it in items.iter().take(40) {
        let (id, tok) = (it["id"].as_str().unwrap_or(""), it["token"].as_str().unwrap_or(""));
        let lab = label(it["label"].as_str().unwrap_or(""));
        if !valid_id(id) || lab.is_empty() || !token_ok(tok) || origin.is_empty() { continue }
        named.push(id.to_string());
        let ident = json!({"email":label(it["email"].as_str().unwrap_or("")),"plan":label(it["plan"].as_str().unwrap_or("")),"auth":"oauth_token"});
        let (made, ends) = (it["madeAt"].as_u64().unwrap_or(0), it["expiresAt"].as_u64().unwrap_or(0));
        let new_token = !tok.is_empty() && have(id).as_deref() != Some(tok);
        match m.profiles.iter_mut().find(|p| p.id == id) {
            Some(p) if p.kind != SYNC || p.origin != origin => continue,
            Some(p) => {
                // who it is, as the announcement says it (an identity this machine holds with more in it is the same account)
                let who = |i: &Value| (i["email"].as_str().unwrap_or("").to_string(), i["plan"].as_str().unwrap_or("").to_string());
                let before = (p.label.clone(), who(&p.identity), p.made_at, p.expires_at, p.state.clone());
                p.label = lab.clone(); p.identity = ident; p.made_at = made; p.expires_at = ends;
                if tok.is_empty() { if p.state != "disconnected" { p.state = "disconnected".into(); p.catalog = vec![]; p.allowance = None; } if have(id).is_some() { m.del.push(id.into()) } }
                else if new_token { m.put.push((id.into(), lab.clone(), tok.into())); if matches!(p.state.as_str(), "disconnected" | "expired") { p.state = "unchecked".into(); p.expired_at = 0; p.message.clear(); } }
                let after = (p.label.clone(), who(&p.identity), p.made_at, p.expires_at, p.state.clone());
                if new_token || before != after {
                    m.changed += 1;
                    m.why.push(if new_token { "token" } else if before.0 != after.0 { "name" } else if before.1 != after.1 { "who" } else if (before.2, before.3) != (after.2, after.3) { "dates" } else { "signed out" });
                }
            }
            None => {
                if m.profiles.len() >= 40 { continue }
                m.profiles.push(Profile { id: id.into(), provider: "claude".into(), label: lab.clone(), epoch: 1, state: if tok.is_empty() { "disconnected".into() } else { "unchecked".into() }, identity: ident,
                    kind: SYNC.into(), origin: origin.into(), made_at: made, expires_at: ends, ..Profile::default() });
                if !tok.is_empty() { m.put.push((id.into(), lab.clone(), tok.into())) }
                m.changed += 1; m.why.push("new");
            }
        }
    }
    let gone: Vec<String> = m.profiles.iter().filter(|p| p.kind == SYNC && p.origin == origin && !named.contains(&p.id)).map(|p| p.id.clone()).collect();
    m.profiles.retain(|p| !gone.contains(&p.id));
    m.changed += gone.len(); m.why.extend(gone.iter().map(|_| "removed")); m.del.extend(gone.iter().cloned()); m.removed = gone;
    m
}
impl Accounts {
    /// The sync accounts this machine made, as its linked machines are told them (with their tokens: the far link is signed and goes only to the owner's own
    /// machines). None while a token cannot be read (a locked vault): nothing is said then, so no machine removes what it holds.
    pub fn sync_out(&self) -> Option<Vec<Value>> {
        let t = self.tokens.lock().unwrap().clone(); if !t.open() { return None }
        let g = self.inner.lock().unwrap(); if !g.load_error.is_empty() { return None }
        let mut out = vec![];
        for p in g.profiles.iter().filter(|p| p.kind == SYNC && p.origin.is_empty()) {
            let tok = t.get(&p.id).unwrap_or_default();
            // a connected account whose token is not there is not one to remove elsewhere: say nothing until it is read
            if tok.is_empty() && !matches!(p.state.as_str(), "disconnected" | "connecting") { return None }
            if p.state == "connecting" && tok.is_empty() { continue } // being made: told once it has its token
            out.push(json!({"id":p.id,"label":p.label,"email":p.identity["email"],"plan":p.identity["plan"],"token":tok,"madeAt":p.made_at,"expiresAt":p.expires_at}));
        }
        Some(out)
    }
    /// What machine `origin` (its link id here) says its sync accounts are (`sync_merge`): tokens into this machine's vault, the registry written. Ok(how many changed).
    pub fn sync_in(&self, origin: &str, items: &[Value]) -> Result<usize, ()> {
        // one announcement at a time: two that arrive together (the ten-minute one and a home move's, Nov 11 on boat-hub) each read the registry before
        // the other wrote it, and both counted the account as new
        static ONE: Mutex<()> = Mutex::new(());
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        let t = self.tokens.lock().unwrap().clone(); if !t.open() { return Err(()) }
        let cur = { let g = self.inner.lock().unwrap(); if !g.load_error.is_empty() { return Err(()) } g.profiles.clone() };
        let m = sync_merge(&cur, origin, items, &|id| t.get(id));
        if m.changed == 0 { return Ok(0) }
        eprintln!("accounts: sync accounts from a linked machine: {}", m.why.join(", "));
        for (id, lab, tok) in &m.put { if !t.put(id, lab, tok) { return Err(()) } }
        for p in &m.profiles { if !m.removed.contains(&p.id) && p.kind == SYNC { let _ = self.directory(p); } }
        let mut np = m.profiles.clone();
        self.mutate_save(|g| {
            // what changed here meanwhile (a check's reading) stays on the profiles the announcement did not change
            for p in np.iter_mut() { if let Some(x) = g.profiles.iter().find(|x| x.id == p.id) { if p.kind != SYNC || p.origin != origin { *p = x.clone() } else { p.catalog = x.catalog.clone(); p.allowance = if p.state == "disconnected" { None } else { x.allowance.clone() }; p.checked_at = x.checked_at; p.fingerprint = x.fingerprint.clone(); p.store = x.store.clone(); p.epoch = x.epoch; } } }
            for x in g.profiles.iter() { if !np.iter().any(|p| p.id == x.id) && !m.removed.contains(&x.id) { np.push(x.clone()) } }
            g.profiles = np; g.attempts.retain(|a| !m.removed.contains(&a.profile));
        })?;
        for id in &m.del { t.del(id) }
        for id in &m.removed { let base = self.root.join(id); if private_dir(&base).is_ok() { let _ = std::fs::remove_dir_all(&base); } }
        Ok(m.changed)
    }
    /// The profile's kind and the machine it came from ("", "" when there is none).
    pub fn kind_of(&self, id: &str) -> (String, String) { self.inner.lock().unwrap().profiles.iter().find(|p| p.id == id).map(|p| (p.kind.clone(), p.origin.clone())).unwrap_or_default() }
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
        let base = Profile { id: "p".into(), provider: "claude".into(), label: "W".into(), epoch: 2, fingerprint: String::new(), store: String::new(), state: "connected".into(), identity: json!({}), catalog: vec![], allowance: None, checked_at: 0, message: String::new(), expired_at: 0, proven_at: 0, ..Profile::default() };
        let dead = Profile { state: "expired".into(), expired_at: 100, message: EXPIRED_SHORT.into(), ..base.clone() };
        let stale = Profile { checked_at: 120, proven_at: 50, ..base.clone() };
        assert_eq!(merge_checked(&dead, stale.clone()).state, "expired", "its proof is older than the failure");
        assert_eq!(merge_checked(&dead, Profile { proven_at: 130, ..stale.clone() }).state, "connected", "a request answered after it");
        assert_eq!(merge_checked(&base, stale).state, "connected");
    }
    #[tokio::test] async fn an_expired_login_is_kept_across_a_restart_and_refuses_a_launch_in_words_the_failover_reads() {
        let _g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let home = crate::plat::tmp::dir("acc-exp");
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
    json.dump({"leak": bool(os.environ.get("ANTHROPIC_API_KEY")), "dir": d, "token": os.environ.get("CLAUDE_CODE_OAUTH_TOKEN", "")}, open(os.path.join(d, "env.json"), "w")) if d else None
    if os.environ.get("CLAUDE_CODE_OAUTH_TOKEN"):
        print(json.dumps({"loggedIn": True, "authMethod": "oauth_token", "configDirectory": d, "loginExpiresAt": None}))
    elif d and os.path.exists(login()):
        j = json.load(open(login())); j.update({"loggedIn": True, "authMethod": "claude.ai", "configDirectory": d}); print(json.dumps(j))
    else: print(json.dumps({"loggedIn": False}))
elif a[:2] == ["auth", "logout"]:
    if os.path.exists(login()): os.remove(login())
elif a[:1] == ["setup-token"]:
    # Claude Code 2.1.322's own screens, replayed from a recording of the real CLI on a pty (testdata/setup_token_2.1.322.json), and its input as measured
    # there: raw keys, bracketed paste on; text and Enter in one read is taken as pasted text and never submits (Nov 11: a 132-character code written with
    # its CR hung at "Checking the code…"); Enter on its own, or after a bracketed paste, submits. A refused code draws "OAuth error: … Press Enter to retry."
    # and waits there for good: the CLI does not exit. The success screen is drawn the way its Ink renders (words placed by cursor column).
    import termios, tty, select
    rec = json.load(open(os.path.join(os.path.dirname(os.path.abspath(sys.argv[0])), "setup_token.json")))
    tty.setraw(0)
    if d: open(os.path.join(d, "setup-token.pid"), "w").write(str(os.getpid()))
    out = lambda t: (os.write(1, t.encode()), None)[1]
    out(rec["prompt"])
    code, pasting = "", False
    def submit(c):
        if c == "CODE#STATE123":
            tok = os.environ.get("STANDIN_TOKEN", "sk-ant-oat01-" + "A" * 90)
            out("\x1b[2K\x1b[1A\x1b[2K\x1b[1A\x1b[2K\x1b[G\r\r\n\x1b[2G\x1b[38;5;114m\u2713\x1b[4GLong-lived\x1b[15Gauthentication\x1b[30Gtoken\x1b[36Gcreated\x1b[44Gsuccessfully!\x1b[39m\r\r\n\r\r\n"
                "\x1b[2GYour\x1b[7GOAuth\x1b[13Gtoken\x1b[19G(valid\x1b[26Gfor\x1b[30G1\x1b[32Gyear):\r\r\n\r\r\n\x1b[2G\x1b[38;5;220m" + tok + "\x1b[39m\r\r\n\r\r\n"
                "\x1b[2G\x1b[38;5;246mStore\x1b[8Gthis\x1b[13Gtoken\x1b[19Gsecurely.\x1b[39m\r\r\n")
            time.sleep(0.5); sys.exit(0)
        if c == "SLOW#STATE123":
            while os.read(0, 4096): pass  # never answers
        out(rec["rejected"])
        while os.read(0, 4096): pass  # waits for Enter to retry, for good
    while True:
        b = os.read(0, 4096)
        if not b: sys.exit(1)
        while b:
            if pasting:
                i = b.find(b"\x1b[201~")
                if i < 0: code += b.decode(); b = b""; break
                code += b[:i].decode(); b = b[i + 6:]; pasting = False; out(rec["echo"]); continue
            if b.startswith(b"\x1b[200~"): pasting = True; b = b[6:]; continue
            if b in (b"\r", b"\n"): submit(code.strip()); b = b""; break
            if b[:1] in (b"\r", b"\n") and not code: b = b[1:]; continue
            if b[:1] in (b"\r", b"\n"): submit(code.strip()); b = b""; break
            # typed or pasted without the markers: one read is one input event, its CR part of the text
            code += b.replace(b"\r", b"").replace(b"\n", b"").decode(); b = b""; out(rec["echo"])
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
    /// The real CLI's screens for `setup-token` (the stand-in replays them): see the file's `cli`.
    const SETUP_TOKEN_REC: &str = include_str!("testdata/setup_token_2.1.322.json");
    fn dirs(tag: &str) -> (crate::plat::tmp::TempDir, PathBuf) {
        let h = crate::plat::tmp::dir(&format!("acc-{tag}"));
        let sd = h.join("bin"); std::fs::create_dir_all(&sd).unwrap();
        for (n, t) in [("claude", CLAUDE), ("codex", CODEX)] { let p = sd.join(n); std::fs::write(&p, t).unwrap(); std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        std::fs::write(sd.join("setup_token.json"), SETUP_TOKEN_REC).unwrap();
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
        assert_eq!(w.len(), 3); assert_eq!((w[0]["name"].clone(), w[0]["shared"].clone(), w[0]["window_minutes"].clone(), w[0]["resets_at"].clone()), (json!("five_hour"), json!(true), json!(300), json!(1_793_880_000u64)));
        assert_eq!(w[1]["used_percent"], 100.0, "over the limit is out, not unknown"); assert_eq!(w[2]["scope"], "seven_day_opus"); assert_eq!(w[2]["shared"], false); assert!(Accounts::claude_windows(&json!({"rate_limits_available":false})).is_empty());
        let c = Accounts::codex_windows(&json!({"rateLimits":{"limitId":"codex","primary":{"usedPercent":10,"resetsAt":5,"windowDurationMins":300},"secondary":{"usedPercent":-1}}}));
        assert_eq!(c.len(), 1); assert_eq!((c[0]["name"].clone(), c[0]["used_percent"].clone(), c[0]["window_minutes"].clone()), (json!("codex/primary"), json!(10.0), json!(300.0)));
    }
    /// Each login's usage is read with its own token from its own store; the CLI's own login with the one in its store; an expired token is not sent (the CLI is
    /// asked instead); a 401 is a dead login; nothing of one account lands on another.
    #[tokio::test(flavor = "multi_thread")] async fn each_account_reads_its_own_usage_with_its_own_token() {
        use super::super::claude_usage::{tests::{BUSY, RECORDED}, Get};
        let _l = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (h, sd) = dirs("own"); let a = mgr(&h, &sd);
        let (wa, _) = a.plant("claude", "kellyhlord@gmail.com", "k@example.com", Value::Null).await; let (wb, _) = a.plant("claude", "Work", "w@example.com", Value::Null).await;
        let creds = |dir: &Path, tok: &str, exp: u64| std::fs::write(dir.join(".credentials.json"), json!({"claudeAiOauth":{"accessToken":tok,"refreshToken":"r","expiresAt":exp}}).to_string()).unwrap();
        let later = now_ms() + 3_600_000;
        let (da, db) = (a.directory(&a.profile(&wa).unwrap()).unwrap(), a.directory(&a.profile(&wb).unwrap()).unwrap());
        creds(&da, "tok-A", later); creds(&db, "tok-B", later);
        let dd = h.join("cli-login"); std::fs::create_dir_all(&dd).unwrap(); creds(&dd, "tok-D", later);
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let s2 = seen.clone();
        let get: Get = Arc::new(move |t: String| { s2.lock().unwrap().push(t.clone()); Box::pin(async move { match t.as_str() {
            "tok-A" => Ok((200, serde_json::from_str::<Value>(RECORDED).unwrap())), "tok-B" => Ok((200, serde_json::from_str::<Value>(BUSY).unwrap())),
            "tok-D" => Ok((200, json!({"five_hour":{"utilization":5.0},"seven_day":{"utilization":63.0,"resets_at":"2026-11-11T17:00:00Z"}}))), _ => Ok((401, Value::Null)) } }) });
        a.set_usage(get, Some(dd.clone()));
        let ua = a.claude_usage(Some(&a.profile(&wa).unwrap())).await.unwrap();
        assert_eq!((ua.source, ua.windows[0]["scope"].clone(), ua.windows[0]["used_percent"].clone()), ("claude-oauth-usage", json!("seven_day"), json!(64.0)));
        assert_eq!(ua.extra.as_ref().unwrap()["limit"], "$50.00");
        let ub = a.claude_usage(Some(&a.profile(&wb).unwrap())).await.unwrap(); assert_eq!(ub.windows[0]["used_percent"], 81.5);
        let ud = a.claude_usage(None).await.unwrap(); assert_eq!((ud.windows[1]["scope"].clone(), ud.windows[1]["used_percent"].clone()), (json!("seven_day"), json!(63.0)));
        assert_eq!(*seen.lock().unwrap(), ["tok-A", "tok-B", "tok-D"], "each read went with its own login's token");
        // a status check keeps the reading on that account only
        let s = a.request(&json!({"op":"status","profile":wa,"asked":true})).await; let (pa, pb) = (prof(&s, &wa), prof(&s, &wb));
        assert_eq!((pa["allowance"]["state"].clone(), pa["allowance"]["source"].clone(), pa["allowance"]["windows"][0]["used_percent"].clone()), (json!("ok"), json!("claude-oauth-usage"), json!(64.0)), "{pa}");
        assert!(pb["allowance"].is_null(), "the other account is not touched: {pb}");
        // an expired token is not sent: the CLI is started in that store (its startup refreshes it), and with no fresh token its own reading is used
        seen.lock().unwrap().clear(); creds(&db, "tok-B", now_ms().saturating_sub(1000));
        let ub = a.claude_usage(Some(&a.profile(&wb).unwrap())).await.unwrap();
        assert_eq!((ub.source, ub.windows[0]["used_percent"].clone()), ("claude-cli-usage", json!(42.5))); assert!(seen.lock().unwrap().is_empty(), "no request with an expired token");
        // a token the service refuses is a dead login, in the words the failover reads
        creds(&db, "tok-dead", later);
        let e = a.claude_usage(Some(&a.profile(&wb).unwrap())).await.unwrap_err(); assert!(super::super::fallback::auth_text(&e), "{e}"); assert!(!e.contains("tok-dead"));
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_usage_read_that_fails_says_why() {
        let _l = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (h, sd) = dirs("usage"); let a = mgr(&h, &sd);
        let script = CLAUDE.replace("resp = {\"rate_limits_available\": True,", "resp = {\"rate_limits_available\": os.environ.get(\"STANDIN_USAGE\") != \"none\",");
        let path = sd.join("claude"); std::fs::write(&path, script).unwrap();
        std::env::set_var("STANDIN_USAGE", "ok"); let w = a.claude_usage(None).await.unwrap().windows; assert_eq!(w[0]["scope"], "five_hour");
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
        assert_eq!(a.request(&json!({"op":"login","profile":id})).await["error"], SIGNING_IN, "one request at a time, and it says why");
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
        a.child_started(&id); assert_eq!(a.request(&json!({"op":"disconnect","profile":id})).await["error"], TURN_RUNNING, "not while a turn runs, and it says why"); a.child_ended(&id);
        let s = a.request(&json!({"op":"disconnect","profile":id})).await; assert_eq!((prof(&s, &id)["state"].clone(), prof(&s, &id)["epoch"].clone()), (json!("disconnected"), json!(4)));
        assert!(std::fs::read_dir(&store).unwrap().flatten().all(|e| !e.path().is_dir()), "the store is forgotten");
        std::env::remove_var("ANTHROPIC_API_KEY"); let _ = std::fs::remove_dir_all(&h);
    }

    // ---- sync accounts ----
    #[test] fn a_token_its_life_and_the_terminals_colours_are_read_from_what_setup_token_printed() {
        assert_eq!(valid_for("Your OAuth token (valid for 1 year):"), Some(365 * 86400)); assert_eq!(valid_for("valid for 364 days"), Some(364 * 86400)); assert_eq!(valid_for("valid for a year"), Some(365 * 86400));
        assert_eq!(valid_for("nothing said"), None); assert_eq!(valid_for("valid for ever"), None);
        assert_eq!(strip_ansi("\u{1b}[33mtok\u{1b}[0m \u{1b}]8;;https://x\u{7}link\u{1b}]8;;\u{7}"), "tok link");
        let t = format!("sk-ant-oat01-{}", "x_Y-9".repeat(20));
        assert_eq!(catch_token(&format!("Your OAuth token:\n{t}\nStore it")), Some(t.clone())); assert_eq!(catch_token("sk-ant-oat01-short"), None); assert_eq!(catch_token("no token"), None);
    }
    #[test] fn only_a_sync_accounts_child_gets_the_token_and_every_named_child_loses_the_inherited_one() {
        let (h, sd) = dirs("syncenv"); let a = mgr(&h, &sd); let mem = Arc::new(MemTokens::default()); a.set_tokens(mem.clone());
        let cli = Profile { id: new_id(), provider: "claude".into(), label: "W".into(), epoch: 1, state: "connected".into(), ..Profile::default() };
        let sync = Profile { id: new_id(), kind: SYNC.into(), ..cli.clone() };
        assert!(a.environment(Some(&sync)).is_err(), "no token: no child, never the person's own login");
        mem.put(&sync.id, "W", "sk-ant-oat01-T");
        let e = a.environment(Some(&sync)).unwrap(); assert!(e.contains(&("CLAUDE_CODE_OAUTH_TOKEN".into(), "sk-ant-oat01-T".into()))); assert!(e.iter().any(|(k, v)| k == "CLAUDE_CONFIG_DIR" && v.contains(&sync.id)));
        assert!(!a.environment(Some(&cli)).unwrap().iter().any(|(k, _)| k == "CLAUDE_CODE_OAUTH_TOKEN"), "a CLI login's child has none");
        let host = || ["CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY", "PATH"].iter().map(|s| s.to_string());
        let (rm, set) = env_plan(host(), &e, true); assert!(rm.contains(&"CLAUDE_CODE_OAUTH_TOKEN".to_string()) && !rm.contains(&"PATH".to_string())); assert_eq!(set, e, "removed from the host's, then its own set");
        assert!(!env_plan(host(), &[], false).0.contains(&"CLAUDE_CODE_OAUTH_TOKEN".to_string()), "the CLI's own login keeps the host's environment");
        assert!(unset_env("CLAUDE_CODE_OAUTH_TOKEN") && unset_env("CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR"));
        let _l = ENV.lock().unwrap_or_else(|e| e.into_inner()); std::env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-HOST");
        assert!(scrub_for(&a.environment(Some(&cli)).unwrap()).contains(&"CLAUDE_CODE_OAUTH_TOKEN".to_string()), "a turn on a named account loses the hub's");
        assert!(scrub_for(&[]).is_empty(), "a turn on the CLI's own login keeps it");
        std::env::remove_var("CLAUDE_CODE_OAUTH_TOKEN"); let _ = std::fs::remove_dir_all(&h);
    }
    fn sp(id: &str, origin: &str, state: &str) -> Profile { Profile { id: id.into(), provider: "claude".into(), label: "S".into(), epoch: 3, state: state.into(), kind: SYNC.into(), origin: origin.into(), identity: json!({"email":"s@x"}), ..Profile::default() } }
    #[test] fn an_announcement_adds_renews_signs_out_and_removes_only_what_its_machine_made() {
        let (a, b, c, mine, cli) = (new_id(), new_id(), new_id(), new_id(), new_id());
        let cur = vec![sp(&a, "L1", "connected"), sp(&b, "L1", "connected"), sp(&mine, "", "connected"), sp(&c, "L2", "connected"), Profile { id: cli.clone(), provider: "claude".into(), label: "W".into(), epoch: 2, state: "connected".into(), ..Profile::default() }];
        let tok = |x: &str| format!("sk-ant-oat01-{x}{}", "z".repeat(30));
        let have = |id: &str| if id == a { Some(tok("old")) } else if id == b { Some(tok("b")) } else { None };
        let d = new_id();
        let items = vec![json!({"id":a,"label":"Work sync","email":"w@x","plan":"max","token":tok("new"),"madeAt":10,"expiresAt":20}), json!({"id":b,"label":"S","email":"s@x","token":""}), json!({"id":d,"label":"New","email":"n@x","token":tok("d"),"expiresAt":99}),
            json!({"id":mine,"label":"Hijack","token":tok("h")}), json!({"id":cli,"label":"Hijack","token":tok("h")}), json!({"id":"bad","label":"X","token":tok("x")}), json!({"id":new_id(),"label":"X","token":"not a token"})];
        let m = sync_merge(&cur, "L1", &items, &have);
        let get = |id: &str| m.profiles.iter().find(|p| p.id == id).cloned();
        let pa = get(&a).unwrap(); assert_eq!((pa.label.as_str(), pa.identity["plan"].as_str().unwrap(), pa.expires_at, pa.epoch), ("Work sync", "max", 20, 3), "renewed in place, same epoch");
        assert!(m.put.iter().any(|(id, _, t)| *id == a && *t == tok("new")), "the new token is kept");
        assert_eq!(get(&b).unwrap().state, "disconnected"); assert!(m.del.contains(&b), "signed out there: its token goes here");
        let pd = get(&d).unwrap(); assert_eq!((pd.kind.as_str(), pd.origin.as_str(), pd.state.as_str(), pd.provider.as_str(), pd.expires_at), (SYNC, "L1", "unchecked", "claude", 99), "a new one, with its id");
        assert_eq!(get(&mine).unwrap().label, "S", "this machine's own is never another's to change"); assert_eq!(get(&cli).unwrap().label, "W", "nor a CLI login");
        assert!(get(&c).is_some(), "another machine's stays"); assert!(get("bad").is_none());
        assert_eq!(m.changed, 3);
        let m2 = sync_merge(&m.profiles, "L1", &items[..1], &|_| Some(tok("new")));
        assert_eq!((m2.removed.len(), m2.changed), (2, 2), "the ones it no longer names are removed: {:?}", m2.removed); assert!(m2.del.contains(&d) && m2.del.contains(&b));
        assert_eq!(sync_merge(&m2.profiles, "L1", &items[..1], &|_| Some(tok("new"))).changed, 0, "told again unchanged: nothing moves");
        // a check here wrote more into who it is (its sign-in method, its org): the same account, told again, is no change (Nov 11, boat-hub counted each ten-minute repeat)
        let mut m3 = m2.profiles.clone(); for p in m3.iter_mut().filter(|p| p.id == a) { p.identity = json!({"email":"w@x","plan":"max","auth":"oauth_token","org":"o1"}); p.state = "connected".into() }
        assert_eq!(sync_merge(&m3, "L1", &items[..1], &|_| Some(tok("new"))).changed, 0, "who it is is its email and plan");
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_sync_token_is_made_by_setup_token_kept_in_the_vault_used_by_its_turns_and_carried_to_another_machine() {
        let _l = ENV.lock().unwrap_or_else(|e| e.into_inner()); std::env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-should-not-leak-from-the-host");
        let (h, sd) = dirs("sync"); let a = mgr(&h, &sd); let mem = Arc::new(MemTokens::default()); a.set_tokens(mem.clone());
        let work = { let (id, _) = a.plant("claude", "Work", "w@example.com", Value::Null).await; id };
        // made next to a connected account: its name and who it is, and setup-token started at once
        let s = a.request(&json!({"op":"synctoken","profile":work})).await; assert_eq!(s["error"], "", "{s}"); let id = s["made"].as_str().unwrap().to_string();
        assert_eq!((prof(&s, &id)["kind"].as_str().unwrap(), prof(&s, &id)["label"].as_str().unwrap(), prof(&s, &id)["identity"]["email"].as_str().unwrap()), (SYNC, "Work sync", "w@example.com"));
        let s = until(&a, |s| s["attempts"].as_array().map_or(false, |x| x.iter().any(|t| t["profile"] == id.as_str() && t["state"] == "waiting"))).await;
        let att = s["attempts"].as_array().unwrap().iter().find(|t| t["profile"] == id.as_str()).unwrap().clone();
        let url = att["url"].as_str().unwrap();
        assert!(url.starts_with("https://claude.com/cai/oauth/authorize?code=true&client_id=") && url.ends_with("&state=STATE123") && !url.contains(['\u{1b}', '\u{7}']), "the link, out of the terminal's colours and its hyperlink: {url}");
        assert_eq!(a.request(&json!({"op":"respond","profile":id,"attempt":att["id"],"code":"CODE#STATE123"})).await["error"], "");
        let s = until(&a, |s| prof(s, &id)["state"] == "connected").await; let p = prof(&s, &id);
        let tok = format!("sk-ant-oat01-{}", "A".repeat(90));
        assert_eq!(mem.get(&id).as_deref(), Some(tok.as_str()), "the token is in the vault");
        assert!(!s.to_string().contains(&tok) && !a.snapshot().to_string().contains("sk-ant-oat01-A"), "and nowhere a client is told");
        let ends = p["tokenExpiresAt"].as_u64().unwrap(); assert!(ends >= now_s() + TOKEN_LIFE - 60 && ends <= now_s() + TOKEN_LIFE, "a year");
        let env: Value = serde_json::from_str(&std::fs::read_to_string(h.join("providers/accounts-v1").join(&id).join("env.json")).unwrap()).unwrap();
        assert_eq!(env["token"].as_str().unwrap(), tok, "its status check ran on its own token, not the host's");
        let epoch = p["epoch"].as_u64().unwrap();
        let plan = a.launch_plan(&id, &epoch.to_string(), "claude").await.unwrap();
        assert!(plan.env.contains(&("CLAUDE_CODE_OAUTH_TOKEN".into(), tok.clone()))); assert_eq!(plan.args, ["--setting-sources", "user"], "no claude.ai-only setting over a token");
        let wplan = a.launch_plan(&work, "2", "claude").await.unwrap(); assert!(!wplan.env.iter().any(|(k, _)| k == "CLAUDE_CODE_OAUTH_TOKEN"), "a CLI login's turn never gets one");
        // a restart keeps the account whole, its token in the vault only
        let reg = std::fs::read_to_string(h.join("providers/accounts-v1/profiles.json")).unwrap(); assert!(!reg.contains("sk-ant-oat01"), "no token in the registry");
        let again = mgr(&h, &sd); again.set_tokens(mem.clone()); let sn = again.snapshot(); let q = prof(&sn, &id);
        assert_eq!((q["kind"].as_str().unwrap(), q["identity"]["email"].as_str().unwrap(), q["tokenExpiresAt"].as_u64().unwrap()), (SYNC, "w@example.com", ends));
        // the other machine: told it, it has it with the same id, its token in its own vault; a turn there gets the token
        let (h2, _) = dirs("sync-b"); let b = mgr(&h2, &sd); let mem2 = Arc::new(MemTokens::default()); b.set_tokens(mem2.clone());
        let out = a.sync_out().unwrap(); assert_eq!(out.len(), 1, "only the sync account goes: {out:?}"); assert_eq!(out[0]["id"], id.as_str());
        assert_eq!(b.sync_in("LA", &out), Ok(1)); assert_eq!(mem2.get(&id).as_deref(), Some(tok.as_str()));
        let bp = prof(&b.snapshot(), &id).clone(); assert_eq!((bp["origin"].as_str().unwrap(), bp["state"].as_str().unwrap()), ("LA", "unchecked"));
        let bplan = b.launch_plan(&id, &bp["epoch"].as_u64().unwrap().to_string(), "claude").await.unwrap(); assert!(bplan.env.contains(&("CLAUDE_CODE_OAUTH_TOKEN".into(), tok.clone())));
        assert!(b.sync_out().unwrap().is_empty(), "what it was given it does not announce");
        // renewal: a new token, the same account and epoch everywhere
        std::env::set_var("STANDIN_TOKEN", format!("sk-ant-oat01-{}", "B".repeat(90)));
        let att = { a.request(&json!({"op":"login","profile":id})).await; let s = until(&a, |s| s["attempts"].as_array().unwrap().iter().any(|t| t["profile"] == id.as_str() && t["state"] == "waiting")).await; s["attempts"].as_array().unwrap().iter().rev().find(|t| t["profile"] == id.as_str()).unwrap()["id"].clone() };
        a.request(&json!({"op":"respond","profile":id,"attempt":att,"code":"CODE#STATE123"})).await;
        let s = until(&a, |s| s["attempts"].as_array().unwrap().iter().any(|t| t["id"] == att && t["state"] == "done")).await; std::env::remove_var("STANDIN_TOKEN");
        assert_eq!(prof(&s, &id)["epoch"].as_u64().unwrap(), epoch, "a renewal is the same account"); assert!(mem.get(&id).unwrap().contains("BBBB"));
        assert_eq!(b.sync_in("LA", &a.sync_out().unwrap()), Ok(1)); assert!(mem2.get(&id).unwrap().contains("BBBB"), "the renewal reaches the other machine");
        // removed where it was made: gone there and, told, here
        assert_eq!(a.request(&json!({"op":"remove","profile":id})).await["error"], ""); assert!(mem.get(&id).is_none(), "its token leaves the vault");
        assert_eq!(b.sync_in("LA", &a.sync_out().unwrap()), Ok(1)); assert!(b.snapshot()["profiles"].as_array().unwrap().iter().all(|p| p["id"] != id.as_str())); assert!(mem2.get(&id).is_none());
        std::env::remove_var("CLAUDE_CODE_OAUTH_TOKEN"); let _ = std::fs::remove_dir_all(&h); let _ = std::fs::remove_dir_all(&h2);
    }
    #[test] fn the_real_setup_token_screens_read_as_words_its_error_line_and_never_a_token() {
        let rec: Value = serde_json::from_str(SETUP_TOKEN_REC).unwrap();
        let prompt = strip_ansi(rec["prompt"].as_str().unwrap());
        assert!(prompt.contains("Paste code here if prompted >") && prompt.contains("long-lived (1-year) auth token setup"), "Ink's column moves are gaps between words: {prompt}");
        assert!(refusal(&prompt).is_none(), "the prompt is no refusal");
        let mut why = String::new();
        let u = scan_claude(&prompt, &mut why).unwrap(); assert!(u.starts_with("https://claude.com/cai/oauth/authorize?") && u.ends_with("state=STATE123"), "{u}"); assert_eq!(why, "");
        let after = strip_ansi(&format!("{}{}", rec["echo"].as_str().unwrap(), rec["rejected"].as_str().unwrap()));
        assert_eq!(refusal(&after).as_deref(), Some("OAuth error: That code wasn't accepted. It may be incomplete, expired, already used, or from an earlier sign-in attempt. Sign in again to get a new code."));
        let _ = scan_claude(&after, &mut why); assert_eq!(why, "code_rejected");
        // what is said is never a token, nor the masked code
        let t = format!("sk-ant-oat01-{}", "Z".repeat(90));
        assert_eq!(last_said(&format!("Error: bad\r\n {t} \r\n******uAQI84\r\n")), "Error: bad");
        assert_eq!(said_line(&"word ".repeat(100)).unwrap().chars().count(), 241);
        assert_eq!(strip_ansi("Your\u{1b}[7GOAuth\u{1b}[13Gtoken\u{1b}[19G(valid\u{1b}[26Gfor\u{1b}[30G1\u{1b}[32Gyear):"), "Your OAuth token (valid for 1 year):");
    }
    async fn sync_waiting(a: &Arc<Accounts>, id: &str) -> String {
        let s = until(a, |s| s["attempts"].as_array().unwrap().iter().any(|t| t["profile"] == id && t["state"] == "waiting")).await;
        s["attempts"].as_array().unwrap().iter().rev().find(|t| t["profile"] == id && t["state"] == "waiting").unwrap()["id"].as_str().unwrap().to_string()
    }
    /// Nov 11: a sync token's code was pasted, Submit pressed, and the page said "Checking the code and your sign-in…" for good: the code went in with its Enter
    /// in one write (pasted text to Claude Code, never submitted), and a refused code's error screen waits for Enter, so nothing ever ended the attempt.
    #[tokio::test(flavor = "multi_thread")] async fn a_sync_codes_answer_always_ends_the_attempt_refused_unanswered_or_cancelled_and_it_can_be_made_again() {
        let (h, sd) = dirs("syncfail"); let a = mgr(&h, &sd); let mem = Arc::new(MemTokens::default()); a.set_tokens(mem.clone());
        let s = a.request(&json!({"op":"add","provider":"claude","kind":"sync","label":"S","start":true})).await; let id = s["made"].as_str().unwrap().to_string();
        // the stand-in's pid, written in its store before it draws its link
        fn pid_in(d: &Path) -> Option<u32> { for e in std::fs::read_dir(d).ok()?.flatten() { let p = e.path(); if p.is_dir() { if let Some(x) = pid_in(&p) { return Some(x) } } else if p.ends_with("setup-token.pid") { if let Ok(t) = std::fs::read_to_string(&p) { if let Ok(n) = t.trim().parse() { return Some(n) } } } } None }
        let root = h.join("providers/accounts-v1").join(&id);
        let pid_now = || pid_in(&root);
        // a refused code (as long as a real one): its error line said, the CLI ended
        let aid = sync_waiting(&a, &id).await; let p = pid_now(); assert!(p.is_some(), "its CLI runs");
        let long = format!("{}#STATE123", "Xy9_".repeat(30));
        assert_eq!(a.request(&json!({"op":"respond","profile":id,"attempt":aid,"code":long})).await["error"], "");
        let s = until(&a, |s| s["attempts"].as_array().unwrap().iter().any(|t| t["id"] == aid.as_str() && t["state"] == "failed")).await;
        let m = s["attempts"].as_array().unwrap().iter().find(|t| t["id"] == aid.as_str()).unwrap()["message"].as_str().unwrap().to_string();
        assert!(m.starts_with("Claude did not accept that code.") && m.contains("Make the sync token") && m.contains("Claude Code said: \u{201c}OAuth error: That code wasn't accepted."), "{m}");
        let p = p.unwrap(); soon(|| !crate::agents::proc::alive(p)).await; assert!(!crate::agents::proc::alive(p), "the CLI on its error screen is ended");
        // one that is never answered: ended when its wait is over, with the CLI's last line
        a.request(&json!({"op":"login","profile":id})).await; let aid = sync_waiting(&a, &id).await;
        a.request(&json!({"op":"respond","profile":id,"attempt":aid,"code":"SLOW#STATE123"})).await;
        let s = until(&a, |s| s["attempts"].as_array().unwrap().iter().any(|t| t["id"] == aid.as_str() && t["state"] == "failed")).await;
        let m = s["attempts"].as_array().unwrap().iter().find(|t| t["id"] == aid.as_str()).unwrap()["message"].as_str().unwrap().to_string();
        assert!(m.starts_with("Claude Code did not finish the sign-in after the code was sent."), "{m}");
        // cancelled: its CLI goes
        a.request(&json!({"op":"login","profile":id})).await; let aid = sync_waiting(&a, &id).await;
        let p = pid_now().unwrap(); assert!(crate::agents::proc::alive(p));
        a.request(&json!({"op":"cancel","profile":id,"attempt":aid})).await;
        soon(|| !crate::agents::proc::alive(p)).await; assert!(!crate::agents::proc::alive(p), "Cancel sign-in ends the CLI");
        // and made again, with the right code: done, its token kept
        a.request(&json!({"op":"login","profile":id})).await; let aid = sync_waiting(&a, &id).await;
        a.request(&json!({"op":"respond","profile":id,"attempt":aid,"code":"CODE#STATE123"})).await;
        until(&a, |s| prof(s, &id)["state"] == "connected").await; assert!(mem.get(&id).unwrap().starts_with("sk-ant-oat01-"));
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_locked_vault_announces_nothing_so_no_machine_removes_what_it_holds() {
        struct Locked; impl Tokens for Locked { fn get(&self, _: &str) -> Option<String> { None } fn put(&self, _: &str, _: &str, _: &str) -> bool { false } fn del(&self, _: &str) {} fn open(&self) -> bool { false } }
        let (h, sd) = dirs("synclock"); let a = mgr(&h, &sd); a.set_tokens(Arc::new(Locked));
        assert!(a.sync_out().is_none(), "a locked vault says nothing, not even that there is nothing");
        let s = a.request(&json!({"op":"add","provider":"claude","kind":"sync","label":"S"})).await; let id = s["made"].as_str().unwrap().to_string();
        a.update(&id, |p| p.state = "connected".into()).unwrap();
        assert!(a.sync_out().is_none()); assert!(a.sync_in("L", &[]).is_err());
        let _ = std::fs::remove_dir_all(&h);
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
        for op in ["status", "login", "disconnect", "remove"] { assert_eq!(a.request(&json!({"op":op,"profile":id})).await["error"], STARTING, "{op} is refused while launches are queued, and says why"); }
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
        assert_eq!(a.request(&json!({"op":"login","profile":id})).await["error"], TURN_RUNNING, "a sign-in is, and it says why");
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
        // the attempt shows failed at once; its candidate store goes when its child is gone (`discard`), a moment later
        let gone = || std::fs::read_dir(h.join("providers/accounts-v1").join(&id)).unwrap().flatten().all(|e| !e.path().is_dir());
        soon(|| gone()).await; assert!(gone(), "the candidate store is discarded");
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
    /// #1108: a CLI that answered and closed its output but never exits (or whose exit the hub misses) held `cli`, and so the account's job, for ever.
    #[tokio::test(flavor = "multi_thread")] async fn a_cli_that_answers_but_never_exits_is_killed_at_the_limit() {
        let (h, _sd) = dirs("hang"); let (prog, pidf) = (h.join("hang.sh"), h.join("hang.pid"));
        std::fs::write(&prog, format!("#!/bin/sh\necho $$ > '{}'\necho '{{\"loggedIn\":true}}'\nexec >&-\nexec sleep 30\n", pidf.display())).unwrap();
        std::fs::set_permissions(&prog, std::fs::Permissions::from_mode(0o755)).unwrap();
        let p2 = prog.clone(); let a = Accounts::open(&h, Arc::new(move |_: &str| Ok(p2.clone())));
        let t0 = std::time::Instant::now();
        assert!(a.cli_within(Duration::from_millis(800), None, "claude", &["auth", "status"]).await.is_err(), "no exit within the limit is an error");
        assert!(t0.elapsed() < Duration::from_secs(5), "ended at the limit, not when the child chose to: {:?}", t0.elapsed());
        let pid: u32 = std::fs::read_to_string(&pidf).unwrap().trim().parse().unwrap();
        let gone = std::time::Instant::now() + Duration::from_secs(10);
        while crate::plat::proc::alive(pid) && std::time::Instant::now() < gone { tokio::time::sleep(Duration::from_millis(50)).await }
        assert!(!crate::plat::proc::alive(pid), "the child was killed (and reaped)");
        // one that exits in time still answers
        std::fs::write(&prog, "#!/bin/sh\necho '{\"loggedIn\":true}'\n").unwrap();
        let (code, data) = a.cli_within(Duration::from_secs(10), None, "claude", &["auth", "status"]).await.unwrap(); assert_eq!((code, data["loggedIn"].clone()), (0, json!(true)));
        let _ = std::fs::remove_dir_all(&h);
    }
    /// #1108: the account's in-progress mark goes with the op, however it ends (an error, a timeout, a dropped future), and a refusal says why.
    #[tokio::test(flavor = "multi_thread")] async fn an_ops_mark_is_released_when_it_is_dropped_and_a_refusal_says_why() {
        let (h, sd) = dirs("job"); let a = mgr(&h, &sd); let id = add(&a, "claude", "Work").await; connect(&a, &id).await;
        { let _j = a.hold(&id, "status", false).unwrap(); assert_eq!(a.begin(&id, "refresh", false), Err(CHECKING), "held"); }
        assert!(a.hold(&id, "refresh", false).is_ok(), "released when the guard drops");
        let a1 = a.clone(); let i1 = id.clone();
        let stuck = async move { let _j = a1.hold(&i1, "status", false).unwrap(); std::future::pending::<()>().await };
        assert!(tokio::time::timeout(Duration::from_millis(100), stuck).await.is_err());
        assert!(a.hold(&id, "status", false).is_ok(), "an op whose future was dropped (a timeout) leaves no mark");
        let _j = a.hold(&id, "refresh", false).unwrap(); // the 5-minute recheck is running
        for op in ["status", "disconnect", "remove"] { assert_eq!(a.request(&json!({"op":op,"profile":id})).await["error"], CHECKING, "{op}: refused, saying a verification runs"); }
        assert_eq!(a.request(&json!({"op":"login","profile":id})).await["error"], CHECKING);
        drop(_j); assert_eq!(a.request(&json!({"op":"status","profile":id})).await["error"], "", "free again");
        a.child_started(&id); assert_eq!(a.begin(&id, "refresh", true), Err(TURN_RUNNING)); a.child_ended(&id);
        let _ = std::fs::remove_dir_all(&h);
    }
}

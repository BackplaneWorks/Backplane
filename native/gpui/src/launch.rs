//! First launch: with no `--url`, the client finds its own hub. `launch.json` beside the client's other per-device files says which home (or which hub on another
//! machine) this device uses. A home's hub is started here, beside this program, and keeps running when the window closes; a second launch finds it through
//! `<home>/hub.lock`. Nothing here decides anything a person sees beyond the home's default and whether a folder can be one; the words are `screens/first.rs`'s.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, time::{Duration, Instant}};

/// The first port a home's hub tries. 3787 is 0.14's and the client refuses it.
pub const PORT_FIRST: u16 = 3800;
pub const PORT_LAST: u16 = 3899;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    /// The home this device runs its hub in.
    pub home: String,
    /// A hub on another machine: its address and the file with this device's token.
    pub url: String, pub token_file: String,
}

fn config_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BACKPLANE_GPUI_CONFIG").filter(|p| !p.is_empty()) { return Some(p.into()) }
    let base = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()).map(PathBuf::from)
        .or_else(|| user_home().map(|h| h.join(".config")))?;
    Some(base.join("backplane-gpui"))
}
pub fn user_home() -> Option<PathBuf> { std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from) }
pub fn saved_path() -> Option<PathBuf> { config_dir().map(|d| d.join("launch.json")) }
pub fn load() -> Saved { saved_path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default() }
pub fn save(s: &Saved) -> Result<()> {
    let p = saved_path().context("no folder for this device's settings")?;
    if let Some(d) = p.parent() { std::fs::create_dir_all(d)?; }
    std::fs::write(p, serde_json::to_vec_pretty(s)?)?;
    Ok(())
}
/// Back to the chooser on the next launch; the homes themselves are not touched.
pub fn forget() { if let Some(p) = saved_path() { let _ = std::fs::remove_file(p); } }

/// Where a home goes when the person takes the default, per system. Each system with a build has a line here.
pub fn default_home(os: &str, user: &Path) -> PathBuf {
    match os { "macos" => user.join("Library/Application Support/Backplane/hub"), _ => user.join(".backplane/hub") }
}
pub fn default_here() -> Option<PathBuf> { user_home().map(|u| default_home(std::env::consts::OS, &u)) }

/// 0.14's home, when this machine has one: it stays as it is, and it cannot be a home here.
pub fn old_home(user: &Path) -> Option<PathBuf> { let p = user.join(".backplane-bend"); p.join("events.jsonl").is_file().then_some(p) }

/// Why a folder cannot be a home ("" when it can). Pure on what it is told.
pub fn home_problem(path: &str, old: Option<&Path>) -> &'static str {
    let t = path.trim();
    if t.is_empty() { return "Choose a folder." }
    if !Path::new(t).is_absolute() { return "Give the full path, starting at the top of the disk." }
    if old.map_or(false, |o| Path::new(t).starts_with(o)) { return "That folder belongs to Backplane 0.14. Choose a new one, and 0.14 keeps its data." }
    ""
}
/// Whether a folder already holds a hub's data.
pub fn has_data(home: &Path) -> bool { home.join("events.jsonl").is_file() }

/// `<home>/hub.lock`: "<pid> <port> flock". The port of the hub the lock names.
pub fn lock_port(text: &str) -> Option<u16> { text.split_whitespace().nth(1)?.parse().ok() }
fn answering(port: u16) -> bool {
    ureq::get(&format!("http://127.0.0.1:{port}/hello")).timeout(Duration::from_secs(2)).call().is_ok()
}
fn free_port(first: u16, last: u16) -> Option<u16> { (first..=last).find(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok()) }

/// The hub program beside this one, or the one `BACKPLANE_HUB_BIN` names.
pub fn hub_bin() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("BACKPLANE_HUB_BIN").filter(|p| !p.is_empty()) { return Ok(p.into()) }
    let me = std::env::current_exe()?;
    let p = me.parent().context("no folder for this program")?.join("backplane-hub");
    if p.is_file() { Ok(p) } else { bail!("The hub program is missing from {}. Install Backplane again.", me.parent().unwrap().display()) }
}

fn detached(cmd: &mut std::process::Command) {
    #[cfg(unix)] { use std::os::unix::process::CommandExt; cmd.process_group(0); }
}
/// The end of the hub's log, for the words when it did not start.
fn log_tail(log: &Path) -> String { let t = std::fs::read_to_string(log).unwrap_or_default(); t.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(200).collect() }

/// The hub for this home: the one running there, or a new one. Answers its address and token file once it answers. Blocks, so it runs off the window's thread.
pub fn ensure_hub(home: &Path) -> Result<(String, PathBuf)> {
    std::fs::create_dir_all(home).with_context(|| format!("Backplane could not make {}", home.display()))?;
    let (lock, token) = (home.join("hub.lock"), home.join("token"));
    if let Some(port) = std::fs::read_to_string(&lock).ok().as_deref().and_then(lock_port) { if answering(port) && token.is_file() { return Ok((format!("http://127.0.0.1:{port}"), token)) } }
    let bin = hub_bin()?;
    let want = std::env::var("BACKPLANE_PORT").ok().and_then(|p| p.parse::<u16>().ok());
    let port = want.or_else(|| free_port(PORT_FIRST, PORT_LAST)).context("No free port between 3800 and 3899.")?;
    let log_path = home.join("hub.log");
    let log = std::fs::OpenOptions::new().create(true).append(true).open(&log_path)?;
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("--home").arg(home).arg("--port").arg(port.to_string()).arg("--host").arg("127.0.0.1").arg("--foreground").stdin(std::process::Stdio::null()).stdout(log.try_clone()?).stderr(log);
    if std::env::var_os("BACKPLANE_NO_TAILSCALE").is_some() { cmd.arg("--no-tailscale"); }
    detached(&mut cmd);
    let mut child = cmd.spawn().with_context(|| format!("Backplane could not start {}", bin.display()))?;
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(60) {
        if answering(port) && token.is_file() { return Ok((format!("http://127.0.0.1:{port}"), token)) }
        if let Ok(Some(code)) = child.try_wait() { bail!("The hub stopped with code {}. {}", code.code().unwrap_or(-1), log_tail(&log_path)) }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("The hub did not answer within a minute. Its log is {}.", log_path.display())
}

/// What a person pasted: an address alone, or a pairing link with its secret after `#invite=`.
pub fn parse_hub(text: &str) -> Option<(String, Option<String>)> {
    let t = text.trim(); if t.is_empty() { return None }
    let (addr, secret) = match t.split_once("#invite=") { Some((a, s)) => (a.trim_end_matches("/pair").trim_end_matches('/'), Some(s.to_string())), None => (t.trim_end_matches('/'), None) };
    let addr = if addr.contains("://") { addr.to_string() } else { format!("http://{addr}") };
    addr.starts_with("http://").then_some((addr, secret))
}
/// The words for an `/auth/enroll` refusal.
pub fn enroll_error(code: &str) -> &'static str {
    match code {
        "used_code" => "That code was used already. Make a new one on the other machine.",
        "expired_code" => "That code expired after ten minutes. Make a new one on the other machine.",
        "revoked_code" => "That code was revoked. Make a new one on the other machine.",
        "rate_limited" => "Too many tries. Wait a minute and try again.",
        "device_limit" => "That hub has too many devices. Revoke one in Settings, Devices.",
        _ => "That code is wrong. Check it and try again.",
    }
}
/// Pair this device with a hub elsewhere: the code buys a token, kept in a private file. Blocks.
pub fn pair(addr: &str, code: &str) -> Result<(String, PathBuf)> {
    let name = std::env::var("HOSTNAME").ok().filter(|n| !n.is_empty()).unwrap_or_else(|| "Backplane desktop".into());
    let r = ureq::post(&format!("{addr}/auth/enroll")).timeout(Duration::from_secs(10)).set("Authorization", &format!("Bearer {}", code.trim())).set("X-Backplane-Client", "native").set("X-Backplane-Name", &name).call();
    let v: serde_json::Value = match r {
        Ok(a) => serde_json::from_str(&a.into_string()?).context("That address did not answer like a hub.")?,
        Err(ureq::Error::Status(_, a)) => a.into_string().ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null),
        Err(_) => bail!("Backplane could not reach {addr}. Check the address and that both machines share a network."),
    };
    let Some(token) = v["token"].as_str() else { bail!("{}", enroll_error(v["error"].as_str().unwrap_or(""))) };
    let dir = config_dir().context("no folder for this device's settings")?; std::fs::create_dir_all(&dir)?;
    let file = dir.join("hub-token");
    let mut o = std::fs::OpenOptions::new(); o.create(true).write(true).truncate(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    std::io::Write::write_all(&mut o.open(&file)?, token.as_bytes())?;
    Ok((addr.to_string(), file))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn each_system_has_its_own_default_home() {
        assert_eq!(default_home("linux", Path::new("/home/a")), Path::new("/home/a/.backplane/hub"));
        assert_eq!(default_home("macos", Path::new("/Users/a")), Path::new("/Users/a/Library/Application Support/Backplane/hub"));
    }
    #[test] fn a_home_must_be_a_full_path_and_never_the_old_one() {
        let old = Path::new("/h/.backplane-bend");
        assert_eq!(home_problem("/h/.backplane/hub", Some(old)), "");
        assert!(home_problem("", None).starts_with("Choose"));
        assert!(home_problem("hub", None).contains("full path"));
        assert!(home_problem("/h/.backplane-bend/x", Some(old)).contains("0.14"));
    }
    #[test] fn the_lock_says_the_port() {
        assert_eq!(lock_port("1234 3801 flock\n"), Some(3801));
        assert_eq!(lock_port(""), None); assert_eq!(lock_port("1234 x flock"), None);
    }
    #[test] fn a_pasted_address_or_link_is_read() {
        assert_eq!(parse_hub("desktop.ts.net:3800/"), Some(("http://desktop.ts.net:3800".into(), None)));
        assert_eq!(parse_hub("http://h:3800/pair#invite=abc"), Some(("http://h:3800".into(), Some("abc".into()))));
        assert_eq!(parse_hub("  "), None); assert_eq!(parse_hub("ftp://x"), None);
    }
    #[test] fn a_free_port_skips_a_taken_one() {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap(); let p = l.local_addr().unwrap().port();
        assert_ne!(free_port(p, p.saturating_add(20)), Some(p));
    }
    #[test] fn each_refusal_has_its_own_words() {
        assert!(enroll_error("used_code").contains("used")); assert!(enroll_error("expired_code").contains("expired")); assert!(enroll_error("nonsense").contains("wrong"));
    }
    #[test] fn a_saved_choice_reads_back_and_a_damaged_file_is_none() {
        let s = Saved { home: "/h".into(), url: String::new(), token_file: String::new() };
        assert_eq!(serde_json::from_slice::<Saved>(&serde_json::to_vec(&s).unwrap()).unwrap(), s);
    }
}

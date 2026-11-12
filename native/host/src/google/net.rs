//! Google's endpoints through `curl` (proc Kind::Net, so a copy of a real log reaches none of them): the URL, the headers (the bearer token) and the body
//! (the client secret, a code, a refresh token, a message) go in 0600 files in `<home>/secrets/google` (0700), never on a command line, and are removed after.
//! The bases move for tests: BACKPLANE_GOOGLE_OAUTH (token, revoke), BACKPLANE_GOOGLE_API (Gmail, Calendar), BACKPLANE_GOOGLE_AUTH (the consent page).
use std::{io::Write, path::{Path, PathBuf}};
use crate::plat::fs::{DirBuilderExt, OpenOptionsExt};

#[derive(Debug, Clone)]
pub struct Ends { pub oauth: String, pub api: String, pub auth: String }
impl Ends {
    pub fn from_env() -> Ends {
        let e = |k: &str, d: &str| std::env::var(k).ok().map(|s| s.trim().trim_end_matches('/').to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| d.to_string());
        Ends { oauth: e("BACKPLANE_GOOGLE_OAUTH", "https://oauth2.googleapis.com"), api: e("BACKPLANE_GOOGLE_API", "https://www.googleapis.com"), auth: e("BACKPLANE_GOOGLE_AUTH", super::oauth::AUTH_BASE) }
    }
    pub fn token(&self) -> String { format!("{}/token", self.oauth) }
    pub fn revoke(&self) -> String { format!("{}/revoke", self.oauth) }
    pub fn api(&self, path: &str) -> String { format!("{}{path}", self.api) }
}

/// What came back: the HTTP status ("000" when nothing answered), the body, and curl's own words when it failed.
#[derive(Debug, Clone, Default)]
pub struct Resp { pub code: String, pub body: Vec<u8>, pub err: String }
impl Resp {
    pub fn ok(&self) -> bool { self.code.starts_with('2') }
    pub fn text(&self) -> String { String::from_utf8_lossy(&self.body).into_owned() }
    pub fn json(&self) -> serde_json::Value { serde_json::from_slice(&self.body).unwrap_or(serde_json::Value::Null) }
    /// Gc.fail: an unsuccessful answer as text for the agent
    pub fn fail(&self) -> String {
        if self.code == "000" { return format!("could not reach Google: {}", if self.err.is_empty() { "no answer" } else { &self.err }) }
        let why = if self.code == "401" { " (Google access expired or was revoked: the person signs in again in Settings, Accounts)" } else { "" };
        format!("HTTP {}: {}{why}", self.code, super::oauth::api_error(&self.text()))
    }
}

/// One request, quoted for a curl config: \ and " escaped, controls dropped
fn q(s: &str) -> String { let mut o = String::from("\""); for c in s.chars() { if (c as u32) < 32 { continue } if c == '"' || c == '\\' { o.push('\\') } o.push(c) } o.push('"'); o }

pub struct Req<'a> { pub method: &'a str, pub url: String, pub headers: Vec<String>, pub body: Option<Vec<u8>>, pub secs: u64 }
impl<'a> Req<'a> {
    pub fn get(url: String, token: &str) -> Req<'a> { Req { method: "GET", url, headers: vec![format!("Authorization: Bearer {token}")], body: None, secs: 30 } }
    pub fn form(url: String, body: String) -> Req<'a> { Req { method: "POST", url, headers: vec!["Content-Type: application/x-www-form-urlencoded".into()], body: Some(body.into_bytes()), secs: 30 } }
    pub fn json(method: &'a str, url: String, token: &str, body: &serde_json::Value) -> Req<'a> {
        Req { method, url, headers: vec![format!("Authorization: Bearer {token}"), "Content-Type: application/json; charset=UTF-8".into()], body: Some(body.to_string().into_bytes()), secs: 30 }
    }
}

fn dir(home: &Path, sub: &str) -> std::io::Result<PathBuf> { let d = home.join("secrets").join(sub); std::fs::DirBuilder::new().mode(0o700).recursive(true).create(&d)?; Ok(d) }
fn w(p: &Path, b: &[u8]) -> std::io::Result<()> { std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(p).and_then(|mut f| f.write_all(b)) }

/// The request, off the async threads.
pub async fn send(home: &Path, r: Req<'_>) -> Resp { send_in(home, "google", r).await }
/// The same through files in `<home>/secrets/<sub>` (0700): another service's key and body (voice/eot.rs: TypeSafe's).
pub async fn send_in(home: &Path, sub: &str, r: Req<'_>) -> Resp {
    let (home, sub, method, url, headers, body, secs) = (home.to_path_buf(), sub.to_string(), r.method.to_string(), r.url, r.headers, r.body, r.secs);
    tokio::task::spawn_blocking(move || send_blocking(&home, &sub, &method, &url, &headers, body.as_deref(), secs)).await.unwrap_or_default()
}
fn send_blocking(home: &Path, sub: &str, method: &str, url: &str, headers: &[String], body: Option<&[u8]>, secs: u64) -> Resp {
    let Ok(d) = dir(home, sub) else { return Resp { code: "000".into(), body: vec![], err: "could not make the secrets folder".into() } };
    let n = super::oauth::nonce(9).unwrap_or_else(|_| format!("{:x}", crate::hub::now_ms())).replace(['-', '_'], "x");
    let (cfg, inp, out) = (d.join(format!("g-{n}.curl")), d.join(format!("g-{n}.in")), d.join(format!("g-{n}.out")));
    let mut c = format!("url = {}\nrequest = {}\n", q(url), q(method));
    for h in headers { c.push_str(&format!("header = {}\n", q(h))) }
    if body.is_some() { c.push_str(&format!("data-binary = {}\n", q(&format!("@{}", inp.to_string_lossy())))) }
    c.push_str(&format!("max-time = {secs}\noutput = {}\nwrite-out = \"%{{http_code}}\"\n", q(&out.to_string_lossy())));
    let r = (|| -> Result<Resp, String> {
        if let Some(b) = body { w(&inp, b).map_err(|e| e.to_string())? }
        w(&cfg, c.as_bytes()).map_err(|e| e.to_string())?;
        let o = crate::proc::std_cmd(crate::proc::Kind::Net, "curl").args(["-sS", "--max-redirs", "0", "-K"]).arg(&cfg).current_dir(&d).output().map_err(|e| e.to_string())?;
        let said = String::from_utf8_lossy(&o.stdout).trim().to_string();
        let code: String = said.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
        Ok(Resp { code: if code.len() == 3 && code.bytes().all(|b| b.is_ascii_digit()) { code } else { "000".into() }, body: std::fs::read(&out).unwrap_or_default(), err: String::from_utf8_lossy(&o.stderr).trim().chars().take(300).collect() })
    })();
    for p in [&cfg, &inp, &out] { let _ = std::fs::remove_file(p); }
    r.unwrap_or_else(|e| Resp { code: "000".into(), body: vec![], err: e })
}

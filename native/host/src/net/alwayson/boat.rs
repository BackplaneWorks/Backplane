//! Boat's HTTP API (https://docs.boat.dev/api/v1), through `curl` (proc Kind::Net, so a copy of a real log reaches none of it). The key is read from
//! the vault (`boat.api_key`) for each call and goes into a 0600 curl config in `<home>/secrets/boat` (0700) with the URL and the body; that file is
//! removed after the call. Nothing here logs a request's headers, the key or a signed upload URL: an error names the call and Boat's own message.
//!
//! The bases move for tests: `BACKPLANE_BOAT_API` (default `https://boat.dev/api/v1`) and `BACKPLANE_BOAT_WEB` (`https://boat.dev`, where the CLI's
//! browser sign-in lives: `POST /api/boat/cli/login/start {provider}` gives `{login_url, poll_token, user_code, expires_at}`, and
//! `GET /api/boat/cli/login/poll/<poll_token>` answers `{status, token}`). That sign-in is not in Boat's documented API: it is the CLI's own, so it
//! is best-effort and the API key box is always offered beside it.
use serde_json::{json, Value};
use std::{io::Write, path::{Path, PathBuf}};
use crate::plat::fs::{DirBuilderExt, OpenOptionsExt};

/// The vault name of the person's Boat key (D1: their own account).
pub const KEY: &str = "boat.api_key";

pub fn api_base() -> String { base("BACKPLANE_BOAT_API", "https://boat.dev/api/v1") }
pub fn web_base() -> String { base("BACKPLANE_BOAT_WEB", "https://boat.dev") }
fn base(k: &str, d: &str) -> String { std::env::var(k).ok().map(|s| s.trim().trim_end_matches('/').to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| d.to_string()) }

/// What came back: the HTTP status ("000" when nothing answered), the body, and curl's own words when it failed.
#[derive(Debug, Clone, Default)]
pub struct Resp { pub code: String, pub body: Vec<u8>, pub err: String }
impl Resp {
    pub fn ok(&self) -> bool { self.code.starts_with('2') }
    pub fn json(&self) -> Value { serde_json::from_slice(&self.body).unwrap_or(Value::Null) }
    /// Boat's own message for a failed call (`message`, `error.message`, `code`), or what curl said.
    pub fn why(&self) -> String {
        if self.code == "000" { return format!("Boat did not answer: {}", if self.err.is_empty() { "no connection" } else { &self.err }) }
        let j = self.json();
        let m = j["message"].as_str().or_else(|| j["error"]["message"].as_str()).or_else(|| j["error"].as_str()).or_else(|| j["code"].as_str()).unwrap_or("");
        let m: String = m.chars().take(240).collect();
        match self.code.as_str() {
            "401" => "Boat refused the key: it was revoked or mistyped. Connect the account again.".into(),
            "402" => format!("Boat asks for billing first{}", if m.is_empty() { String::new() } else { format!(": {m}") }),
            c => if m.is_empty() { format!("Boat answered {c}") } else { format!("Boat answered {c}: {m}") },
        }
    }
}

/// curl config quoting: \ and " escaped, control characters dropped.
fn q(s: &str) -> String { let mut o = String::from("\""); for c in s.chars() { if (c as u32) < 32 { continue } if c == '"' || c == '\\' { o.push('\\') } o.push(c) } o.push('"'); o }
fn dir(home: &Path) -> std::io::Result<PathBuf> { let d = home.join("secrets").join("boat"); std::fs::DirBuilder::new().mode(0o700).recursive(true).create(&d)?; Ok(d) }
fn w(p: &Path, b: &[u8]) -> std::io::Result<()> { std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(p).and_then(|mut f| f.write_all(b)) }
fn tag() -> String { hex::encode(crate::net::pairing::urandom(8)) }

/// One request. `bearer` "" sends none. `upload`: a file sent as the body (`upload-file`, for a signed PUT URL).
pub struct Call { pub method: String, pub url: String, pub bearer: String, pub body: Option<Vec<u8>>, pub upload: Option<PathBuf>, pub headers: Vec<String>, pub secs: u64 }
impl Call {
    pub fn new(method: &str, url: String, bearer: &str) -> Call { Call { method: method.into(), url, bearer: bearer.into(), body: None, upload: None, headers: vec![], secs: 40 } }
    pub fn json(mut self, v: &Value) -> Call { self.body = Some(v.to_string().into_bytes()); self.headers.push("Content-Type: application/json".into()); self }
    pub fn secs(mut self, s: u64) -> Call { self.secs = s; self }
}

/// The request, off the async threads.
pub async fn send(home: &Path, c: Call) -> Resp {
    let home = home.to_path_buf();
    tokio::task::spawn_blocking(move || send_blocking(&home, &c)).await.unwrap_or_default()
}
fn send_blocking(home: &Path, c: &Call) -> Resp {
    let Ok(d) = dir(home) else { return Resp { code: "000".into(), body: vec![], err: "could not make the secrets folder".into() } };
    let n = tag();
    let (cfg, inp, out) = (d.join(format!("b-{n}.curl")), d.join(format!("b-{n}.in")), d.join(format!("b-{n}.out")));
    let mut k = format!("url = {}\nrequest = {}\n", q(&c.url), q(&c.method));
    if !c.bearer.is_empty() { k.push_str(&format!("header = {}\n", q(&format!("Authorization: Bearer {}", c.bearer)))) }
    for h in &c.headers { k.push_str(&format!("header = {}\n", q(h))) }
    if c.body.is_some() { k.push_str(&format!("data-binary = {}\n", q(&format!("@{}", inp.to_string_lossy())))) }
    if let Some(f) = &c.upload { k.push_str(&format!("upload-file = {}\n", q(&f.to_string_lossy()))) }
    k.push_str(&format!("max-time = {}\noutput = {}\nwrite-out = \"%{{http_code}}\"\n", c.secs, q(&out.to_string_lossy())));
    let r = (|| -> Result<Resp, String> {
        if let Some(b) = &c.body { w(&inp, b).map_err(|e| e.to_string())? }
        w(&cfg, k.as_bytes()).map_err(|e| e.to_string())?;
        let o = crate::proc::std_cmd(crate::proc::Kind::Net, "curl").args(["-sS", "--max-redirs", "0", "-K"]).arg(&cfg).current_dir(&d).output().map_err(|e| e.to_string())?;
        let said = String::from_utf8_lossy(&o.stdout).trim().to_string();
        let code: String = said.chars().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect();
        // curl's stderr can name the URL (a signed upload URL holds a token): only its first words are kept, never a URL
        let err: String = String::from_utf8_lossy(&o.stderr).split_whitespace().filter(|w| !w.contains("://") && !w.contains('?')).collect::<Vec<_>>().join(" ").chars().take(160).collect();
        Ok(Resp { code: if code.len() == 3 && code.bytes().all(|b| b.is_ascii_digit()) { code } else { "000".into() }, body: std::fs::read(&out).unwrap_or_default(), err })
    })();
    for p in [&cfg, &inp, &out] { let _ = std::fs::remove_file(p); }
    r.unwrap_or_else(|e| Resp { code: "000".into(), body: vec![], err: e })
}

/// The person's Boat account, as one client: the key, and the calls the always-on Boat needs.
#[derive(Clone)]
pub struct Boat { pub home: PathBuf, key: String, api: String }
impl Boat {
    /// The key in the vault (None when there is none, or the vault is locked).
    pub fn from_vault(home: &Path) -> Option<Boat> { crate::settings::vault::person_get(KEY).filter(|k| !k.trim().is_empty()).map(|k| Boat::with_key(home, k.trim())) }
    pub fn with_key(home: &Path, key: &str) -> Boat { Boat { home: home.to_path_buf(), key: key.to_string(), api: api_base() } }
    fn url(&self, path: &str) -> String { format!("{}{path}", self.api) }
    async fn call(&self, method: &str, path: &str, body: Option<Value>, secs: u64) -> Result<Value, String> {
        let mut c = Call::new(method, self.url(path), &self.key).secs(secs);
        if let Some(b) = body { c = c.json(&b) }
        let r = send(&self.home, c).await;
        if r.ok() { Ok(r.json()) } else { Err(r.why()) }
    }
    pub async fn get(&self, path: &str) -> Result<Value, String> { self.call("GET", path, None, 40).await }

    /// `GET /me` and `GET /limits`, as the account row shows them: `{email, login, plan, plan_dollars, hours_left, hours, period_end, rate_hour, types, trial}`,
    /// and what this key may do: `actions` (["*"] for a sign-in or an admin key) and `missing`, the actions the always-on Boat needs that it lacks.
    pub async fn account(&self) -> Result<Value, String> {
        let me = self.get("/me").await.map_err(|e| if e.contains("api_key_action_forbidden") || e.contains("403") { format!("This key cannot read your Boat account: it lacks account.read. {}", MAKE_KEY) } else { e })?;
        let l = self.get("/limits").await?;
        let mut a = account_of(&me, &l);
        let acts = self.get("/api-keys").await.ok().and_then(|k| key_actions(&k, &self.key));
        a["actions"] = json!(acts.clone().unwrap_or_else(|| vec!["*".to_string()]));
        a["missing"] = json!(missing(acts.as_deref().unwrap_or(&["*".to_string()])));
        Ok(a)
    }
    /// A key of the always-on Boat's own (`ACTIONS`, a year), made with this sign-in's token: the secret, shown once by Boat.
    pub async fn make_key(&self, name: &str) -> Result<String, String> {
        let r = self.call("POST", "/api-keys/scoped", Some(json!({"name": name, "ttl": "365d", "actions": ACTIONS})), 40).await?;
        r["secret"].as_str().or_else(|| r["apiKey"]["secret"].as_str()).filter(|s| !s.is_empty()).map(str::to_string).ok_or_else(|| "Boat made the key but did not hand it over".into())
    }
    /// End this sign-in's session (the CLI's own logout), once its narrower key is kept.
    pub async fn logout(&self) -> Result<(), String> {
        let r = send(&self.home, Call::new("POST", format!("{}/api/boat/auth/logout", web_base()), &self.key).secs(20)).await;
        if r.ok() { Ok(()) } else { Err(r.why()) }
    }
    /// `POST /sandboxes`: no auto-stop (`ttlSeconds: null`), none of the account's stored logins (`noEnv`), labelled so it can be found again.
    /// The Idempotency-Key makes a retry after a lost answer return the same Boat rather than a second one that bills.
    pub async fn create(&self, size: &str, name: &str, idem: &str) -> Result<Value, String> {
        let mut c = Call::new("POST", self.url("/sandboxes"), &self.key).secs(90).json(&create_body(size, name));
        if !idem.is_empty() { c.headers.push(format!("Idempotency-Key: {idem}")) }
        let r = send(&self.home, c).await;
        if r.ok() { Ok(r.json()["sandbox"].clone()) } else { Err(r.why()) }
    }
    pub async fn sandbox(&self, id: &str) -> Result<Value, String> { Ok(self.get(&format!("/sandboxes/{id}")).await?["sandbox"].clone()) }
    /// `DELETE /sandboxes/{id}`: Boat asks for the id again in `X-Ascii-Confirm-Delete` (a mismatch is refused with 409).
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let mut c = Call::new("DELETE", self.url(&format!("/sandboxes/{id}")), &self.key).secs(60);
        c.headers.push(format!("X-Ascii-Confirm-Delete: {id}"));
        let r = send(&self.home, c).await;
        if r.ok() { Ok(()) } else { Err(r.why()) }
    }
    pub async fn stop(&self, id: &str) -> Result<(), String> { self.call("POST", &format!("/sandboxes/{id}/stop"), Some(json!({})), 120).await.map(|_| ()) }
    /// Resume, optionally on another size (Resize: stop, then resume with `type`).
    pub async fn resume(&self, id: &str, size: &str) -> Result<(), String> {
        let b = if size.is_empty() { json!({}) } else { json!({"type": size}) };
        self.call("POST", &format!("/sandboxes/{id}/resume"), Some(b), 120).await.map(|_| ())
    }
    /// No auto-stop again (a Boat made elsewhere, or one whose TTL came back after a resume).
    pub async fn keep_on(&self, id: &str) -> Result<(), String> { self.call("PATCH", &format!("/sandboxes/{id}"), Some(json!({"ttlSeconds": null})), 40).await.map(|_| ()) }
    /// A command, waited for (at most `secs`, Boat's limit is 600): `{exitCode, stdout, stderr}`.
    pub async fn run(&self, id: &str, cmd: &str, secs: u64) -> Result<Value, String> {
        self.call("POST", &format!("/sandboxes/{id}/commands"), Some(json!({"command": cmd, "timeoutSeconds": secs.min(600)})), secs.min(600) + 30).await
    }
    /// `run`, and an exit code other than 0 is an error with the command's last words.
    pub async fn sh(&self, id: &str, cmd: &str, secs: u64) -> Result<String, String> {
        let r = self.run(id, cmd, secs).await?;
        let out = r["stdout"].as_str().unwrap_or("").to_string();
        if r["exitCode"].as_i64() == Some(0) { return Ok(out) }
        let tail = |s: &str| s.trim().lines().rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ");
        let e = r["stderr"].as_str().unwrap_or("");
        Err(format!("exit {}: {}", r["exitCode"], if e.trim().is_empty() { tail(&out) } else { tail(e) }).chars().take(300).collect())
    }
    /// A detached command (a long install): its process id, then `command_status`.
    pub async fn detach(&self, id: &str, cmd: &str) -> Result<String, String> {
        let r = self.call("POST", &format!("/sandboxes/{id}/commands"), Some(json!({"command": cmd, "detached": true})), 60).await?;
        // (Boat's processId is a number; commandId names its history entry)
        r["processId"].as_u64().map(|n| n.to_string()).or_else(|| r["processId"].as_str().map(str::to_string)).ok_or_else(|| "Boat started the command but named no process".into())
    }
    pub async fn command_status(&self, id: &str, pid: &str) -> Result<Value, String> { self.get(&format!("/sandboxes/{id}/commands/{pid}")).await }
    /// A small text file on the Boat (`PUT /files`).
    pub async fn write(&self, id: &str, path: &str, content: &str) -> Result<(), String> {
        self.call("PUT", &format!("/sandboxes/{id}/files"), Some(json!({"path": path, "content": content, "encoding": "utf8"})), 60).await.map(|_| ())
    }
    /// A local file to the Boat: a signed upload URL (`POST /files/upload-url`), then the file PUT to it. The URL is never shown or kept.
    pub async fn upload(&self, id: &str, local: &Path, path: &str) -> Result<(), String> {
        let u = self.call("POST", &format!("/sandboxes/{id}/files/upload-url"), Some(json!({"path": path, "ttlSeconds": 900})), 40).await?;
        let Some(url) = u["url"].as_str() else { return Err("Boat gave no upload address".into()) };
        let mut c = Call::new(u["method"].as_str().unwrap_or("PUT"), url.to_string(), "").secs(900);
        c.upload = Some(local.to_path_buf());
        let r = send(&self.home, c).await;
        if r.ok() { Ok(()) } else { Err(format!("the upload failed: {}", if r.code == "000" { r.why() } else { format!("HTTP {}", r.code) })) }
    }
    /// `POST /host`: a stable HTTPS address for a port on the Boat's loopback. `public`: no Boat token in front (the hub's own auth is).
    pub async fn host(&self, id: &str, port: u16, public: bool) -> Result<String, String> {
        let r = self.call("POST", &format!("/sandboxes/{id}/host"), Some(json!({"port": port, "public": public, "localhost": true, "title": "Backplane"})), 60).await?;
        r["url"].as_str().map(|u| u.split('?').next().unwrap_or(u).trim_end_matches('/').to_string()).ok_or_else(|| "Boat gave no address".into())
    }
    /// What the Boat used since `since` (ISO time): `{seconds, dollars, billingMultiplier}`.
    pub async fn usage(&self, id: &str, since: &str) -> Result<Value, String> {
        let p = if since.is_empty() { format!("/sandboxes/{id}/usage") } else { format!("/sandboxes/{id}/usage?since={since}") };
        self.get(&p).await
    }
}

/// What the always-on Boat asks of the person's Boat key, and nothing more: make, read and keep on (update: no auto-stop), stop and resume (Restart,
/// Resize), delete only the Boats this key made, run commands, write files (the build goes up through a signed upload URL), host the hub's port,
/// and read the account and its usage. Checked against Boat on Nov 11 2026: each call the Boat makes answered with a key of exactly these actions.
pub const ACTIONS: &[&str] = &["sandbox.create", "sandbox.read", "sandbox.update", "sandbox.stop", "sandbox.resume", "sandbox.delete-own", "exec", "file.write", "host", "account.read"];
/// How to make that key, said where the key is pasted.
pub fn key_command() -> String { format!("boat api-key create backplane-always-on --ttl 365d --actions {}", ACTIONS.join(",")) }
pub const MAKE_KEY: &str = "Make one with the Boat command below, or on boat.dev, Dashboard, API Keys: New key, with the actions listed.";
/// This key's actions, from `GET /api-keys` (Boat shows each key's prefix and last four). None when it is not listed: a sign-in's session, which may do everything.
pub fn key_actions(list: &Value, key: &str) -> Option<Vec<String>> {
    let k = list["apiKeys"].as_array()?.iter().find(|k| {
        let (p, l) = (k["keyPrefix"].as_str().unwrap_or(""), k["keyLastFour"].as_str().unwrap_or(""));
        !p.is_empty() && !l.is_empty() && key.starts_with(p) && key.ends_with(l)
    })?;
    Some(k["scope"]["actions"].as_array()?.iter().filter_map(|a| a.as_str().map(str::to_string)).collect())
}
/// The actions the always-on Boat needs that `have` lacks (`*` has them all; `sandbox.delete` covers `sandbox.delete-own`).
pub fn missing(have: &[String]) -> Vec<&'static str> {
    if have.iter().any(|a| a == "*") { return vec![] }
    ACTIONS.iter().copied().filter(|n| !have.iter().any(|h| h == n || (*n == "sandbox.delete-own" && h == "sandbox.delete"))).collect()
}

pub fn create_body(size: &str, name: &str) -> Value {
    json!({"type": size, "ttlSeconds": null, "noEnv": true, "name": name, "labels": {"backplane": "alwayson"}})
}

/// The account row's facts from `GET /me` and `GET /limits`.
pub fn account_of(me: &Value, l: &Value) -> Value {
    let u = &me["user"];
    let pi = &l["planIncluded"];
    let types: Vec<Value> = l["machineTypes"].as_array().into_iter().flatten().filter(|t| t["available"] != false).filter_map(|t| t["type"].as_str().map(|s| json!(s))).collect();
    json!({
        "email": u["email"].as_str().unwrap_or(""), "login": u["login"].as_str().unwrap_or(""),
        "plan": l["planName"].as_str().or_else(|| l["plan"].as_str()).unwrap_or(""), "plan_dollars": l["sandboxPlanDollars"].as_f64().unwrap_or(0.0),
        "hours_left": pi["vmHoursRemaining"].as_f64().unwrap_or(0.0), "hours": pi["vmHours"].as_f64().unwrap_or(0.0), "period_end": pi["periodEnd"].as_str().unwrap_or(""),
        "rate_hour": l["overagePrices"]["vmHourDollars"].as_f64().unwrap_or(super::sizes::DEFAULT_RATE),
        "wallet": l["wallet"]["balanceDollars"].as_f64().unwrap_or(0.0),
        "types": types, "trial": l["accessTier"].as_str() == Some("trial") || !l["subscriptionTrialEndsAt"].is_null() && l["hasPaymentHistory"] == false,
        "can_start": l["canStart"] != false, "blocked": l["startBlockedReason"].as_str().or_else(|| l["blockedReason"].as_str()).unwrap_or(""),
    })
}

/// The browser sign-in Boat's CLI uses (best-effort: not in the documented API). `provider` is github or google.
pub async fn login_start(home: &Path, provider: &str) -> Result<Value, String> {
    let r = send(home, Call::new("POST", format!("{}/api/boat/cli/login/start", web_base()), "").secs(30).json(&json!({"provider": provider}))).await;
    if !r.ok() { return Err(format!("Boat's sign-in did not start: {}. Paste an API key instead.", r.why())) }
    let j = r.json();
    match (j["login_url"].as_str(), j["poll_token"].as_str()) {
        (Some(u), Some(p)) if u.starts_with("https://") && !p.is_empty() => Ok(json!({"url": u, "poll": p, "code": j["user_code"].as_str().unwrap_or(""), "expires": j["expires_at"].as_str().unwrap_or("")})),
        _ => Err("Boat's sign-in answered in a form this Backplane does not know. Paste an API key instead.".into()),
    }
}
/// One poll: Ok(Some(token)) once signed in, Ok(None) while pending, Err when it ended (expired, refused, or the endpoint changed).
pub async fn login_poll(home: &Path, poll: &str) -> Result<Option<String>, String> {
    if !poll.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err("bad sign-in".into()) }
    let r = send(home, Call::new("GET", format!("{}/api/boat/cli/login/poll/{poll}", web_base()), "").secs(20)).await;
    if !r.ok() { return Err(format!("Boat's sign-in stopped: {}. Paste an API key instead.", r.why())) }
    let j = r.json();
    match (j["status"].as_str().unwrap_or(""), j["token"].as_str()) {
        (_, Some(t)) if !t.is_empty() => Ok(Some(t.to_string())),
        ("pending" | "waiting" | "", _) => Ok(None),
        (s, _) => Err(format!("Boat's sign-in ended: {s}. Try again, or paste an API key.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_create_asks_for_no_auto_stop_and_none_of_the_accounts_logins() {
        let b = create_body("small", "bp-always-on");
        assert_eq!(b["ttlSeconds"], Value::Null); assert_eq!(b["noEnv"], true); assert_eq!(b["type"], "small"); assert_eq!(b["labels"]["backplane"], "alwayson");
    }
    #[test] fn the_account_row_reads_me_and_limits() {
        let me = json!({"user": {"email": "a@b.c", "login": "a"}});
        let l = json!({"planName": "Standard", "sandboxPlanDollars": 100, "planIncluded": {"vmHours": 2777.78, "vmHoursRemaining": 2413.0, "periodEnd": "2026-12-03T17:19:55.000Z"},
            "overagePrices": {"vmHourDollars": 0.036}, "machineTypes": [{"type": "small", "available": true}, {"type": "xlarge", "available": false}], "accessTier": "standard", "canStart": true});
        let a = account_of(&me, &l);
        assert_eq!((a["email"].as_str(), a["plan"].as_str(), a["hours_left"].as_f64()), (Some("a@b.c"), Some("Standard"), Some(2413.0)));
        assert_eq!(a["types"], json!(["small"])); assert_eq!(a["trial"], false);
    }
    #[test] fn the_keys_actions_are_read_from_boats_list_and_what_it_lacks_is_named() {
        let list = json!({"apiKeys": [
            {"keyPrefix": "boat_4ftM", "keyLastFour": "EsZ1", "scope": {"actions": ["*"]}},
            {"keyPrefix": "boat_abcd", "keyLastFour": "WXYZ", "scope": {"actions": ["sandbox.read", "sandbox.update", "exec", "file.read", "file.write", "host", "account.read"]}}]});
        let fs = key_actions(&list, "boat_abcd1234567890WXYZ").unwrap();
        assert_eq!(missing(&fs), ["sandbox.create", "sandbox.stop", "sandbox.resume", "sandbox.delete-own"], "Boat's full-sandbox preset cannot create");
        assert!(missing(&key_actions(&list, "boat_4ftMzzzEsZ1").unwrap()).is_empty(), "an admin key");
        assert_eq!(key_actions(&list, "session-token-not-listed"), None, "a sign-in's session is not a listed key");
        assert!(missing(&ACTIONS.iter().map(|a| a.to_string()).collect::<Vec<_>>()).is_empty());
        let mut d: Vec<String> = ACTIONS.iter().map(|a| a.to_string()).collect(); d.retain(|a| a != "sandbox.delete-own"); d.push("sandbox.delete".into());
        assert!(missing(&d).is_empty(), "delete covers delete-own");
        assert!(key_command().starts_with("boat api-key create backplane-always-on --ttl 365d --actions sandbox.create,sandbox.read,") && !key_command().contains("admin") && !key_command().contains('*'));
    }
    #[test] fn a_refused_key_says_to_connect_again_and_boats_message_is_kept() {
        assert!(Resp { code: "401".into(), body: b"{}".to_vec(), err: String::new() }.why().contains("Connect the account again"));
        assert_eq!(Resp { code: "409".into(), body: br#"{"ok":false,"message":"sandbox_starting"}"#.to_vec(), err: String::new() }.why(), "Boat answered 409: sandbox_starting");
        assert!(Resp { code: "000".into(), body: vec![], err: "timeout".into() }.why().starts_with("Boat did not answer"));
    }
}

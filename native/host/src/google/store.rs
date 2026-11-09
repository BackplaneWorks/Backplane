//! Where Google's sign-ins live. Each account is one vault item `hub.google.<id>` (settings/vault.rs: AES-256-GCM, the key in the OS keyring), holding
//! `{email, refresh, scopes, client, at, state, why}`: the refresh token never leaves the vault and this module, and `hub.` names are refused to secret_run, the
//! secret box and secret_request. Access tokens are kept in memory only. The OAuth client is the environment's, else one baked into this build, else the person's
//! vault secrets `google-client-id` and `google-client-secret` (put there with secret_request). Which bots may use which accounts is the setting
//! `google.bot.<bot>`: the account ids, one a line (not secret).
use crate::settings::vault;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const PREFIX: &str = "hub.google.";
pub const CLIENT_ID: &str = "google-client-id";
pub const CLIENT_SECRET: &str = "google-client-secret";

/// An account's id: the first 16 hex of the SHA-256 of its address in lower case (a vault name, a setting key, a phone's profile id after `google:`).
pub fn id_of(email: &str) -> String { hex::encode(Sha256::digest(email.trim().to_lowercase().as_bytes()))[..16].to_string() }
pub fn id_ok(id: &str) -> bool { id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) }

#[derive(Debug, Clone, PartialEq)]
pub struct Acct { pub id: String, pub email: String, pub refresh: String, pub scopes: Vec<String>, pub client: String, pub at: u64, pub state: String, pub why: String }
impl Acct {
    pub fn expired(&self) -> bool { self.state == "expired" }
    fn json(&self) -> String { json!({"email":self.email,"refresh":self.refresh,"scopes":self.scopes,"client":self.client,"at":self.at,"state":self.state,"why":self.why}).to_string() }
    fn of(id: &str, text: &str) -> Option<Acct> {
        let j: Value = serde_json::from_str(text).ok()?;
        let s = |k: &str| j[k].as_str().unwrap_or("").to_string();
        Some(Acct { id: id.into(), email: s("email"), refresh: s("refresh"), scopes: j["scopes"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            client: s("client"), at: j["at"].as_u64().unwrap_or(0), state: s("state"), why: s("why") })
    }
}

/// Every account signed in, in the order they were added (the first is the default a tool uses when it names none).
pub fn accounts() -> Vec<Acct> {
    let mut v: Vec<Acct> = vault::own_names(PREFIX).iter().filter_map(|n| { let id = n.strip_prefix(PREFIX)?; Acct::of(id, &vault::own_get(n)?) }).collect();
    v.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.email.cmp(&b.email))); v
}
pub fn get(id: &str) -> Option<Acct> { if !id_ok(id) { return None } Acct::of(id, &vault::own_get(&format!("{PREFIX}{id}"))?) }
/// Kept (a sign-in to an address already here replaces it, keeping when it was first added). "ok" or why not.
pub fn put(a: &Acct) -> Result<(), String> {
    let r = vault::own_put(&format!("{PREFIX}{}", a.id), &a.json(), &format!("Google sign-in for {}", a.email));
    if crate::settings::secret::saved_is(&r) { Ok(()) } else { Err(r.strip_prefix("fail ").unwrap_or(&r).to_string()) }
}
pub fn del(id: &str) -> Result<(), String> { let r = vault::own_del(&format!("{PREFIX}{id}")); if crate::settings::secret::saved_is(&r) || r.starts_with("fail no secret") { Ok(()) } else { Err(r.strip_prefix("fail ").unwrap_or(&r).to_string()) } }

/// The OAuth client a sign-in uses, and where it came from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Client { pub id: String, pub secret: String, pub from: &'static str }
impl Client { pub fn ready(&self) -> bool { !self.id.is_empty() && !self.secret.is_empty() } }
/// The client: BACKPLANE_GOOGLE_CLIENT_ID/_SECRET at run time, else the pair this build was made with, else the vault's google-client-id/-secret.
/// An installed app's secret is not a secret (RFC 8252 8.5); PKCE protects the code. It still stays out of the repo, the log and every reply.
pub fn client() -> Client {
    let env = |k: &str| std::env::var(k).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if let (Some(id), Some(secret)) = (env("BACKPLANE_GOOGLE_CLIENT_ID"), env("BACKPLANE_GOOGLE_CLIENT_SECRET")) { return Client { id, secret, from: "environment" } }
    if let (Some(id), Some(secret)) = (option_env!("BACKPLANE_GOOGLE_CLIENT_ID").filter(|s| !s.is_empty()), option_env!("BACKPLANE_GOOGLE_CLIENT_SECRET").filter(|s| !s.is_empty())) { return Client { id: id.into(), secret: secret.into(), from: "build" } }
    match (vault::person_get(CLIENT_ID), vault::person_get(CLIENT_SECRET)) { (Some(id), Some(secret)) => Client { id: id.trim().into(), secret: secret.trim().into(), from: "vault" }, _ => Client::default() }
}

/// The setting naming the accounts a bot may use.
pub fn grant_key(bot: &str) -> String { format!("google.bot.{bot}") }
pub fn grants_of(text: &str) -> Vec<String> { text.lines().map(str::trim).filter(|l| id_ok(l)).map(str::to_string).collect() }
pub fn grants_text(ids: &[String]) -> String { ids.join("\n") }
/// The grant changed: an account added or dropped, the order kept.
pub fn granted(text: &str, id: &str, on: bool) -> String {
    let mut v = grants_of(text); v.retain(|x| x != id); if on { v.push(id.into()) } grants_text(&v)
}

/// 0.14's `<old>/secrets/google.json`: its OAuth client (as google-client-id/-secret, when none is set here) and every account with a refresh token that is not
/// here yet. A refresh token belongs to the client that got it: the account keeps that client's id. Grants are not copied (0.14 let every bot use every account;
/// here the person allows each bot). How many accounts came across.
pub fn adopt_014(old: &std::path::Path) -> usize {
    let Some(j) = std::fs::read_to_string(old.join("secrets").join("google.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return 0 };
    let s = |v: &Value, k: &str| v[k].as_str().unwrap_or("").trim().to_string();
    let (cid, csec) = (s(&j, "client_id"), s(&j, "client_secret"));
    let have = vault::names();
    if !cid.is_empty() && !csec.is_empty() && !have.contains(&CLIENT_ID.to_string()) && !have.contains(&CLIENT_SECRET.to_string()) {
        let _ = vault::put(CLIENT_ID, &cid, "Google OAuth client id (from 0.14)", "import"); let _ = vault::put(CLIENT_SECRET, &csec, "Google OAuth client secret (from 0.14)", "import");
    }
    // a file from before accounts kept its one account's fields at its top
    let mut xs: Vec<Value> = j["accounts"].as_array().cloned().unwrap_or_default(); if xs.is_empty() && !s(&j, "refresh_token").is_empty() { xs.push(j.clone()) }
    let now = crate::hub::now_ms() / 1000; let mut n = 0;
    for (i, a) in xs.iter().enumerate() {
        let (email, refresh) = (s(a, "email"), s(a, "refresh_token"));
        if email.is_empty() || refresh.is_empty() || get(&id_of(&email)).is_some() { continue }
        let acct = Acct { id: id_of(&email), email, refresh, scopes: super::oauth::SCOPES.iter().map(|x| x.to_string()).collect(), client: cid.clone(), at: now + i as u64, state: "ok".into(), why: String::new() };
        if put(&acct).is_ok() { n += 1 }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn ids_are_stable_short_and_ignore_case() { assert_eq!(id_of("A@B.com"), id_of(" a@b.com ")); assert!(id_ok(&id_of("x@y.z"))); assert!(!id_ok("../x") && !id_ok("ABCDEF0123456789")); }
    #[test] fn grants_add_and_drop_in_order() {
        let (a, b) = (id_of("a@x"), id_of("b@x"));
        let t = granted(&granted("", &a, true), &b, true); assert_eq!(grants_of(&t), vec![a.clone(), b.clone()]);
        assert_eq!(grants_of(&granted(&t, &a, false)), vec![b.clone()]); assert_eq!(grants_of("junk\n../x"), Vec::<String>::new());
    }
    #[test] fn a_014_sign_in_comes_across_without_any_grant() {
        let _l = vault::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); std::env::set_var("BACKPLANE_VAULT_KEYRING", "file");
        let (home, old) = (vault::test_home("g-new"), vault::test_home("g-old"));
        std::fs::create_dir_all(old.join("secrets")).unwrap();
        std::fs::write(old.join("secrets/google.json"), r#"{"client_id":"old-client.apps.googleusercontent.com","client_secret":"old-client-secret","accounts":[{"email":"a@example.com","refresh_token":"1//old-test-a","access_token":"x","expiry":1},{"email":"b@example.com","refresh_token":""}]}"#).unwrap();
        vault::open(&home);
        assert_eq!(adopt_014(&old), 1); assert_eq!(adopt_014(&old), 0, "once");
        let a = get(&id_of("a@example.com")).unwrap(); assert_eq!((a.refresh.as_str(), a.client.as_str(), a.state.as_str()), ("1//old-test-a", "old-client.apps.googleusercontent.com", "ok"));
        assert_eq!(vault::person_get(CLIENT_ID).as_deref(), Some("old-client.apps.googleusercontent.com"));
    }
    #[test] fn an_account_round_trips_through_its_vault_text() {
        let a = Acct { id: id_of("a@b.c"), email: "a@b.c".into(), refresh: "1//r".into(), scopes: vec!["email".into()], client: "cid".into(), at: 5, state: "ok".into(), why: "".into() };
        assert_eq!(Acct::of(&a.id, &a.json()).unwrap(), a);
    }
}

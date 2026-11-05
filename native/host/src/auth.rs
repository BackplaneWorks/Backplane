//! Who may connect: the hub's pairing token (<home>/token) or a paired device's credential
//! (devices-v1.json), exactly as core/auth.bend and effects/pairing.js decide it.
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct Req { pub method: String, pub target: String, pub headers: Vec<(String, String)> }
impl Req {
    pub fn header(&self, name: &str) -> &str {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).unwrap_or("")
    }
    pub fn route(&self) -> &str { self.target.split('?').next().unwrap_or("") }
    pub fn param(&self, name: &str) -> String {
        self.target.split_once('?').map(|(_, q)| q).unwrap_or("").split('&')
            .find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v.to_string())).unwrap_or_default()
    }
}

fn same(a: &str, b: &str) -> bool {
    // constant time over equal lengths (core/auth.bend Auth.same)
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}
fn digest(x: &str) -> String { hex::encode(Sha256::digest(x.as_bytes())) }

pub struct Auth { root: String, devices: Vec<(String, bool)> }
impl Auth {
    pub fn load(home: &Path) -> Auth {
        let root = std::fs::read_to_string(home.join("token")).unwrap_or_default().trim().to_string();
        let mut devices = Vec::new();
        if let Ok(text) = std::fs::read_to_string(home.join("devices-v1.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                for d in v["devices"].as_array().into_iter().flatten() {
                    if let Some(h) = d["hash"].as_str() { devices.push((h.to_string(), d["revoked"].as_bool().unwrap_or(false))) }
                }
            }
        }
        Auth { root, devices }
    }
    pub fn with_root(root: &str) -> Auth { Auth { root: root.to_string(), devices: vec![] } }
    /// Origin is absent (a native client) or the host's own; a cross-site fetch is refused.
    pub fn browser(r: &Req) -> bool {
        let (o, h) = (r.header("origin").to_lowercase(), r.header("host").to_lowercase());
        let origin = o.is_empty() || (!h.is_empty() && (o == format!("http://{h}") || o == format!("https://{h}")));
        origin && !r.header("sec-fetch-site").eq_ignore_ascii_case("cross-site")
    }
    fn cookie(r: &Req) -> String {
        r.header("cookie").split(';').find_map(|c| c.trim().strip_prefix("bp_token=").map(str::to_string)).unwrap_or_default()
    }
    /// Pairing.resolve: the credential the request presents (bearer, else cookie, else ?token=),
    /// the hub token itself, or a paired device's 32-hex token; "" when none holds.
    fn resolve(&self, r: &Req) -> String {
        let bearer = r.header("authorization");
        let secret = if let Some(b) = bearer.strip_prefix("Bearer ") { b.to_string() } else {
            let c = Self::cookie(r); if c.is_empty() { r.param("token") } else { c } };
        if secret.is_empty() || same(&self.root, &secret) { return self.root.clone() }
        if secret.len() != 32 || !secret.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) { return String::new() }
        let d = digest(&secret);
        if self.devices.iter().any(|(h, revoked)| !*revoked && same(h, &d)) { secret } else { String::new() }
    }
    /// Auth.allowed: browser rules, then a credential equal to the resolved token.
    pub fn allowed(&self, r: &Req) -> bool {
        let t = self.resolve(r);
        Self::browser(r) && !t.is_empty() && (same(r.header("authorization"), &format!("Bearer {t}")) || same(&r.param("token"), &t) || same(&Self::cookie(r), &t))
    }
    /// Auth.cache_request: GET, browser rules, and the bearer or the cookie (never ?token=).
    pub fn paired_request(&self, r: &Req) -> bool {
        r.method == "GET" && Self::browser(r) && !self.root.is_empty()
            && (same(r.header("authorization"), &format!("Bearer {}", self.root)) || same(&Self::cookie(r), &self.root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn req(target: &str, hs: &[(&str, &str)]) -> Req { Req { method: "GET".into(), target: target.into(), headers: hs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() } }
    fn h(k: &str, v: &str) -> (&'static str, String) { (Box::leak(k.to_string().into_boxed_str()), v.to_string()) }
    fn with(hs: &[(&str, &str)], path: &str) -> Req { req(path, hs) }
    // laws auth_* of LAWS.bend, stated over many paths and tokens
    #[test] fn auth_foreign_origin_denied() {
        for path in ["/ws", "/ws?token=secret", "/x?token=t&y=1", ""] { for token in ["secret", "t", "", "a b"] {
            let a = Auth::with_root(token);
            assert!(!a.allowed(&with(&[("origin", "https://foreign.example"), ("host", "localhost:3787")], path))); } }
    }
    #[test] fn auth_empty_remote_token_denied() { assert!(!Auth::with_root("").allowed(&with(&[], "/ws?token="))); }
    #[test] fn auth_cookie_prefix_denied() { assert!(!Auth::with_root("secret").allowed(&with(&[("cookie", "bp_token=secret-extra")], "/ws"))); }
    #[test] fn auth_loopback_host_needs_token() { assert!(!Auth::with_root("secret").allowed(&with(&[("host", "127.0.0.1:3787")], "/ws"))); }
    #[test] fn auth_localhost_needs_token() { assert!(!Auth::with_root("secret").allowed(&with(&[("host", "localhost:3787")], "/mcp/t1"))); }
    #[test] fn auth_no_host_needs_token() { assert!(!Auth::with_root("secret").allowed(&with(&[], "/ws"))); }
    #[test] fn auth_loopback_with_token() { assert!(Auth::with_root("secret").allowed(&with(&[("host", "127.0.0.1:3787")], "/ws?token=secret"))); let _ = h; }
    #[test] fn token_forms() {
        let a = Auth::with_root("tok123");
        assert!(a.allowed(&req("/ws?token=tok123", &[]))); assert!(a.allowed(&req("/ws", &[("Authorization", "Bearer tok123")])));
        assert!(a.allowed(&req("/ws", &[("Cookie", "x=1; bp_token=tok123")])));
        assert!(!a.allowed(&req("/ws?token=nope", &[]))); assert!(!a.allowed(&req("/ws", &[])));
    }
    #[test] fn an_empty_token_admits_nobody() { let a = Auth::with_root(""); assert!(!a.allowed(&req("/ws?token=", &[]))); assert!(!a.allowed(&req("/ws", &[]))); }
    #[test] fn browser_rules() {
        let a = Auth::with_root("t");
        assert!(!a.allowed(&req("/ws?token=t", &[("Origin", "http://evil.example"), ("Host", "hub:1")])));
        assert!(a.allowed(&req("/ws?token=t", &[("Origin", "http://hub:1"), ("Host", "hub:1")])));
        assert!(!a.allowed(&req("/ws?token=t", &[("Sec-Fetch-Site", "cross-site")])));
    }
    #[test] fn capability_takes_no_query_token() {
        let a = Auth::with_root("t");
        assert!(!a.paired_request(&req("/client/capabilities?token=t", &[]))); assert!(a.paired_request(&req("/client/capabilities", &[("Authorization", "Bearer t")])));
    }
    #[test] fn paired_device() {
        let dev = "0123456789abcdef0123456789abcdef";
        let mut a = Auth::with_root("root"); a.devices.push((digest(dev), false));
        assert!(a.allowed(&req(&format!("/ws?token={dev}"), &[]))); a.devices[0].1 = true; assert!(!a.allowed(&req(&format!("/ws?token={dev}"), &[])));
    }
}

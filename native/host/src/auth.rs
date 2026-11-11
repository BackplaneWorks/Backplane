//! Who may connect: the hub's pairing token (<home>/token) or a paired device's credential
//! (devices-v1.json), exactly as core/auth and effects/pairing.js decide it.
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Req {
    pub method: String, pub target: String, pub headers: Vec<(String, String)>,
    /// the request body, read by the hub only for POSTs to routes that decide for themselves (`own_auth`, /auth/*): at most `BODY_MAX`
    pub body: Vec<u8>,
    /// the accepted socket's peer (owner trust asks the tailnet about this address); None in tests
    pub peer: Option<std::net::SocketAddr>,
    /// The socket's peer is the owner's own device (decided by the tailnet listener's whois) or loopback. NEVER a credential by itself:
    /// handlers still apply owner_pair / owner_peer / the credential.
    pub trusted: bool,
}
/// The most body bytes the hub reads for a request (/far bodies reach 256 KB).
pub const BODY_MAX: usize = 1 << 20;
impl Req {
    pub fn new(method: &str, target: &str, headers: Vec<(String, String)>) -> Req { Req { method: method.into(), target: target.into(), headers, body: vec![], peer: None, trusted: false } }
    pub fn header(&self, name: &str) -> &str {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).unwrap_or("")
    }
    pub fn route(&self) -> &str { self.target.split('?').next().unwrap_or("") }
    pub fn param(&self, name: &str) -> String {
        // H.Http.param: the segment after the first '?' up to the next '?' (a second '?' ends the query)
        self.target.split('?').nth(1).unwrap_or("").split('&')
            .find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v.to_string())).unwrap_or_default()
    }
}

/// JS `!!d.revoked`: anything but false, null, 0, "" and a missing field is revoked ("yes", 1, [] ...). Never narrower than the JS rule.
fn truthy(v: &serde_json::Value) -> bool {
    use serde_json::Value::*;
    match v { Null => false, Bool(b) => *b, Number(n) => n.as_f64().map_or(true, |f| f != 0.0 && !f.is_nan()), String(s) => !s.is_empty(), _ => true }
}

fn same(a: &str, b: &str) -> bool {
    // constant time over equal lengths
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}
fn digest(x: &str) -> String { hex::encode(Sha256::digest(x.as_bytes())) }

pub struct Auth { root: String, devices: Vec<(String, bool)>, pairing: std::sync::OnceLock<std::sync::Arc<dyn crate::api::Pairing>> }
impl Auth {
    pub fn load(home: &Path) -> Auth {
        let root = std::fs::read_to_string(home.join("token")).unwrap_or_default().trim().to_string();
        let mut devices = Vec::new();
        if let Ok(text) = std::fs::read_to_string(home.join("devices-v1.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                for d in v["devices"].as_array().into_iter().flatten() {
                    if let Some(h) = d["hash"].as_str() { devices.push((h.to_string(), truthy(&d["revoked"]))) }
                }
            }
        }
        Auth { root, devices, pairing: Default::default() }
    }
    /// From now on the pairing registry decides which device credentials hold (devices-v1.json is no longer read).
    pub fn set_pairing(&self, p: std::sync::Arc<dyn crate::api::Pairing>) { let _ = self.pairing.set(p); }
    /// The credential a request presents: bearer, else cookie, else ?token=.
    pub fn credential(r: &Req) -> String {
        if let Some(b) = r.header("authorization").strip_prefix("Bearer ") { return b.to_string() }
        let c = Self::cookie(r); if c.is_empty() { r.param("token") } else { c }
    }
    pub fn with_root(root: &str) -> Auth { Auth { root: root.to_string(), devices: vec![], pairing: Default::default() } }
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
        if let Some(p) = self.pairing.get() { return if p.holds(&secret) { secret } else { String::new() } }
        let d = digest(&secret);
        if self.devices.iter().any(|(h, revoked)| !*revoked && same(h, &d)) { secret } else { String::new() }
    }
    /// Auth.allowed: browser rules, then a credential equal to the resolved token.
    pub fn allowed(&self, r: &Req) -> bool {
        let t = self.resolve(r);
        Self::browser(r) && !t.is_empty() && (same(r.header("authorization"), &format!("Bearer {t}")) || same(&r.param("token"), &t) || same(&Self::cookie(r), &t))
    }
    /// Auth.cache_request: GET, browser rules, and the bearer or the cookie (never ?token=) equal to the RESOLVED credential: the hub token, or a paired
    /// device's own (a phone's first step after pairing is `GET /client/capabilities` with its device token as the bearer).
    pub fn paired_request(&self, r: &Req) -> bool {
        if r.method != "GET" || !Self::browser(r) || self.root.is_empty() { return false }
        let t = self.resolve(r);
        !t.is_empty() && (same(r.header("authorization"), &format!("Bearer {t}")) || same(&Self::cookie(r), &t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn req(target: &str, hs: &[(&str, &str)]) -> Req { Req::new("GET", target, hs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()) }
    fn h(k: &str, v: &str) -> (&'static str, String) { (Box::leak(k.to_string().into_boxed_str()), v.to_string()) }
    fn with(hs: &[(&str, &str)], path: &str) -> Req { req(path, hs) }
    // laws auth_* of LAWS, stated over many paths and tokens
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

    // the registry, once installed, decides every device credential; the root token keeps working; nothing else does
    struct Held(&'static str);
    impl crate::api::Pairing for Held {
        fn holds(&self, token: &str) -> bool { token == self.0 }
        fn resolve(&self, token: &str, _c: u64, _r: &str) -> String { if token == self.0 { token.into() } else { String::new() } }
        fn active(&self, _c: u64) -> bool { true }
        fn forget(&self, _c: u64) {}
    }
    #[test] fn the_pairing_registry_decides_device_credentials_and_nothing_else_passes() {
        let dev = "0123456789abcdef0123456789abcdef"; let other = "fedcba9876543210fedcba9876543210";
        let a = Auth::with_root("roottoken"); a.set_pairing(std::sync::Arc::new(Held(dev)));
        let get = |tok: &str| req("/x", &[("authorization", &format!("Bearer {tok}"))]);
        assert!(a.allowed(&get("roottoken")), "the hub token holds");
        assert!(a.allowed(&get(dev)), "a held device credential holds");
        assert!(!a.allowed(&get(other)), "a well-formed credential nobody holds is denied");
        assert!(!a.allowed(&get("not-hex")), "a malformed credential is denied");
        assert!(!a.allowed(&req("/x", &[])), "no credential is denied");
        assert!(!a.allowed(&req("/x", &[("authorization", &format!("Bearer {dev}")), ("origin", "http://evil.example"), ("host", "h:1")])), "a foreign origin is denied");
    }

    #[test] fn a_paired_device_passes_the_paired_request_check_with_its_bearer_or_cookie_never_its_query_token() {
        let dev = "0123456789abcdef0123456789abcdef"; let other = "fedcba9876543210fedcba9876543210";
        let a = Auth::with_root("roottoken"); a.set_pairing(std::sync::Arc::new(Held(dev)));
        let bearer = |t: &str| req("/client/capabilities", &[("Authorization", &format!("Bearer {t}"))]);
        assert!(a.paired_request(&bearer(dev)), "a held device's bearer");
        assert!(a.paired_request(&req("/client/capabilities", &[("Cookie", &format!("bp_token={dev}"))])), "a held device's cookie");
        assert!(a.paired_request(&bearer("roottoken")), "the hub token still holds");
        assert!(!a.paired_request(&req(&format!("/client/capabilities?token={dev}"), &[])), "never ?token=");
        assert!(!a.paired_request(&bearer(other)), "a credential nobody holds (revoked or unknown)");
        assert!(!a.paired_request(&bearer("not-hex")) && !a.paired_request(&req("/client/capabilities", &[])), "malformed or none");
        assert!(!a.paired_request(&req("/client/capabilities", &[("Authorization", &format!("Bearer {dev}")), ("Origin", "http://evil.example"), ("Host", "h:1")])), "a foreign origin");
        let mut post = bearer(dev); post.method = "POST".into(); assert!(!a.paired_request(&post), "GET only");
    }
    #[test] fn revoked_is_read_as_javascript_truthiness_not_only_true() {
        use serde_json::json;
        for v in [json!(true), json!(1), json!("yes"), json!("false"), json!([]), json!({}), json!(-1), json!(0.5)] { assert!(truthy(&v), "{v}") }
        for v in [json!(false), json!(null), json!(0), json!(""), json!(0.0)] { assert!(!truthy(&v), "{v}") }
        // end to end through load(): a device whose revoked field is 1 or "yes" is not admitted
        let dir = crate::plat::tmp::dir("auth-rev");
        let tok = "0123456789abcdef0123456789abcdef"; let hash = hex::encode(Sha256::digest(tok.as_bytes()));
        std::fs::write(dir.join("token"), "roottoken").unwrap();
        for (revoked, admitted) in [("false", true), ("null", true), ("0", true), ("1", false), ("\"yes\"", false), ("true", false)] {
            std::fs::write(dir.join("devices-v1.json"), format!("{{\"devices\":[{{\"hash\":\"{hash}\",\"revoked\":{revoked}}}]}}")).unwrap();
            let a = Auth::load(&dir);
            assert_eq!(a.allowed(&req("/x", &[("authorization", &format!("Bearer {tok}"))])), admitted, "revoked = {revoked}");
        }
    }
    #[test] fn param_takes_the_second_segment_like_the_param() {
        let r = req("/ws?token=abc?x=1&y=2", &[]);
        assert_eq!(r.param("token"), "abc", "a second '?' ends the query");
        assert_eq!(r.param("x"), "", "nothing after the second '?' is read");
        assert_eq!(req("/ws?a=1&token=t", &[]).param("token"), "t");
        assert_eq!(req("/ws", &[]).param("token"), "");
        assert_eq!(req("?token=z", &[]).param("token"), "z");
    }
}

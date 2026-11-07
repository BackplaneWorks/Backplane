//! core/auth's owner-trust and pairing-proof rules, state for state. Each test is named for the rule it
//! checks; the laws it states are mapped to the test in scripts/law-map.tsv (`auth_*`, `cache_identity_*`).
use sha2::{Digest, Sha256};

pub type Headers<'a> = &'a [(String, String)];

pub fn header<'a>(hs: Headers<'a>, name: &str) -> &'a str {
    hs.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).unwrap_or("")
}
fn header_count(hs: Headers, name: &str) -> usize { hs.iter().filter(|(k, _)| k.eq_ignore_ascii_case(name)).count() }

/// Auth.same: constant time over equal lengths.
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 { k[..32].copy_from_slice(&Sha256::digest(key)) } else { k[..key.len()].copy_from_slice(key) }
    let (mut ipad, mut opad) = ([0x36u8; 64], [0x5cu8; 64]);
    for i in 0..64 { ipad[i] ^= k[i]; opad[i] ^= k[i] }
    let inner = Sha256::new().chain_update(ipad).chain_update(msg).finalize();
    Sha256::new().chain_update(opad).chain_update(inner).finalize().into()
}
fn hmac_hex(key: &str, msg: &str) -> String { hex::encode(hmac_sha256(key.as_bytes(), msg.as_bytes())) }

/// Auth.origin: Origin is absent (a native client) or exactly the host's own scheme://authority.
pub fn origin_ok(origin: &str, host: &str) -> bool {
    let o = origin.to_lowercase();
    origin.is_empty() || (!host.is_empty() && (o == format!("http://{}", host.to_lowercase()) || o == format!("https://{}", host.to_lowercase())))
}
/// Auth.browser: the origin rule, and Fetch Metadata never says cross-site.
pub fn browser(hs: Headers) -> bool {
    origin_ok(header(hs, "origin"), header(hs, "host")) && !header(hs, "sec-fetch-site").eq_ignore_ascii_case("cross-site")
}

fn cookie_has(hs: Headers, token: &str) -> bool {
    header(hs, "cookie").split(';').any(|c| same(c.trim(), &format!("bp_token={token}")))
}
/// H.Http.param: only the second `?` segment is the query (`/x?a?&token=T` has no token).
pub fn param(path: &str, name: &str) -> String {
    path.split('?').nth(1).unwrap_or("").split('&')
        .find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| v.to_string())).unwrap_or_default()
}
/// Auth.credential: the bearer, the ?token= or a bp_token cookie equals `token`; an empty token admits nobody.
pub fn credential(hs: Headers, path: &str, token: &str) -> bool {
    !token.is_empty() && (same(header(hs, "authorization"), &format!("Bearer {token}")) || same(&param(path, "token"), token) || cookie_has(hs, token))
}
/// Auth.allowed
pub fn allowed(hs: Headers, path: &str, token: &str) -> bool { browser(hs) && credential(hs, path, token) }
/// Auth.cache_request: GET only by construction; the bearer or the cookie, never ?token=.
pub fn cache_request(method: &str, hs: Headers, token: &str) -> bool {
    method == "GET" && browser(hs) && !token.is_empty() && (same(header(hs, "authorization"), &format!("Bearer {token}")) || cookie_has(hs, token))
}
/// Auth.pair: pairing proves the shared secret in the bearer; it never inherits a localhost exemption.
pub fn pair(hs: Headers, token: &str) -> bool {
    browser(hs) && !token.is_empty() && same(header(hs, "authorization"), &format!("Bearer {token}"))
}

/// Auth.owner.hosts: the listener's own authorities, from its tailnet address and Self.DNSName only.
pub fn owner_hosts(ip: &str, dns: &str, port: &str) -> Vec<String> {
    [ip, dns].iter().filter(|n| !n.is_empty()).flat_map(|n| [n.to_lowercase(), format!("{}:{port}", n.to_lowercase())]).collect()
}
fn owner_host(own: &[String], host: &str) -> bool { own.iter().any(|x| !x.is_empty() && x == host) }
/// Auth.owner.peer: the same identity on a linked machine's signed request; it vouches the link is the
/// owner's own machine and grants nothing else (the signature is still checked).
pub fn owner_peer(trusted: bool, own: &[String], hs: Headers) -> bool {
    trusted && header_count(hs, "host") == 1 && header(hs, "origin").is_empty() && header(hs, "tailscale-user-login").is_empty()
        && owner_host(own, &header(hs, "host").to_lowercase())
}

/// Auth.local.host: a Host naming this machine.
pub fn local_host(host: &str) -> bool {
    ["localhost", "127.0.0.1", "[::1]"].iter().any(|h| host == *h || host.starts_with(&format!("{h}:")))
}
/// Auth.trusted: a loopback socket, not proxied by tailscale serve. Never a credential by itself.
pub fn trusted(socket_trusted: bool, hs: Headers) -> bool {
    socket_trusted && header(hs, "tailscale-user-login").is_empty()
        && (local_host(&header(hs, "host").to_lowercase()) || (header(hs, "host").is_empty() && header(hs, "origin").is_empty()))
}
/// Auth.mcp.key: one thread's tools open with the token's HMAC of the segment, not the token.
pub fn mcp_key(token: &str, seg: &str) -> String { hmac_hex(token, &format!("mcp\n{seg}")) }
pub fn mcp_keyed(path: &str, token: &str, seg: &str) -> bool { !token.is_empty() && same(&param(path, "k"), &mcp_key(token, seg)) }
/// Auth.cache_key: readable cache identity, a separate HMAC domain; "" for an empty token.
pub fn cache_key(token: &str) -> String { if token.is_empty() { String::new() } else { hmac_hex(token, "ui-cache\nv1") } }
/// Auth.cache_cookie: "" for an empty token (law cache_identity_empty_cookie).
pub fn cache_cookie(token: &str, https: bool) -> String {
    if token.is_empty() { return String::new() }
    format!("Set-Cookie: bp_cache_v1={}; Path=/; SameSite=Strict; Max-Age=31536000{}\r\n", cache_key(token), if https { "; Secure" } else { "" })
}
/// Auth.cookie ++ Auth.cache_cookie (Auth.pair.cookies)
pub fn pair_cookies(token: &str, https: bool) -> String {
    format!("Set-Cookie: bp_token={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000{}\r\n", if https { "; Secure" } else { "" }) + &cache_cookie(token, https)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hs(v: &[(&str, &str)]) -> Vec<(String, String)> { v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() }
    const OWN: [&str; 2] = ["100.1.2.3", "100.1.2.3:3787"];
    fn own() -> Vec<String> { OWN.iter().map(|s| s.to_string()).collect() }

    #[test] fn hmac_matches_rfc4231() {
        // test case 2
        assert_eq!(hex::encode(hmac_sha256(b"Jefe", b"what do ya want for nothing?")), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        // key longer than the block (case 6)
        assert_eq!(hex::encode(hmac_sha256(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First")), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
    }
    #[test] fn owner_peer_grants_only_the_vouch() {
        let ok = hs(&[("Host", "100.1.2.3")]);
        assert!(owner_peer(true, &own(), &ok));
        assert!(!owner_peer(false, &own(), &ok));
        assert!(!owner_peer(true, &own(), &hs(&[("Host", "100.1.2.3"), ("Origin", "x")])));
        assert!(!owner_peer(true, &own(), &hs(&[("Host", "100.1.2.3"), ("Tailscale-User-Login", "x")])));
        assert!(!owner_peer(true, &own(), &hs(&[])));
    }
    #[test] fn owner_hosts_come_from_the_listener() {
        assert_eq!(owner_hosts("100.1.2.3", "M.tail.ts.net", "3787"), vec!["100.1.2.3", "100.1.2.3:3787", "m.tail.ts.net", "m.tail.ts.net:3787"]);
        assert!(owner_hosts("", "", "3787").is_empty());
    }
    #[test] fn pair_requires_the_bearer_and_ignores_loopback() {
        assert!(pair(&hs(&[("Authorization", "Bearer t"), ("Host", "127.0.0.1:1")]), "t"));
        assert!(!pair(&hs(&[("Host", "127.0.0.1:1")]), "t"));
        assert!(!pair(&hs(&[("Authorization", "Bearer t-extra")]), "t"));
        assert!(!pair(&hs(&[("Authorization", "Bearer t")]), ""));
        assert!(!pair(&hs(&[("Authorization", "Bearer t"), ("Origin", "http://evil")]), "t"));
    }
    #[test] fn trusted_is_loopback_and_unproxied() {
        assert!(trusted(true, &hs(&[("Host", "localhost:3787")])));
        assert!(trusted(true, &hs(&[])));
        assert!(!trusted(true, &hs(&[("Host", "localhost"), ("Tailscale-User-Login", "a")])));
        assert!(!trusted(true, &hs(&[("Host", "evil.example")])));
        assert!(!trusted(true, &hs(&[("Origin", "http://x")])));
        assert!(!trusted(false, &hs(&[("Host", "localhost")])));
    }
    #[test] fn mcp_key_opens_one_segment() {
        let k = mcp_key("tok", "t1.3");
        assert!(mcp_keyed(&format!("/mcp/t1.3?k={k}"), "tok", "t1.3"));
        assert!(!mcp_keyed(&format!("/mcp/t2.3?k={k}"), "tok", "t2.3"));
        assert!(!mcp_keyed(&format!("/mcp/t1.3?k={k}"), "", "t1.3"));
        assert!(!mcp_keyed("/mcp/t1.3", "tok", "t1.3"));
    }
    #[test] fn cache_key_is_not_the_token() {
        assert_eq!(cache_key(""), ""); assert_eq!(cache_cookie("", true), "", "cache_identity_empty_cookie");
        assert_ne!(cache_key("tok"), "tok");
        assert_eq!(cache_key("tok"), hmac_hex("tok", "ui-cache\nv1"));
        assert!(pair_cookies("", false).contains("bp_token=;") && !pair_cookies("", false).contains("bp_cache_v1"));
        assert!(pair_cookies("abc", true).matches("; Secure").count() == 2);
    }
    #[test] fn cache_request_takes_no_query_token() {
        assert!(!cache_request("GET", &hs(&[]), "t"));
        assert!(cache_request("GET", &hs(&[("Cookie", "a=b; bp_token=t")]), "t"));
        assert!(!cache_request("GET", &hs(&[("Cookie", "bp_token=t-more")]), "t"));
        assert!(!cache_request("POST", &hs(&[("Authorization", "Bearer t")]), "t"), "cache_identity_post_refused");
        assert!(!cache_request("GET", &hs(&[("Authorization", "Bearer t")]), ""), "an empty token admits nobody");
    }
    #[test] fn param_is_the_second_question_mark_segment() {
        assert_eq!(param("/x?a=1&token=T", "token"), "T");
        assert_eq!(param("/x?a?&token=T", "token"), "", "splits on every ?, so a third segment is not read");
        assert_eq!(param("/x", "token"), "");
    }
    #[test] fn origin_rules() {
        assert!(origin_ok("", "h")); assert!(origin_ok("HTTP://H:1", "h:1")); assert!(!origin_ok("http://h:1/x", "h:1")); assert!(!origin_ok("http://h", ""));
    }
}

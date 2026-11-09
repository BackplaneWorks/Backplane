//! OAuth 2.0 for installed apps (RFC 6749, RFC 7636 PKCE, RFC 8252 loopback redirect), as 0.14's core/google.bend: pure. The consent page's URL, the
//! code and refresh bodies, the token answer, the state that names the hub a sign-in started on (the tailnet hop), and which redirects a sign-in may begin with.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64U, Engine};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// read, draft, send and label mail; read and write calendar events; who signed in (Google.scopes)
pub const SCOPES: [&str; 4] = ["openid", "email", "https://www.googleapis.com/auth/gmail.modify", "https://www.googleapis.com/auth/calendar.events"];
pub const AUTH_BASE: &str = "https://accounts.google.com/o/oauth2/v2/auth";

/// Url.encode: every byte outside the unreserved set as %XX
pub fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() { if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { o.push(b as char) } else { o.push_str(&format!("%{b:02X}")) } }
    o
}
/// Url.decode: %XX and '+' (a space, as forms write it); bytes read as UTF-8
pub fn dec(s: &str) -> String {
    let b = s.as_bytes(); let mut out = Vec::with_capacity(b.len()); let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => { out.push(b' '); i += 1 }
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) { Ok(x) => { out.push(x); i += 3 } Err(_) => { out.push(b'%'); i += 1 } }
            }
            c => { out.push(c); i += 1 }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
pub fn pair(k: &str, v: &str) -> String { format!("{}={}", k, enc(v)) }
/// Url.query: after the first '?', before '#'; the text itself when it has no '?'
pub fn query(s: &str) -> &str { let q = s.split_once('?').map_or(s, |(_, q)| q); q.split('#').next().unwrap_or("") }
/// Url.param: a parameter of a query string, decoded ("" when absent)
pub fn param(q: &str, name: &str) -> String { q.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == name).map(|(_, v)| dec(v))).unwrap_or_default() }

/// RFC 7636: the S256 challenge of a verifier
pub fn challenge(verifier: &str) -> String { B64U.encode(Sha256::digest(verifier.as_bytes())) }
/// random text from n bytes, base64url (RFC 7636's unreserved characters)
pub fn nonce(n: usize) -> Result<String, String> { let mut b = vec![0u8; n]; crate::plat::fs::random(&mut b).map_err(|_| "no secure random source".to_string())?; Ok(B64U.encode(b)) }

/// Google.auth_url: the page the person opens to allow access (a refresh token comes back because of access_type=offline and prompt=consent)
pub fn auth_url(base: &str, client_id: &str, redirect: &str, challenge: &str, state: &str, hint: &str) -> String {
    let mut ps = vec![pair("client_id", client_id), pair("redirect_uri", redirect), "response_type=code".into(), pair("scope", &SCOPES.join(" ")),
        pair("code_challenge", challenge), "code_challenge_method=S256".into(), pair("state", state), "access_type=offline".into(), "prompt=select_account%20consent".into()];
    if !hint.is_empty() { ps.push(pair("login_hint", hint)) }
    format!("{base}?{}", ps.join("&"))
}
/// Google.code_body
pub fn code_body(id: &str, secret: &str, code: &str, verifier: &str, redirect: &str) -> String {
    [ "grant_type=authorization_code".to_string(), pair("code", code), pair("client_id", id), pair("client_secret", secret), pair("redirect_uri", redirect), pair("code_verifier", verifier)].join("&")
}
/// Google.refresh_body
pub fn refresh_body(id: &str, secret: &str, refresh: &str) -> String { ["grant_type=refresh_token".to_string(), pair("refresh_token", refresh), pair("client_id", id), pair("client_secret", secret)].join("&") }

/// What a token endpoint answered: the access token, its expiry (epoch seconds), the refresh token and email when given, the scopes granted.
#[derive(Debug, Clone, PartialEq)]
pub struct Tok { pub access: String, pub expiry: u64, pub refresh: String, pub email: String, pub scopes: Vec<String> }
/// Google.jwt_email: the email claim of an id_token (no signature check: it came straight from Google over TLS)
pub fn jwt_email(t: &str) -> String {
    let Some(p) = t.split('.').nth(1) else { return String::new() };
    let raw = B64U.decode(p.trim_end_matches('=')).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
    raw["email"].as_str().unwrap_or("").to_string()
}
/// Google.tokens: the tokens of a token endpoint's answer, or None
pub fn tokens(text: &str, now: u64) -> Option<Tok> {
    let j: Value = serde_json::from_str(text).ok()?;
    let access = j["access_token"].as_str().filter(|s| !s.is_empty())?.to_string();
    let exp = j["expires_in"].as_u64().or_else(|| j["expires_in"].as_str().and_then(|s| s.parse().ok())).unwrap_or(3600);
    Some(Tok { access, expiry: now + exp, refresh: j["refresh_token"].as_str().unwrap_or("").into(), email: jwt_email(j["id_token"].as_str().unwrap_or("")),
        scopes: j["scope"].as_str().unwrap_or("").split(' ').filter(|s| !s.is_empty()).map(str::to_string).collect() })
}
/// Google.fresh: an access token is good for a request when it lives a minute more
pub fn fresh(now: u64, expiry: u64) -> bool { now + 60 < expiry }
/// Google.api_error: the message of an OAuth or API error answer (its first 300 characters when it is not one)
pub fn api_error(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(j) => match &j["error"] {
            Value::String(s) => { let d = j["error_description"].as_str().unwrap_or(""); if d.is_empty() { s.clone() } else { format!("{s}: {d}") } }
            Value::Object(o) => o.get("message").and_then(Value::as_str).filter(|m| !m.is_empty()).map(str::to_string).unwrap_or_else(|| text.trim().chars().take(300).collect()),
            _ => text.trim().chars().take(300).collect(),
        },
        Err(_) => text.trim().chars().take(300).collect(),
    }
}
/// The refresh token no longer works: revoked, expired (an app in testing has 7-day refresh tokens), or the password changed. The account is then "sign-in expired".
pub fn dead_grant(code: &str, body: &str) -> bool {
    let e = serde_json::from_str::<Value>(body).ok().and_then(|j| j["error"].as_str().map(str::to_string)).unwrap_or_default();
    (code == "400" || code == "401") && (e == "invalid_grant" || e == "unauthorized_client" || e == "invalid_client")
}

/// What a redirect carried; `bare` when the person pasted only the code
#[derive(Debug, PartialEq)]
pub struct CodeState { pub code: String, pub state: String, pub bare: bool }
/// Google.code_of: the code and state of a pasted redirect: the whole URL, its query, or only the code (text without '?', '=' or '&')
pub fn code_of(s: &str) -> Option<CodeState> {
    let t = s.trim();
    if t.contains('?') || t.contains('=') || t.contains('&') {
        let q = query(t); let code = param(q, "code");
        if code.is_empty() { None } else { Some(CodeState { code, state: param(q, "state"), bare: false }) }
    } else if t.is_empty() { None } else { Some(CodeState { code: dec(t), state: String::new(), bare: true }) }
}
/// Google.error_of: the error a redirect carried (access_denied when the person said no), or ""
pub fn error_of(s: &str) -> String { param(query(s.trim()), "error") }

// ---- which hub started a sign-in (the tailnet hop) ----
// Google sends the browser back to http://127.0.0.1:<port>, so a sign-in started on another machine's hub lands on the hub of the machine the browser runs on.
// The state names the hub that started it (its tailnet address, base64url, after a '.'; the nonce is base64url, which has no '.'), and a hub whose sign-in it is
// not sends the browser there, once.

/// Google.state
pub fn state(nonce: &str, home: &str) -> String { if home.is_empty() { nonce.into() } else { format!("{nonce}.{}", B64U.encode(home.as_bytes())) } }
/// Google.state.home: the hub a state names ("" for none)
pub fn state_home(s: &str) -> String { s.split_once('.').and_then(|(_, b)| B64U.decode(b.trim_end_matches('=')).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default() }
fn label_ok(h: &str) -> bool { !h.is_empty() && h.len() <= 63 && h.bytes().all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == b'-') && !h.starts_with('-') && !h.ends_with('-') }
/// Google.hop.ok: a hub the browser may be sent to: a machine on a tailnet (http(s)://<name>.ts.net[:port], nothing after). A forwarded state is untrusted input
/// until the destination checks PKCE: only DNS labels and an optional decimal port, so neither the decoded origin nor the query can add a header.
pub fn hop_ok(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or("");
    let mut parts = rest.split(':');
    let (host, port) = (parts.next().unwrap_or(""), parts.next());
    if parts.next().is_some() { return false }
    let h = host.to_lowercase();
    let host_ok = h.ends_with(".ts.net") && h.split('.').all(label_ok) && h == host;
    let port_ok = port.map_or(true, |p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u32>().map_or(false, |n| n > 0 && n <= 65535));
    host_ok && port_ok
}
fn query_safe(q: &str) -> bool { q.bytes().all(|x| (33..=126).contains(&x) && x != b'#') }
/// Google.hop: where to send the browser Google sent back (`q` the redirect's query) when no sign-in here is the one it names: the hub its state names, on the
/// tailnet, once (hop=1); None finishes here
pub fn hop(q: &str, mine: bool) -> Option<String> {
    let home = state_home(&param(q, "state"));
    (query_safe(q) && !mine && param(q, "hop") != "1" && hop_ok(&home)).then(|| format!("{home}/oauth/google?{q}&hop=1"))
}
/// Google.loopback: a redirect a client listens on itself (RFC 8252 7.3): exactly http://127.0.0.1:<1-5 digits>/oauth/google
pub fn loopback(url: &str) -> bool {
    url.strip_prefix("http://127.0.0.1:").and_then(|r| r.strip_suffix("/oauth/google")).map_or(false, |p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u32>().map_or(false, |n| n > 0 && n <= 65535))
}

/// The scopes as a person reads them: "Gmail, Calendar" (what the token was granted; a sign-in that unticked one shows without it)
pub fn scope_words(scopes: &[String]) -> String {
    let has = |s: &str| scopes.iter().any(|x| x == s || x.starts_with(&format!("{s}.")) || x == &format!("https://www.googleapis.com/auth/{s}"));
    let mut v = vec![];
    if scopes.iter().any(|x| x.contains("auth/gmail")) || has("gmail") { v.push("Gmail") }
    if scopes.iter().any(|x| x.contains("auth/calendar")) { v.push("Calendar") }
    if v.is_empty() { "no Gmail or Calendar access".into() } else { v.join(", ") }
}
pub fn has_gmail(scopes: &[String]) -> bool { scopes.iter().any(|x| x.contains("auth/gmail") || x.contains("mail.google.com")) }
pub fn has_calendar(scopes: &[String]) -> bool { scopes.iter().any(|x| x.contains("auth/calendar")) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn urls_encode_and_decode_as_rfc_3986_and_forms() {
        assert_eq!(enc("a b/é~"), "a%20b%2F%C3%A9~"); assert_eq!(dec("a%20b+c%2F%C3%A9"), "a b c/é"); assert_eq!(dec("100%"), "100%"); assert_eq!(dec("%zz"), "%zz");
        assert_eq!(param("code=4%2F0A&state=x.y", "code"), "4/0A"); assert_eq!(param("a=1", "b"), ""); assert_eq!(query("http://h/p?x=1#f"), "x=1");
    }
    #[test] fn the_pkce_challenge_is_rfc_7636s_example() { assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"); }
    #[test] fn the_consent_url_asks_for_offline_access_with_pkce() {
        let u = auth_url(AUTH_BASE, "cid", "http://127.0.0.1:3799/oauth/google", "CH", "st", "");
        for p in ["client_id=cid", "redirect_uri=http%3A%2F%2F127.0.0.1%3A3799%2Foauth%2Fgoogle", "code_challenge=CH", "code_challenge_method=S256", "state=st", "access_type=offline", "prompt=select_account%20consent", "scope=openid%20email%20https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fgmail.modify%20https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcalendar.events"] { assert!(u.contains(p), "{p} in {u}") }
        assert!(auth_url(AUTH_BASE, "c", "r", "x", "s", "a@b.c").ends_with("&login_hint=a%40b.c"));
    }
    #[test] fn token_answers_give_the_tokens_the_email_and_the_scopes() {
        let idt = format!("h.{}.s", B64U.encode(br#"{"email":"a@b.com"}"#));
        let t = tokens(&format!(r#"{{"access_token":"ya29","expires_in":3599,"refresh_token":"1//r","id_token":"{idt}","scope":"openid https://www.googleapis.com/auth/gmail.modify"}}"#), 100).unwrap();
        assert_eq!((t.access.as_str(), t.expiry, t.refresh.as_str(), t.email.as_str()), ("ya29", 3699, "1//r", "a@b.com")); assert_eq!(scope_words(&t.scopes), "Gmail");
        assert!(tokens(r#"{"error":"invalid_grant"}"#, 0).is_none()); assert!(tokens("nope", 0).is_none());
        assert!(fresh(100, 161) && !fresh(100, 160));
    }
    #[test] fn errors_read_as_google_says_them() {
        assert_eq!(api_error(r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#), "invalid_grant: Token has been expired or revoked.");
        assert_eq!(api_error(r#"{"error":{"code":403,"message":"Insufficient Permission"}}"#), "Insufficient Permission"); assert_eq!(api_error("<html>"), "<html>");
        assert!(dead_grant("400", r#"{"error":"invalid_grant"}"#) && !dead_grant("500", r#"{"error":"invalid_grant"}"#) && !dead_grant("400", r#"{"error":"invalid_request"}"#));
    }
    #[test] fn a_pasted_redirect_gives_its_code_and_state() {
        assert_eq!(code_of("http://127.0.0.1:3799/oauth/google?state=n.x&code=4%2F0Ab&scope=email"), Some(CodeState { code: "4/0Ab".into(), state: "n.x".into(), bare: false }));
        assert_eq!(code_of(" 4/0Ab "), Some(CodeState { code: "4/0Ab".into(), state: "".into(), bare: true }));
        assert_eq!(code_of("http://127.0.0.1:1/oauth/google?error=access_denied"), None); assert_eq!(error_of("http://x/?error=access_denied&state=s"), "access_denied"); assert_eq!(code_of(""), None);
    }
    #[test] fn a_state_names_its_hub_and_a_hop_goes_only_to_the_tailnet_once() {
        let s = state("abc", "https://desk.tail1.ts.net:3790"); assert_eq!(state_home(&s), "https://desk.tail1.ts.net:3790"); assert_eq!(state_home("abc"), "");
        let q = format!("state={s}&code=c");
        assert_eq!(hop(&q, false).unwrap(), format!("https://desk.tail1.ts.net:3790/oauth/google?{q}&hop=1"));
        assert_eq!(hop(&q, true), None, "the sign-in is this hub's"); assert_eq!(hop(&format!("{q}&hop=1"), false), None, "only once");
        for bad in ["https://evil.com", "https://a.ts.net/x", "https://a.ts.net:99999", "https://A.ts.net", "https://a.ts.net:1:2", "https://a.ts.net\r\nX: y", "https://-a.ts.net", "javascript:alert(1)//.ts.net", "https://a@b.ts.net"] {
            assert!(!hop_ok(bad), "{bad}"); assert_eq!(hop(&format!("state={}&code=c", state("n", bad)), false), None, "{bad}");
        }
        assert!(hop_ok("http://box.tail1.ts.net") && hop_ok("https://box.tail1.ts.net:443"));
        assert_eq!(hop(&format!("state={s}&code=c d"), false), None, "a query with a space or control is not forwarded");
    }
    #[test] fn only_a_loopback_redirect_of_a_client_is_taken() {
        assert!(loopback("http://127.0.0.1:49152/oauth/google"));
        for bad in ["http://127.0.0.1:/oauth/google", "http://127.0.0.1:0/oauth/google", "http://127.0.0.1:123456/oauth/google", "http://127.0.0.1:80/oauth/google/x", "https://127.0.0.1:80/oauth/google", "http://localhost:80/oauth/google", "http://127.0.0.1:80@evil/oauth/google"] { assert!(!loopback(bad), "{bad}") }
    }
    #[test] fn scopes_read_as_words() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(scope_words(&s(&SCOPES)), "Gmail, Calendar"); assert_eq!(scope_words(&s(&["openid", "https://www.googleapis.com/auth/calendar.events"])), "Calendar"); assert_eq!(scope_words(&s(&["email"])), "no Gmail or Calendar access");
    }
}

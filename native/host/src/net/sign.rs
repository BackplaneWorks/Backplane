//! core/sign.bend: signed requests between hubs, the replay cache and peer invites.
//!
//!   X-Backplane-Timestamp: <unix seconds>
//!   X-Backplane-Signature: sha256=<hex HMAC-SHA256(secret, ts + "." + body)>
//!
//! A secret is 32 random bytes handed out as 64 hex digits; it keys our own signatures as those 32 bytes (a
//! secret that is not hex keys as its text). Laws `sign_*`, `seen_*`, `invite_*`.
use super::owner::{hmac_sha256, same};

/// How far a timestamp may sit from the hub's clock, in seconds.
pub const WINDOW: u64 = 300;

/// Hex.decode: either case; None on an odd length or a character that is not a digit.
fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 { return None }
    s.as_bytes().chunks(2).map(|p| Some((val(p[0])? << 4) | val(p[1])?)).collect()
}
fn val(c: u8) -> Option<u8> { match c { b'0'..=b'9' => Some(c - b'0'), b'a'..=b'f' => Some(c - b'a' + 10), b'A'..=b'F' => Some(c - b'A' + 10), _ => None } }

/// The secret's bytes: its hex, or its text when it is not hex.
fn key(secret_hex: &str) -> Vec<u8> { unhex(secret_hex).unwrap_or_else(|| secret_hex.as_bytes().to_vec()) }

/// The signature header's value for a body sent at `ts`.
pub fn header(secret_hex: &str, ts: u64, body: &str) -> String {
    format!("sha256={}", hex::encode(hmac_sha256(&key(secret_hex), format!("{ts}.{body}").as_bytes())))
}
/// |now - ts| <= window
pub fn fresh(now: u64, ts: u64) -> bool { now.abs_diff(ts) <= WINDOW }
/// A request is genuine when it is fresh and its signature matches, compared in constant time.
pub fn check(secret_hex: &str, now: u64, ts: u64, body: &str, sig: &str) -> bool {
    fresh(now, ts) && same(sig, &header(secret_hex, ts, body))
}
/// A webhook's bearer token against the one kept; a hook with no token lets no bearer in.
pub fn bearer(kept: &str, given: &str) -> bool { !kept.is_empty() && same(given, kept) }

/// Signatures (or delivery ids) accepted within the window; a key is accepted once.
#[derive(Default)]
pub struct Seen { items: Vec<(String, u64)> }
impl Seen {
    pub fn has(&self, key: &str) -> bool { self.items.iter().any(|(k, _)| k == key) }
    /// Remember `key` at `now`, forgetting what has left the window.
    pub fn add(&mut self, key: &str, now: u64) {
        self.items.retain(|(_, at)| fresh(now, *at));
        self.items.insert(0, (key.to_string(), now));
    }
    /// Bn.accept: false when the key was seen within the window (a replay), else it is now held.
    pub fn accept(&mut self, key: &str, now: u64) -> bool { if self.has(key) { false } else { self.add(key, now); true } }
}

/// bp1:<url>:<peer id>:<secret hex>; the url has colons of its own, so the id and secret are the last two fields.
#[derive(Debug, PartialEq, Clone)]
pub struct Inv { pub url: String, pub id: String, pub secret: String }
pub fn invite_make(url: &str, id: &str, secret: &str) -> String { format!("bp1:{url}:{id}:{secret}") }
/// An invite as pasted (surrounding space ignored); None when malformed.
pub fn invite_parse(s: &str) -> Option<Inv> {
    let t = s.trim();
    let rest = t.strip_prefix("bp1:")?;
    let mut parts: Vec<&str> = rest.split(':').collect();
    if parts.len() < 3 { return None }
    let secret = parts.pop()?; let id = parts.pop()?; let url = parts.join(":");
    if url.is_empty() || id.is_empty() || secret.len() != 64 || unhex(secret).is_none() { return None }
    Some(Inv { url, id: id.into(), secret: secret.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    const SEC: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    #[test] fn sign_header_is_hmac_of_ts_dot_body() {
        // independent value: HMAC-SHA256 with the 32 key bytes over "1700000000.{}" (python hmac)
        assert_eq!(header(SEC, 1700000000, "{}"), "sha256=e9d5a2b30cca00fa73391ec4e7206c79ea1670086dd395c0a7508a03de9a0dfd");
    }
    #[test] fn sign_stale_rejected() {
        let sig = header(SEC, 1000, "b");
        assert!(check(SEC, 1000, 1000, "b", &sig));
        assert!(check(SEC, 1300, 1000, "b", &sig) && check(SEC, 700, 1000, "b", &sig));
        assert!(!check(SEC, 1301, 1000, "b", &sig) && !check(SEC, 699, 1000, "b", &sig), "outside the window, whatever the signature");
        assert!(!check(SEC, 1000, 1000, "other", &sig) && !check(SEC, 1000, 1000, "b", "sha256=00"));
        assert!(!check("ff".repeat(32).as_str(), 1000, 1000, "b", &sig), "another secret");
    }
    #[test] fn sign_window_300() { assert_eq!(WINDOW, 300); }
    #[test] fn sign_a_text_secret_keys_as_its_text() {
        assert_ne!(header("not hex!", 1, "b"), header(SEC, 1, "b"));
        assert_eq!(header("not hex!", 1, "b"), format!("sha256={}", hex::encode(hmac_sha256(b"not hex!", b"1.b"))));
        assert_eq!(header("ABab", 1, "b"), header("abab", 1, "b"), "either case decodes alike");
    }
    #[test] fn seen_add_has() {
        let mut s = Seen::default();
        assert!(s.accept("k", 100) && !s.accept("k", 150), "accepted once");
        s.add("old", 100); s.add("new", 1000);
        assert!(!s.has("old") && s.has("new") && s.has("k") == false, "what left the window is forgotten");
    }
    #[test] fn invite_roundtrip() {
        let i = invite_make("http://host:3787", "p1", SEC);
        assert_eq!(invite_parse(&i), Some(Inv { url: "http://host:3787".into(), id: "p1".into(), secret: SEC.into() }));
        assert_eq!(invite_parse(&format!("  {i}\n")).unwrap().id, "p1");
        for bad in ["", "bp1:", "bp2:a:b:c", "bp1:http://h:p1:zz", &format!("bp1::p1:{SEC}"), &format!("bp1:http://h::{SEC}"), &format!("bp1:http://h:p1:{}", &SEC[..62])] {
            assert_eq!(invite_parse(bad), None, "{bad}");
        }
    }
}

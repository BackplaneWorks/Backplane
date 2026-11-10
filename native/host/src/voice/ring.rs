//! A Pebble Index ring's webhook (pure; 0.14's core/ring, docs/bots.md, docs/ring.md): its signature, and a form that is not the ring's as JSON for a bot. A multipart form with `transcription` (the ring's own words), `recordedAt`, `client`, `test` and maybe
//! `audio` (M4A). A part's value is the text between the blank line after its headers and the next boundary; the first part of a name wins, so what Backplane heard goes first.
use crate::net::{owner, sign};
use serde_json::{json, Map, Value};

/// A form may carry a recording: 2 MB, a few minutes of it. Anything else stays at a hook's 256 KB.
pub const FORM_MAX: usize = 2 << 20;
pub const BODY_MAX: usize = 256 << 10;

pub fn is_form(ctype: &str) -> bool { ctype.trim().starts_with("multipart/form-data") }
pub fn has_audio(body: &str) -> bool { body.contains("name=\"audio\"") }
/// A part's text ("" when the body has no such part).
pub fn part<'a>(body: &'a str, name: &str) -> &'a str {
    let Some((_, r)) = body.split_once(&format!("name=\"{name}\"")) else { return "" };
    let Some((_, v)) = r.split_once("\r\n\r\n") else { return "" };
    v.split("\r\n--").next().unwrap_or("")
}
pub fn text(body: &str) -> &str { part(body, "transcription").trim() }
pub fn unsure(body: &str) -> &str { part(body, "unsure").trim() }
pub fn test(body: &str) -> bool { part(body, "test").trim() == "true" }
/// The body under a bearer token: its text fields as a JSON object for the bot, which reads it as untrusted data like any webhook's. The recording stays out: a bot cannot hear M4A bytes.
pub fn form_json(body: &str, size: &str) -> String {
    let mut o = Map::new(); let client = part(body, "client").trim();
    o.insert("source".into(), json!(if client == "ring" { "pebble-index-01" } else { "form-data" }));
    let mut kv = |k: &str, v: &str| { if !v.is_empty() { o.insert(k.into(), json!(v)); } };
    kv("transcription", text(body));
    let at = part(body, "recordedAt").trim().to_string(); let num = at.parse::<u64>().ok();
    if num.is_none() { kv("recordedAt", &at) }
    kv("client", client); kv("test", if test(body) { "true" } else { "" }); kv("heardBy", part(body, "heard").trim()); kv("unsure", unsure(body));
    let size = size.trim(); let audio = if !has_audio(body) { String::new() } else if size.is_empty() { "omitted".to_string() } else { format!("omitted, {size} bytes") }; kv("audio", &audio);
    if let Some(n) = num { o.insert("recordedAt".into(), json!(n)); }
    Value::Object(o).to_string()
}
/// The Index protocol, version 1: X-Index-Signature is the lowercase hex HMAC-SHA256, keyed with the secret's text as entered, of "v1\n<ts>\n<delivery>\n<trigger>\n<0|1>\n" and the
/// raw body. Stale timestamps fail. The delivery id is the replay key, tagged ":i:" so the hub knows the ring; "!sig" when it does not prove itself.
pub fn index_key(secret: &str, hid: &str, now: u64, h: &dyn Fn(&str) -> String, raw: &[u8]) -> String {
    let (dl, tr, sig) = (h("x-index-delivery"), h("x-index-trigger"), h("x-index-signature"));
    let ts = h("x-index-timestamp").parse::<u64>();
    let Ok(ts) = ts else { return "!sig".into() };
    if secret.is_empty() || h("x-index-webhook-version") != "1" || dl.is_empty() || !sign::fresh(now, ts) { return "!sig".into() }
    let mut m = format!("v1\n{ts}\n{dl}\n{tr}\n{}\n", if h("x-index-test") == "true" { "1" } else { "0" }).into_bytes(); m.extend_from_slice(raw);
    if owner::same(&sig, &hex::encode(owner::hmac_sha256(secret.as_bytes(), &m))) { format!("hook:{hid}:i:{dl}") } else { "!sig".into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form(parts: &[(&str, &str)]) -> String { let mut b = String::new(); for (k, v) in parts { b.push_str(&format!("--x\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")); } b.push_str("--x--\r\n"); b }
    #[test] fn parts_of_a_form() {
        let b = form(&[("transcription", " Buy milk "), ("recordedAt", "1700000000000"), ("client", "ring"), ("test", "true")]);
        assert_eq!(text(&b), "Buy milk"); assert!(test(&b)); assert_eq!(part(&b, "nope"), ""); assert!(is_form(" multipart/form-data; boundary=x")); assert!(!is_form("application/json"));
    }
    #[test] fn a_form_under_a_token_becomes_json_without_the_recording() {
        let b = form(&[("transcription", "Buy milk"), ("recordedAt", "1700000000000"), ("client", "ring"), ("audio", "M4A")]);
        let j: Value = serde_json::from_str(&form_json(&b, "2048")).unwrap();
        assert_eq!(j, json!({"source":"pebble-index-01","transcription":"Buy milk","recordedAt":1700000000000u64,"client":"ring","audio":"omitted, 2048 bytes"}));
        let j: Value = serde_json::from_str(&form_json(&form(&[("recordedAt", "soon"), ("audio", "x")]), "")).unwrap(); assert_eq!(j, json!({"source":"form-data","recordedAt":"soon","audio":"omitted"}));
    }
    #[test] fn the_index_signature_is_checked_over_the_bytes_and_fresh() {
        let raw = b"--x\r\n\xff\xfe\x00binary"; let (ts, secret) = (1_700_000_000u64, "the secret as pasted");
        let mut m = format!("v1\n{ts}\nd-1\nhold\n0\n").into_bytes(); m.extend_from_slice(raw); let sig = hex::encode(owner::hmac_sha256(secret.as_bytes(), &m));
        let h = |sig: String, v: &'static str| move |n: &str| match n { "x-index-webhook-version" => v.to_string(), "x-index-timestamp" => ts.to_string(), "x-index-delivery" => "d-1".to_string(), "x-index-trigger" => "hold".to_string(), "x-index-signature" => sig.clone(), _ => String::new() };
        assert_eq!(index_key(secret, "h1", ts + 5, &h(sig.clone(), "1"), raw), "hook:h1:i:d-1");
        assert_eq!(index_key(secret, "h1", ts + 5, &h("00".into(), "1"), raw), "!sig"); assert_eq!(index_key("other", "h1", ts + 5, &h(sig.clone(), "1"), raw), "!sig");
        assert_eq!(index_key(secret, "h1", ts + 1000, &h(sig.clone(), "1"), raw), "!sig", "stale"); assert_eq!(index_key(secret, "h1", ts, &h(sig.clone(), "2"), raw), "!sig", "another version"); assert_eq!(index_key("", "h1", ts, &h(sig, "1"), raw), "!sig");
    }
}

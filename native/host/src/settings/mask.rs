//! Sec.mask / Sec.json / Sec.clean (core/secret.bend, laws sec_*): every character inside an occurrence of a stored
//! value becomes U+2022, the length kept. The values live in one process-wide set the vault and `api.env` feed; only
//! the masks below read it, and nothing here returns a value.
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex, RwLock};

const BULLET: char = '\u{2022}';
/// Values shorter than this would mask ordinary text (vault.js bp_vault_all).
pub const MIN: usize = 8;

static VALUES: RwLock<Option<Arc<Vec<String>>>> = RwLock::new(None);
static API: Mutex<Option<ApiCache>> = Mutex::new(None);
struct ApiCache { at: std::time::Instant, mtime: Option<std::time::SystemTime>, vals: Vec<String>, path: std::path::PathBuf }

/// The vault's open values (called by the vault after every change).
pub fn set_vault_values(v: Vec<String>) { let mut w = VALUES.write().unwrap(); *w = Some(Arc::new(v.into_iter().filter(|x| ok(x)).collect())); }

/// Sec.ok: not empty and not starting with the mask's own character.
pub fn ok(s: &str) -> bool { !s.is_empty() && !s.starts_with(BULLET) }

/// The `api.env` values (8+ characters, one line), read again at most once a second, when the file changed.
pub fn set_api_path(path: std::path::PathBuf) { *API.lock().unwrap() = Some(ApiCache { at: std::time::Instant::now() - std::time::Duration::from_secs(5), mtime: None, vals: vec![], path }); }

fn api_values() -> Vec<String> {
    let mut g = API.lock().unwrap();
    let Some(c) = g.as_mut() else { return vec![] };
    if c.at.elapsed() < std::time::Duration::from_secs(1) { return c.vals.clone() }
    c.at = std::time::Instant::now();
    let mt = std::fs::metadata(&c.path).and_then(|m| m.modified()).ok();
    if mt.is_none() { c.vals.clear(); c.mtime = None; return vec![] }
    if mt == c.mtime { return c.vals.clone() }
    c.mtime = mt;
    c.vals = std::fs::read_to_string(&c.path).map(|t| crate::settings::keys::values(&t)).unwrap_or_default();
    c.vals.clone()
}

/// Every value masked now: the vault's and `api.env`'s, 8+ characters, no line break.
pub fn all() -> Vec<String> {
    let mut out: Vec<String> = VALUES.read().unwrap().as_ref().map(|a| a.iter().filter(|v| v.chars().count() >= MIN).cloned().collect()).unwrap_or_default();
    for v in api_values() { if v.chars().count() >= MIN && !v.contains('\n') && !v.contains('\r') && ok(&v) { out.push(v) } }
    out
}

/// Sec.clean: no value occurs in `s`.
pub fn clean_with(vals: &[String], s: &str) -> bool { vals.iter().all(|v| !s.contains(v.as_str())) }

/// Sec.fine: each character inside an occurrence of a value, as a bullet (occurrences may overlap).
pub fn fine(vals: &[String], s: &str) -> String {
    let mut cover = vec![0i32; s.len() + 1];
    for v in vals {
        if v.is_empty() { continue }
        let mut from = 0;
        while let Some(i) = s[from..].find(v.as_str()) {
            let at = from + i;
            cover[at] += 1; cover[at + v.len()] -= 1;
            from = at + s[at..].chars().next().map_or(1, char::len_utf8);
        }
    }
    let mut res = String::with_capacity(s.len());
    let mut acc = 0;
    let mut idx = 0;
    for c in s.chars() {
        acc += cover[idx]; // every start and end lies on a character boundary
        res.push(if acc > 0 { BULLET } else { c });
        idx += c.len_utf8();
    }
    res
}

/// Sec.mask over an explicit value list: clean text is left alone; the cover is checked and, should it leave a value,
/// the whole text becomes bullets (law sec_mask_total holds whatever `fine` does).
pub fn mask_with(vals: &[String], s: &str) -> String {
    if vals.is_empty() || clean_with(vals, s) { return s.to_string() }
    let f = fine(vals, s);
    if clean_with(vals, &f) { f } else { std::iter::repeat(BULLET).take(f.chars().count()).collect() }
}

/// Sec.mask over everything the hub holds now.
pub fn mask(s: &str) -> String { let v = all(); mask_with(&v, s) }

/// Sec.json: every string and every key of a tree masked; a clean tree comes back as it was.
pub fn json_with(vals: &[String], j: &mut Value) {
    match j {
        Value::String(s) => { if !clean_with(vals, s) { *s = mask_with(vals, s) } }
        Value::Array(a) => for x in a { json_with(vals, x) },
        Value::Object(m) => {
            if m.keys().all(|k| clean_with(vals, k)) { for (_, x) in m.iter_mut() { json_with(vals, x) } }
            else { let old = std::mem::take(m); let mut n = Map::new(); for (k, mut x) in old { json_with(vals, &mut x); n.insert(mask_with(vals, &k), x); } *m = n; }
        }
        _ => {}
    }
}
pub fn json(j: &mut Value) { let v = all(); if !v.is_empty() { json_with(&v, j) } }

/// The commit's masking (Hub.step.go, Secret.changes): every event of a step before it is written. With nothing stored the
/// events pass through untouched (law sec_changes_empty).
pub fn events(evs: &mut [Value]) { let v = all(); if v.is_empty() { return } for e in evs { json_with(&v, e) } }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn v(xs: &[&str]) -> Vec<String> { xs.iter().map(|s| s.to_string()).collect() }

    #[test] fn sec_mask_total() {
        for (vals, s) in [(v(&["SECRET-FIXTURE-1"]), "a SECRET-FIXTURE-1 b"), (v(&["abcdefgh", "defghijk"]), "xabcdefghijkx"), (v(&["héllo-wörld"]), "💥héllo-wörld💥"), (v(&["aaaaaaaa"]), "aaaaaaaaaaaa")] {
            let m = mask_with(&vals, s);
            assert!(clean_with(&vals, &m), "{s} -> {m}");
            assert_eq!(m.chars().count(), s.chars().count(), "length kept");
        }
        assert_eq!(mask_with(&v(&["SECRET-FIXTURE-1"]), "in SECRET-FIXTURE-1."), "in ••••••••••••••••.");
    }
    #[test] fn sec_mask_idem() { let vals = v(&["abcdefgh", "defghijk"]); for s in ["xabcdefghijkx", "none", "abcdefgh abcdefgh"] { let a = mask_with(&vals, s); assert_eq!(mask_with(&vals, &a), a) } }
    #[test] fn sec_mask_clean_id() { let vals = v(&["abcdefgh"]); for s in ["", "plain text", "abcdefg"] { assert_eq!(mask_with(&vals, s), s) } }
    #[test] fn sec_empty_id() { for s in ["", "abc", "••••"] { assert_eq!(mask_with(&[], s), s) } }
    #[test] fn overlapping_values_are_covered_together() { assert_eq!(mask_with(&v(&["abcdefgh", "defghijk"]), "xabcdefghijkx"), "x•••••••••••x"); }
    #[test] fn sec_json_total() {
        let vals = v(&["tok-123456789"]);
        let mut j = json!({"a":"x tok-123456789 y","tok-123456789":["z", {"k":"tok-123456789"}],"n":1,"b":true,"z":null});
        json_with(&vals, &mut j);
        assert!(!j.to_string().contains("tok-123456789"), "{j}");
        assert_eq!(j["n"], 1);
    }
    #[test] fn sec_changes_empty() { let mut e = vec![json!({"$":"X","a":"anything"})]; let before = e.clone(); json_with(&[], &mut e[0]); assert_eq!(e, before); }
    #[test] fn a_value_starting_with_the_mask_character_is_not_storable() { assert!(!ok("•abc")); assert!(!ok("")); assert!(ok("abcdefgh")); }
}

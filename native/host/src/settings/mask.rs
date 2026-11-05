//! Sec.mask / Sec.json / Sec.clean (core/secret.bend, laws sec_*): every character inside an occurrence of a stored
//! value becomes U+2022, the length kept. The values live in one process-wide set the vault and `api.env` feed; only
//! the masks below read it, and nothing here returns a value.
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex, RwLock};

const BULLET: char = '\u{2022}';
/// Values shorter than this would mask ordinary text (vault.js bp_vault_all).
pub const MIN: usize = 8;

static VAULT: RwLock<Vec<String>> = RwLock::new(Vec::new());
static MERGED: RwLock<Option<Arc<Vec<String>>>> = RwLock::new(None);
static API: Mutex<Option<ApiCache>> = Mutex::new(None);
struct ApiCache { at: std::time::Instant, mtime: Option<std::time::SystemTime>, vals: Vec<String>, path: std::path::PathBuf }

/// The vault's open values (called by the vault after every change).
pub fn set_vault_values(v: Vec<String>) { *VAULT.write().unwrap() = storable(v); rebuild(); }

/// Sec.vault / Sec.keep: only the values the masks can cover (law sec_vault_valid: every one is `ok`).
pub fn storable(v: Vec<String>) -> Vec<String> { v.into_iter().filter(|x| ok(x)).collect() }

/// Sec.ok: not empty and not starting with the mask's own character.
pub fn ok(s: &str) -> bool { !s.is_empty() && !s.starts_with(BULLET) }

/// The `api.env` values (8+ characters, one line), read again at most once a second, when the file changed.
pub fn set_api_path(path: std::path::PathBuf) { *API.lock().unwrap() = Some(ApiCache { at: std::time::Instant::now() - std::time::Duration::from_secs(5), mtime: None, vals: vec![], path }); rebuild(); }

/// Every form one value is masked in: as it is and, where it differs, JSON-escaped (agent lines and arguments are JSON text, so a
/// key holding a quote, a backslash or a control character is written escaped there).
pub fn forms_of(v: &str) -> Vec<String> {
    let mut out = vec![v.to_string()];
    if let Ok(j) = serde_json::to_string(v) { let e = j[1..j.len() - 1].to_string(); if e != v { out.push(e) } }
    out
}

/// The merged set (vault + api.env, 8+ characters, no line break in an api.env value, JSON forms added), built when either changes.
pub fn merge(vault: &[String], api: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for v in vault.iter().chain(api.iter().filter(|v| !v.contains('\n') && !v.contains('\r'))) {
        if v.chars().count() < MIN || !ok(v) { continue }
        for f in forms_of(v) { if !out.contains(&f) { out.push(f) } }
    }
    out
}

#[cfg(test)]
pub fn clear_api() { *API.lock().unwrap() = None; rebuild(); }

/// Rebuilds the merged set. MERGED's write lock is taken FIRST and held across reading both sources and publishing, so two overlapping
/// rebuilds are serialized: the one that runs last read the newest VAULT/API, and an older view can never be written after a newer one (a
/// person's saved secret would otherwise stay unmasked until the next change). Lock order: MERGED, then API, then VAULT; `set_vault_values`
/// and `refresh_api` hold neither of the latter two when they call this.
fn rebuild() {
    let mut out = MERGED.write().unwrap();
    let api: Vec<String> = API.lock().unwrap().as_ref().map(|c| c.vals.clone()).unwrap_or_default();
    let m = merge(&VAULT.read().unwrap(), &api);
    *out = Some(Arc::new(m));
}

/// api.env was just written by this process (add or delete a key): read it NOW. The one-second cache below exists for edits made by someone else; a
/// key the person saves in Settings must never wait out the cache, or it sits unmasked in the log for up to a second (found by the end-to-end leak test).
pub fn reload_api() {
    {
        let mut g = API.lock().unwrap();
        let Some(c) = g.as_mut() else { return };
        c.at = std::time::Instant::now();
        c.mtime = std::fs::metadata(&c.path).and_then(|m| m.modified()).ok();
        c.vals = if c.mtime.is_none() { vec![] } else { std::fs::read_to_string(&c.path).map(|t| crate::settings::keys::values(&t)).unwrap_or_default() };
    }
    rebuild();
}

fn refresh_api() {
    let changed = {
        let mut g = API.lock().unwrap();
        let Some(c) = g.as_mut() else { return };
        if c.at.elapsed() < std::time::Duration::from_secs(1) { return }
        c.at = std::time::Instant::now();
        let mt = std::fs::metadata(&c.path).and_then(|m| m.modified()).ok();
        if mt == c.mtime { false }
        else { c.mtime = mt; c.vals = if mt.is_none() { vec![] } else { std::fs::read_to_string(&c.path).map(|t| crate::settings::keys::values(&t)).unwrap_or_default() }; true }
    };
    if changed { rebuild() }
}

/// The values to mask now: an `Arc` clone, no allocation (the merged set changes only when the vault or api.env does).
pub fn snapshot() -> Arc<Vec<String>> {
    refresh_api();
    MERGED.read().unwrap().clone().unwrap_or_default()
}

/// Every value masked now (and the JSON forms), for the vault's run.
pub fn all() -> Vec<String> { snapshot().to_vec() }

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

/// Sec.mask over everything the hub holds now. With nothing stored it returns the input as it is: no scan, no allocation.
pub fn mask_cow(s: &str) -> std::borrow::Cow<'_, str> {
    let v = snapshot();
    if v.is_empty() || clean_with(&v, s) { std::borrow::Cow::Borrowed(s) } else { std::borrow::Cow::Owned(mask_with(&v, s)) }
}
pub fn mask(s: &str) -> String { mask_cow(s).into_owned() }
/// For an owned line (an agent's stdout, a line to its stdin): the same String comes back when it is clean, nothing is copied.
pub fn mask_owned(s: String) -> String {
    let v = snapshot();
    if v.is_empty() || clean_with(&v, &s) { s } else { mask_with(&v, &s) }
}

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
pub fn json(j: &mut Value) { let v = snapshot(); if !v.is_empty() { json_with(&v, j) } }

/// The commit's masking (Hub.step.go, Secret.changes): every event of a step before it is written. With nothing stored the
/// events pass through untouched (law sec_changes_empty).
pub fn events(evs: &mut [Value]) { let v = snapshot(); if v.is_empty() { return } for e in evs { json_with(&v, e) } }

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
    #[test] fn sec_vault_valid() { let k = storable(v(&["", "\u{2022}abcdefgh", "abcdefgh", "\u{2022}", "x"])); assert_eq!(k, v(&["abcdefgh", "x"])); assert!(k.iter().all(|x| ok(x))); }
    #[test] fn a_value_starting_with_the_mask_character_is_not_storable() { assert!(!ok("•abc")); assert!(!ok("")); assert!(ok("abcdefgh")); }

    use crate::settings::vault::TEST_LOCK;
    fn with_values<T>(vault: &[&str], f: impl FnOnce() -> T) -> T {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_api(); set_vault_values(vault.iter().map(|s| s.to_string()).collect()); let r = f(); set_vault_values(vec![]); r
    }
    #[test] fn with_nothing_stored_a_line_comes_back_as_it_is_without_a_copy() {
        with_values(&[], || {
            let line = String::from("a very ordinary line"); let p = line.as_ptr(); let out = mask_owned(line); assert_eq!(out.as_ptr(), p, "the same allocation");
            assert!(matches!(mask_cow("plain"), std::borrow::Cow::Borrowed(_)));
        });
        with_values(&["abcdefgh-1234"], || { let line = String::from("clean line"); let p = line.as_ptr(); assert_eq!(mask_owned(line).as_ptr(), p, "a clean line is not copied either"); assert_eq!(mask_owned("x abcdefgh-1234".into()), format!("x {}", "\u{2022}".repeat(13))); });
    }
    #[test] fn the_json_escaped_form_of_a_value_is_masked_too() {
        // an api.env key may hold a quote, a backslash, a control character; an agent line is JSON text, where it is written escaped
        let key = "ab\"cd\\ef\tgh-12345";
        let esc = forms_of(key); assert_eq!(esc.len(), 2); assert_eq!(esc[1], "ab\\\"cd\\\\ef\\tgh-12345");
        with_values(&[], || {
            set_vault_values(vec![key.to_string()]);
            let line = json!({"type":"user","message":{"content":key}}).to_string();
            assert!(line.contains(&esc[1]) && !line.contains(key), "the line holds the escaped form: {line}");
            let m = mask_owned(line); assert!(!m.contains(&esc[1]) && !m.contains("ab\\\"cd"), "{m}");
            assert!(serde_json::from_str::<Value>(&m).is_ok(), "still JSON: {m}");
        });
    }
    /// KNOWN LIMIT (Bend's per-line Hub.tagged has it too): live text is masked chunk by chunk, so a value split across two chunks is not
    /// covered in the live frames; the committed message that replaces the live text is masked whole.
    #[test] fn a_secret_split_across_two_live_chunks_is_a_known_limit_the_committed_text_is_masked_whole() {
        with_values(&["abcdefgh-1234"], || {
            let (a, b) = ("see abcdefgh", "-1234 here");
            assert_eq!(format!("{}{}", mask(a), mask(b)), "see abcdefgh-1234 here", "per chunk: not covered");
            assert!(!mask(&format!("{a}{b}")).contains("abcdefgh-1234"), "whole: covered");
        });
    }
    /// What the line mask costs per MB (printed, not asserted; run with --nocapture): a clean line, then a line holding a value.
    #[test] fn line_mask_cost_per_mb() {
        for n in [1usize, 50] {
            let vals: Vec<String> = (0..n).map(|i| format!("secret-value-{i:04}-xyz")).collect();
            let clean = "x".repeat(1 << 20); let dirty = format!("{}{}{}", "y".repeat(500_000), vals[n - 1], "y".repeat(500_000));
            let t = std::time::Instant::now(); for _ in 0..5 { assert!(clean_with(&vals, &clean)); } let c = t.elapsed() / 5;
            let t = std::time::Instant::now(); for _ in 0..5 { assert!(clean_with(&vals, &mask_with(&vals, &dirty))); } let d = t.elapsed() / 5;
            println!("MASKCOST secrets={n}: clean 1 MB {c:?}; 1 MB holding a value {d:?}");
        }
    }

    /// The leak the end-to-end test found: a key saved right after the api.env cache was read stayed unmasked until the cache expired.
    #[test] fn an_api_key_is_masked_the_instant_it_is_saved_not_when_the_cache_expires() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let home = std::env::temp_dir().join(format!("bp-apirace-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        clear_api(); set_vault_values(vec![]); set_api_path(crate::settings::keys::path(&home));
        for i in 0..20 {
            let _ = snapshot(); // a commit just read (and cached) the file, which has no such key yet
            let key = format!("apikey-Race-{i:04}-xyz");
            crate::settings::keys::add(&home, &format!("RACE_{i}"), &key).unwrap();
            assert_eq!(mask(&format!("a {key} b")), format!("a {} b", "\u{2022}".repeat(key.len())), "key {i} leaked");
            let mut ev = vec![json!({"$":"MessagePosted","text":format!("x {key}")})]; events(&mut ev); assert!(!ev[0].to_string().contains(&key));
        }
        crate::settings::keys::del(&home, "RACE_3").unwrap(); assert_eq!(mask("apikey-Race-0003-xyz"), "apikey-Race-0003-xyz", "a deleted key is no longer masked");
        clear_api(); let _ = std::fs::remove_dir_all(&home);
    }
}

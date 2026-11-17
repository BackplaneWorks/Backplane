//! KiCad pins from a signed feed (docs/kicad-pins.md): the KiCad runtime ships without a Backplane release. `kicad-pins.json`, an asset of
//! BackplaneWorks/Backplane's fixed release `kicad-pins`, is signed by `.github/workflows/kicad-pins.yml` with the release key, the same key and the
//! same check (`update::rel::verify`) as a release's manifest. The hub fetches it at start, every six hours and when an editor opens at Edit, and
//! takes a platform's pin from it only when the whole document verifies, its sequence is not below the last one seen, it has not expired, every
//! field passes `install::check_package`, and the pin is newer than the one compiled in (`install::older`), for this platform, and allows this hub
//! (`min_hub`). Anything else leaves the compiled pin (`install::LINUX`, `MAC`, `WINDOWS`), offline included. The last good document is kept in
//! `<home>/kicad-pins/pins.json`, so a replayed older one is refused after a restart too. The download it names is still checked against its
//! sha256 and the release's SHA256SUMS by `install::fetch`, and comes only from the KiCad release `install::FROM` names.
use super::install::{self, Package};
use crate::{hub::Hub, update::rel};
use base64::Engine;
use serde_json::Value;
use std::{path::{Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}};

/// The published document.
pub const URL: &str = "https://github.com/BackplaneWorks/Backplane/releases/download/kicad-pins/kicad-pins.json";
/// What the signed body says it is: a release manifest signed by the same key has no `kind` (and a pin document has no `assets`), so neither passes as the other.
pub const KIND: &str = "backplane-kicad-pins";
/// The largest document read (the real one is about 2 KB).
pub const MAX_DOC: u64 = 256 << 10;
/// The largest runtime a pin may name.
pub const MAX_SIZE: u64 = 8 << 30;
/// How often the feed is read, and how soon after the last try an editor opening at Edit reads it again.
pub const EVERY: u64 = 6 * 3600;
pub const NUDGE: u64 = 15 * 60;
const PLATFORMS: [&str; 3] = ["linux-x86_64", "darwin-arm64", "windows-x86_64"];

/// One platform's pin, as signed.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin { pub platform: String, pub version: String, pub release: String, pub asset: String, pub sha256: String, pub size: u64, pub min_hub: String, pub caps: Vec<String>, pub min_os: Vec<u32> }
/// A verified document. `raw`: the envelope's bytes as fetched, which is what the cache keeps (it is verified again when read back).
#[derive(Clone, Debug, PartialEq)]
pub struct Doc { pub sequence: u64, pub expires: u64, pub pins: Vec<Pin>, pub raw: Vec<u8>,
    /// Signed by the E2E's test key (feature `e2e-trust` only): its pins are all below rel's ceiling, so none is newer than a real compiled pin,
    /// and a test pin is compared with the installed runtime only.
    pub test: bool }

/// The envelope `{"schema":1,"keyid","sig","doc"}` (sig: base64 of the 64-byte ed25519 signature over doc's bytes; doc: base64 of the body)
/// verified under `keys`, then its body read. One file, so a document and its signature are always uploaded together.
pub fn open(envelope: &[u8], keys: &[(String, [u8; 32])]) -> Result<Doc, String> {
    if envelope.len() as u64 > MAX_DOC { return Err("the pin document is too large".into()) }
    let e: Value = serde_json::from_slice(envelope).map_err(|_| "the pin document is not JSON".to_string())?;
    if e["schema"].as_u64() != Some(1) { return Err("the pin document is from a newer Backplane".into()) }
    let b64 = |k: &str| e[k].as_str().and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok()).ok_or_else(|| format!("the pin document's {k} is malformed"));
    let (body, sig, keyid) = (b64("doc")?, b64("sig")?, e["keyid"].as_str().unwrap_or(""));
    rel::verify(&body, &sig, keyid, keys).map_err(|e| e.replace("release", "pin document"))?;
    let mut d = parse(&body)?;
    // a test key (feature e2e-trust only) may pin nothing a real runtime is: every version below rel's ceiling
    for p in &d.pins { rel::ceiling_ok(keyid, &p.version)? }
    d.raw = envelope.to_vec(); d.test = rel::is_e2e_key(keyid);
    Ok(d)
}

/// The signed body: `{"schema":1,"kind","sequence","expires","pins":{<platform>:{version,release,asset,sha256,size,min_hub,caps?,min_os?}}}`.
/// Any field that is wrong refuses the whole document: it was signed, so a bad field is a mistake in it, and nothing of it is trusted.
pub fn parse(body: &[u8]) -> Result<Doc, String> {
    let v: Value = serde_json::from_slice(body).map_err(|_| "the pin document's body is not JSON".to_string())?;
    if v["schema"].as_u64() != Some(1) { return Err("the pin document is from a newer Backplane".into()) }
    if v["kind"].as_str() != Some(KIND) { return Err("the signed document is not a pin document".into()) }
    let sequence = v["sequence"].as_u64().filter(|n| *n > 0).ok_or("the pin document has no sequence")?;
    let expires = v["expires"].as_u64().filter(|n| *n > 0).ok_or("the pin document has no expiry")?;
    let mut pins = vec![];
    for (plat, p) in v["pins"].as_object().ok_or("the pin document lists no pins")? {
        let compiled = compiled(plat).ok_or_else(|| format!("the pin document names an unknown platform {plat:?}"))?;
        let s = |k: &str| p[k].as_str().map(str::to_string).ok_or_else(|| format!("{plat}: no {k}"));
        let caps = match &p["caps"] { Value::Null => vec![], Value::Array(a) => a.iter().map(|c| c.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().ok_or_else(|| format!("{plat}: caps is not a list of names"))?, _ => return Err(format!("{plat}: caps is not a list")) };
        if caps.len() > 256 || !caps.iter().all(|c| !c.is_empty() && c.len() <= 64 && c.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')) { return Err(format!("{plat}: a capability name is malformed")) }
        let min_os = match &p["min_os"] { Value::Null => vec![], Value::String(s) => os_numbers(s).ok_or_else(|| format!("{plat}: min_os is malformed"))?, _ => return Err(format!("{plat}: min_os is malformed")) };
        let pin = Pin { platform: plat.clone(), version: s("version")?, release: s("release")?, asset: s("asset")?, sha256: s("sha256")?, size: p["size"].as_u64().unwrap_or(0), min_hub: s("min_hub")?, caps, min_os };
        if !rel::tag_ok(&format!("v{}", pin.min_hub)) { return Err(format!("{plat}: min_hub {:?} is not a Backplane version", pin.min_hub)) }
        if pin.size == 0 || pin.size > MAX_SIZE { return Err(format!("{plat}: the size is out of range")) }
        install::check_fields(compiled.platform, &pin.version, &pin.release, &pin.asset, &pin.sha256, compiled.format).map_err(|e| format!("{plat}: {e}"))?;
        pins.push(pin);
    }
    Ok(Doc { sequence, expires, pins, raw: vec![], test: false })
}
fn os_numbers(s: &str) -> Option<Vec<u32>> {
    let v: Vec<u32> = s.split('.').map(|x| (!x.is_empty() && x.len() <= 4).then(|| x.parse().ok()).flatten()).collect::<Option<_>>()?;
    (!v.is_empty() && v.len() <= 4).then_some(v)
}
/// The compiled pin of a platform, whose format and launcher a feed pin keeps (they are the platform's, never the document's).
fn compiled(platform: &str) -> Option<&'static Package> { [&install::LINUX, &install::MAC, &install::WINDOWS].into_iter().find(|p| p.platform == platform && PLATFORMS.contains(&platform)) }

/// Whether `new` may replace `prev` (the last good document): never an expired one, never a lower sequence (an old document replayed), and
/// never another document under the same sequence. The same document again is fine (nothing changes).
pub fn admit(prev: Option<&Doc>, new: Doc, now: u64) -> Result<Doc, String> {
    if new.expires <= now { return Err("the pin document has expired".into()) }
    if let Some(p) = prev {
        if new.sequence < p.sequence { return Err(format!("the pin document's sequence {} is below the last one seen ({})", new.sequence, p.sequence)) }
        if new.sequence == p.sequence && (new.pins != p.pins || new.expires != p.expires) { return Err(format!("another pin document under sequence {}", new.sequence)) }
    }
    Ok(new)
}

/// The pin this hub takes from `doc`: the one for `compiled`'s platform, unexpired, newer than the compiled pin, allowing `hub` (its version)
/// and not above this OS (`os_have`, as `install::os_version` says it). None: the compiled pin stays.
pub fn choose<'a>(doc: &'a Doc, compiled: &Package, hub: &str, now: u64, os_have: &str) -> Option<&'a Pin> {
    if doc.expires <= now { return None }
    let p = doc.pins.iter().find(|p| p.platform == compiled.platform)?;
    if !install::older_than(compiled.version, &p.version) && !doc.test { return None }
    if rel::Ver::parse(&p.min_hub).newer(&rel::Ver::parse(hub)) { return None }
    let below = install::os_floor(compiled).and_then(|(name, f)| install::below(name, if p.min_os.is_empty() { f } else { &p.min_os }, os_have));
    below.is_none().then_some(p)
}

/// A pin as the installer's package. Its strings live as long as the hub (the hub keeps `&'static Package`s); one is made only when the pin taken
/// changes, so this is bounded by the pins ever published.
pub fn package_of(p: &Pin, c: &'static Package) -> &'static Package {
    let s = |x: &str| -> &'static str { Box::leak(x.to_string().into_boxed_str()) };
    let caps: Vec<&'static str> = p.caps.iter().map(|c| s(c)).collect();
    Box::leak(Box::new(Package { platform: c.platform, version: s(&p.version), release: s(&p.release), asset: s(&p.asset), sha256: s(&p.sha256), format: c.format, cli: c.cli, size: p.size,
        caps: Box::leak(caps.into_boxed_slice()), min_os: Box::leak(p.min_os.clone().into_boxed_slice()) }))
}

// ---- the hub's state ----

/// What this hub holds: the last good document and the pin it took from it (with the document's expiry), for the hub version `hub`.
pub struct Feed { doc: Option<Doc>, taken: Option<(&'static Package, u64)>, hub: String, last_try: u64, test: bool }
impl Feed {
    pub const fn new() -> Feed { Feed { doc: None, taken: None, hub: String::new(), last_try: 0, test: false } }
    /// The pin taken for `compiled`'s platform while its document is unexpired and it is still newer than `compiled`.
    pub fn taken(&self, compiled: &Package, now: u64) -> Option<&'static Package> {
        self.taken.filter(|(p, exp)| p.platform == compiled.platform && *exp > now && (self.test || install::older(compiled.version, p))).map(|(p, _)| p)
    }
    /// Take `doc` (opened) when `admit` lets it replace the last good one: keep it, and the pin it gives a hub on `here` (this computer's compiled
    /// pin; None: none is published for it) and `os_have`. Ok((whether the pin taken changed, what to log)).
    pub fn take(&mut self, doc: Doc, here: Option<&'static Package>, now: u64, os_have: &str) -> Result<(bool, String), String> {
        let doc = admit(self.doc.as_ref(), doc, now)?;
        Ok(self.hold(doc, here, now, os_have))
    }
    /// `take` without `admit`: the cached document read back at start, which holds its sequence against a replay even once it has expired.
    fn hold(&mut self, doc: Doc, here: Option<&'static Package>, now: u64, os_have: &str) -> (bool, String) {
        let chosen = here.and_then(|c| choose(&doc, c, &self.hub, now, os_have).map(|p| (p.clone(), c)));
        let key = |p: &Package| (p.version.to_string(), p.sha256.to_string(), p.size, p.caps.iter().map(|c| c.to_string()).collect::<Vec<_>>(), p.min_os.to_vec());
        let before = self.taken.map(|(p, _)| key(p));
        self.taken = chosen.map(|(p, c)| {
            let same = self.taken.filter(|(t, _)| Some(key(t)) == Some((p.version.clone(), p.sha256.clone(), p.size, p.caps.clone(), p.min_os.clone()))).map(|(t, _)| t);
            (same.unwrap_or_else(|| package_of(&p, c)), doc.expires)
        });
        let now_key = self.taken.map(|(p, _)| key(p));
        let words = match (self.taken, here) {
            (Some((p, _)), _) => format!("sequence {}: Backplane KiCad {} for this computer", doc.sequence, p.version),
            (None, Some(c)) => format!("sequence {}: nothing newer than the compiled {} for this computer", doc.sequence, c.version),
            (None, None) => format!("sequence {}: no pin for this computer", doc.sequence),
        };
        self.test = doc.test; self.doc = Some(doc);
        (before != now_key, words)
    }
    fn words(&self) -> String { self.taken.map_or("the compiled pins".into(), |(p, _)| format!("Backplane KiCad {} from the feed", p.version)) }
}
static STATE: Mutex<Feed> = Mutex::new(Feed::new());
static BUSY: AtomicBool = AtomicBool::new(false);
fn now_s() -> u64 { crate::hub::now_ms() / 1000 }
fn here() -> Option<&'static Package> { install::pick(std::env::consts::OS, std::env::consts::ARCH) }

/// The feed's pin for `compiled`'s platform while its document is unexpired, else None (the compiled pin): `install::package` asks this.
pub fn taken(compiled: &'static Package) -> Option<&'static Package> { STATE.lock().unwrap().taken(compiled, now_s()) }

/// Whether the feed is read: never by a copy of a log, never with `BACKPLANE_KICAD_PINS=off`, never by a test build's hubs (the tests drive
/// `Feed` and `fetch` themselves).
pub fn on() -> bool { !cfg!(test) && !crate::copy::on() && std::env::var("BACKPLANE_KICAD_PINS").map_or(true, |v| v != "off") }
/// Where the document is read: `BACKPLANE_KICAD_PINS_URL` (an E2E's or a rig's; the environment only, never a setting a client can write), else URL.
/// The signature has to match whatever serves it.
pub fn url() -> String { url_from(std::env::var("BACKPLANE_KICAD_PINS_URL").ok().as_deref()) }
pub fn url_from(v: Option<&str>) -> String { v.map(str::trim).filter(|u| u.starts_with("https://") || u.starts_with("http://")).map_or_else(|| URL.to_string(), str::to_string) }
fn dir(home: &Path) -> PathBuf { home.join("kicad-pins") }

/// At start: the cached document (verified again; an expired one still holds its sequence against a replay), then a read of the feed, and one
/// every EVERY seconds.
pub fn start(hub: &Arc<Hub>) {
    let mut st = STATE.lock().unwrap(); st.hub = hub.cfg.version.clone();
    if let Ok(b) = std::fs::read(dir(&hub.cfg.home).join("pins.json")) {
        match open(&b, &rel::keys()) {
            Ok(d) => { let (_, w) = st.hold(d, here(), now_s(), &install::os_version()); eprintln!("kicad: pins: cached {w}") }
            Err(e) => eprintln!("kicad: pins: the cached document is refused ({e}); the compiled pins stand"),
        }
    }
    drop(st);
    if !on() { return }
    let h = hub.clone();
    // after a read that failed (offline at start), the next comes in NUDGE seconds, not six hours
    tokio::spawn(async move { loop { let ok = refresh(&h).await; tokio::time::sleep(std::time::Duration::from_secs(if ok { EVERY } else { NUDGE })).await } });
}
/// An editor opening at Edit, where the update row is: read the feed again unless it was read in the last NUDGE seconds.
pub fn nudge(hub: &Arc<Hub>) {
    if !on() || now_s().saturating_sub(STATE.lock().unwrap().last_try) < NUDGE { return }
    let h = hub.clone(); tokio::spawn(async move { refresh(&h).await; });
}

/// The document at `url`, downloaded into `dir` (curl through proc.rs, at most MAX_DOC bytes, https never falling to http on a redirect).
pub fn fetch(url: &str, dir: &Path) -> Result<Vec<u8>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let f = dir.join("fetched.json"); let _ = std::fs::remove_file(&f);
    crate::update::svc::curl(url, &f, MAX_DOC).map_err(|e| e.replace("release feed", "KiCad pin feed"))?;
    let b = std::fs::read(&f).map_err(|e| e.to_string()); let _ = std::fs::remove_file(&f); b
}
/// One pass over fetched bytes: open, admit and take into `feed`, and keep a document that is new in `dir`. Ok(whether the pin taken changed, what
/// to log). Whatever fails leaves `feed` as it was.
pub fn apply(feed: &mut Feed, got: Result<Vec<u8>, String>, keys: &[(String, [u8; 32])], dir: &Path, here: Option<&'static Package>, now: u64, os_have: &str) -> Result<(bool, String), String> {
    let doc = open(&got?, keys)?;
    let new = feed.doc.as_ref().map_or(true, |p| p.raw != doc.raw);
    let raw = doc.raw.clone();
    let r = feed.take(doc, here, now, os_have)?;
    if new { write_cache(dir, &raw).map_err(|e| format!("could not keep the pin document: {e}"))?; }
    Ok(r)
}

/// One read of the feed. Whatever fails leaves what was there (the last good document's pin, else the compiled one) and says why in the log;
/// a pin that changed is published at once (`ipc::publish_cad`: the update row at Edit and Settings, CAD).
/// true when the feed answered with a document (taken or refused); false when it could not be read.
pub async fn refresh(hub: &Arc<Hub>) -> bool {
    if BUSY.swap(true, Ordering::SeqCst) { return true }
    STATE.lock().unwrap().last_try = now_s();
    let (d, u) = (dir(&hub.cfg.home), url());
    let d2 = d.clone();
    let got = tokio::task::spawn_blocking(move || fetch(&u, &d2)).await.unwrap_or_else(|e| Err(e.to_string()));
    let read = got.is_ok();
    let r = { let mut st = STATE.lock().unwrap(); let r = apply(&mut st, got, &rel::keys(), &d, here(), now_s(), &install::os_version()); r.map_err(|e| format!("{e}; keeping {}", st.words())) };
    match r {
        Ok((changed, w)) => { eprintln!("kicad: pins: {w}"); if changed { super::ipc::publish_cad(hub).await; hub.bump_views(); } }
        Err(e) => eprintln!("kicad: pins: {e}"),
    }
    BUSY.store(false, Ordering::SeqCst);
    read
}
fn write_cache(d: &Path, raw: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(d)?; let t = d.join("pins.json.new"); std::fs::write(&t, raw)?; std::fs::rename(&t, d.join("pins.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_compact::{KeyPair, Seed};
    fn pair(n: u8) -> KeyPair { KeyPair::from_seed(Seed::new([n; 32])) }
    fn keys() -> Vec<(String, [u8; 32])> { vec![("k1".into(), *pair(3).pk)] }
    const SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    fn entry(plat: &str, v: &str) -> Value {
        let fmt = if plat.starts_with("windows") { "zip" } else { "tar.gz" };
        serde_json::json!({"version": v, "release": format!("backplane-kicad-v{v}"), "asset": format!("backplane-kicad-{plat}-{v}.{fmt}"), "sha256": SHA, "size": 1_100_000_000u64, "min_hub": "0.16.1", "caps": ["drc-zone-guard", "global-tables-seeded"]})
    }
    fn body(seq: u64, expires: u64, pins: Value) -> Vec<u8> { serde_json::to_vec(&serde_json::json!({"schema": 1, "kind": KIND, "sequence": seq, "expires": expires, "pins": pins})).unwrap() }
    fn seal(body: &[u8], kp: &KeyPair, keyid: &str) -> Vec<u8> {
        let e = base64::engine::general_purpose::STANDARD;
        serde_json::to_vec(&serde_json::json!({"schema": 1, "keyid": keyid, "sig": e.encode(kp.sk.sign(body, None).as_ref()), "doc": e.encode(body)})).unwrap()
    }
    fn good(seq: u64) -> Vec<u8> { seal(&body(seq, 2_000_000_000, serde_json::json!({"linux-x86_64": entry("linux-x86_64", "10.99.0-bp.13"), "darwin-arm64": entry("darwin-arm64", "10.99.0-bp.13"), "windows-x86_64": entry("windows-x86_64", "10.99.0-bp.13")})), &pair(3), "k1") }
    const NOW: u64 = 1_800_000_000;

    #[test] fn a_good_document_opens_with_every_platforms_pin() {
        let d = open(&good(4), &keys()).unwrap();
        assert_eq!((d.sequence, d.pins.len()), (4, 3));
        let l = d.pins.iter().find(|p| p.platform == "linux-x86_64").unwrap();
        assert_eq!((l.version.as_str(), l.size, l.caps.len()), ("10.99.0-bp.13", 1_100_000_000, 2));
        assert_eq!(d.raw, good(4), "the envelope is what is cached");
    }
    #[test] fn a_bad_signature_or_another_key_is_refused() {
        let mut b = body(4, 2_000_000_000, serde_json::json!({"linux-x86_64": entry("linux-x86_64", "10.99.0-bp.13")}));
        let sealed = seal(&b, &pair(3), "k1");
        // the body changed after signing: the signature no longer matches
        let mut v: Value = serde_json::from_slice(&sealed).unwrap(); b[10] ^= 1; v["doc"] = base64::engine::general_purpose::STANDARD.encode(&b).into();
        assert!(open(&serde_json::to_vec(&v).unwrap(), &keys()).unwrap_err().contains("does not match"));
        // signed by another key under our key's id
        assert!(open(&seal(&body(4, 2_000_000_000, serde_json::json!({})), &pair(9), "k1"), &keys()).unwrap_err().contains("does not match"));
        // a key id this build does not trust
        assert!(open(&seal(&body(4, 2_000_000_000, serde_json::json!({})), &pair(3), "k2"), &keys()).unwrap_err().contains("does not trust"));
        // a short signature, no signature, junk
        let mut v: Value = serde_json::from_slice(&sealed).unwrap(); v["sig"] = "AAAA".into(); assert!(open(&serde_json::to_vec(&v).unwrap(), &keys()).is_err());
        let mut v: Value = serde_json::from_slice(&sealed).unwrap(); v.as_object_mut().unwrap().remove("sig"); assert!(open(&serde_json::to_vec(&v).unwrap(), &keys()).unwrap_err().contains("sig"));
        assert!(open(b"not json", &keys()).is_err()); assert!(open(&vec![b' '; (MAX_DOC + 1) as usize], &keys()).unwrap_err().contains("too large"));
        assert!(open(&good(4), &[]).unwrap_err().contains("no update key"));
    }
    #[test] fn the_release_key_trusts_no_document_signed_elsewhere() {
        assert!(open(&seal(&body(4, 2_000_000_000, serde_json::json!({})), &pair(3), "6c899f869be656d3"), &rel::keys()).unwrap_err().contains("does not match"));
        assert!(open(&good(4), &rel::keys()).unwrap_err().contains("does not trust"));
    }
    #[test] fn a_release_manifest_and_a_pin_document_never_pass_as_each_other() {
        // the same key signs both: the kind keeps them apart
        let manifest = br#"{"schema":1,"tag":"v0.16.1","version":"0.16.1","channel":"stable","commit":"c","assets":{}}"#;
        assert!(open(&seal(manifest, &pair(3), "k1"), &keys()).unwrap_err().contains("not a pin document"));
        assert!(rel::parse(&body(4, 2_000_000_000, serde_json::json!({}))).is_err(), "a pin document is no manifest");
    }
    #[test] fn a_replayed_lower_sequence_an_expired_document_or_a_second_one_under_a_sequence_is_refused() {
        let d4 = open(&good(4), &keys()).unwrap(); let d5 = open(&good(5), &keys()).unwrap();
        assert!(admit(None, d4.clone(), NOW).is_ok());
        assert!(admit(Some(&d4), d5.clone(), NOW).is_ok(), "a higher sequence");
        assert!(admit(Some(&d5), d4.clone(), NOW).unwrap_err().contains("below the last one seen"), "an older document replayed");
        assert!(admit(Some(&d4), d4.clone(), NOW).is_ok(), "the same one again");
        let other4 = open(&seal(&body(4, 2_000_000_000, serde_json::json!({"linux-x86_64": entry("linux-x86_64", "10.99.0-bp.14")})), &pair(3), "k1"), &keys()).unwrap();
        assert!(admit(Some(&d4), other4, NOW).unwrap_err().contains("another pin document"));
        let old = open(&seal(&body(6, NOW - 1, serde_json::json!({})), &pair(3), "k1"), &keys()).unwrap();
        assert!(admit(Some(&d5), old.clone(), NOW).unwrap_err().contains("expired")); assert!(admit(None, old, NOW).is_err());
    }
    #[test] fn any_field_that_fails_the_packages_check_refuses_the_whole_document() {
        let with = |f: &dyn Fn(&mut Value)| { let mut e = entry("linux-x86_64", "10.99.0-bp.13"); f(&mut e); open(&seal(&body(4, 2_000_000_000, serde_json::json!({"linux-x86_64": e, "darwin-arm64": entry("darwin-arm64", "10.99.0-bp.13")})), &pair(3), "k1"), &keys()) };
        assert!(with(&|_| {}).is_ok());
        for (why, f) in [
            ("short sha", &(|e: &mut Value| e["sha256"] = "abc".into()) as &dyn Fn(&mut Value)),
            ("upper sha", &|e| e["sha256"] = SHA.to_uppercase().into()),
            ("not hex", &|e| e["sha256"] = "g".repeat(64).into()),
            ("no sha", &|e| { e.as_object_mut().unwrap().remove("sha256"); }),
            ("another release", &|e| e["release"] = "backplane-kicad-v10.99.0-bp.12".into()),
            ("a path for an asset", &|e| e["asset"] = "../../backplane-hub".into()),
            ("a shell word in the version", &|e| { e["version"] = "1;rm".into(); e["release"] = "backplane-kicad-v1;rm".into(); e["asset"] = "backplane-kicad-linux-x86_64-1;rm.tar.gz".into(); }),
            ("no size", &|e| e["size"] = 0.into()),
            ("too big", &|e| e["size"] = (MAX_SIZE + 1).into()),
            ("min_hub not a version", &|e| e["min_hub"] = "soon".into()),
            ("caps not names", &|e| e["caps"] = serde_json::json!(["Has Space"])),
            ("min_os junk", &|e| e["min_os"] = "fifteen".into()),
        ] { assert!(with(f).is_err(), "{why}"); }
        let unknown = open(&seal(&body(4, 2_000_000_000, serde_json::json!({"freebsd-x86_64": entry("linux-x86_64", "10.99.0-bp.13")})), &pair(3), "k1"), &keys());
        assert!(unknown.unwrap_err().contains("unknown platform"));
        let wrong_fmt = open(&seal(&body(4, 2_000_000_000, serde_json::json!({"windows-x86_64": entry("linux-x86_64", "10.99.0-bp.13")})), &pair(3), "k1"), &keys());
        assert!(wrong_fmt.unwrap_err().contains("Invalid runtime archive"), "a Linux tarball under Windows");
        for (k, v) in [("sequence", serde_json::json!(0)), ("expires", serde_json::json!(null)), ("kind", serde_json::json!("x")), ("schema", serde_json::json!(2))] {
            let mut b: Value = serde_json::from_slice(&body(4, 2_000_000_000, serde_json::json!({}))).unwrap(); b[k] = v; assert!(open(&seal(&serde_json::to_vec(&b).unwrap(), &pair(3), "k1"), &keys()).is_err(), "{k}");
        }
    }
    #[test] fn a_pin_is_taken_only_newer_for_this_platform_and_this_hub() {
        let d = open(&good(4), &keys()).unwrap();
        assert_eq!(choose(&d, &install::LINUX, "0.16.1", NOW, "").map(|p| p.version.as_str()), Some("10.99.0-bp.13"));
        assert_eq!(choose(&d, &install::WINDOWS, "0.17.0", NOW, "").map(|p| p.platform.as_str()), Some("windows-x86_64"), "each platform its own");
        assert_eq!(choose(&d, &install::LINUX, "0.16.0", NOW, ""), None, "min_hub above this hub");
        assert_eq!(choose(&d, &install::LINUX, "0.16.1-beta.2", NOW, ""), None, "a beta of min_hub is below it");
        assert_eq!(choose(&d, &install::LINUX, "0.16.1", 2_000_000_000, ""), None, "expired while the hub ran");
        let newer = Package { version: "10.99.0-bp.14", release: "backplane-kicad-v10.99.0-bp.14", ..install::LINUX };
        assert_eq!(choose(&d, &newer, "0.16.1", NOW, ""), None, "never older than the compiled pin");
        let same = Package { version: "10.99.0-bp.13", ..install::LINUX };
        assert_eq!(choose(&d, &same, "0.16.1", NOW, ""), None, "nor the same");
        let only_mac = open(&seal(&body(4, 2_000_000_000, serde_json::json!({"darwin-arm64": entry("darwin-arm64", "10.99.0-bp.13")})), &pair(3), "k1"), &keys()).unwrap();
        assert_eq!(choose(&only_mac, &install::LINUX, "0.16.1", NOW, ""), None, "another platform's pin");
        let mut e = entry("darwin-arm64", "10.99.0-bp.13"); e["min_os"] = "26.0".into();
        let mac26 = open(&seal(&body(4, 2_000_000_000, serde_json::json!({"darwin-arm64": e})), &pair(3), "k1"), &keys()).unwrap();
        assert_eq!(choose(&mac26, &install::MAC, "0.16.1", NOW, "15.4"), None, "a Mac below the pin's macOS");
        assert!(choose(&mac26, &install::MAC, "0.16.1", NOW, "26.1").is_some());
    }
    #[test] fn a_taken_pin_installs_and_offers_like_a_compiled_one() {
        let d = open(&good(4), &keys()).unwrap();
        let p = package_of(choose(&d, &install::WINDOWS, "0.16.1", NOW, "").unwrap(), &install::WINDOWS);
        assert_eq!((p.format, p.cli, p.platform), ("zip", "bin/kicad-cli.exe", "windows-x86_64"), "the platform's, never the document's");
        assert!(install::check_package(p).is_ok());
        assert_eq!(install::base_url_from(p, None), "https://github.com/BackplaneWorks/Backplane_KiCad/releases/download/backplane-kicad-v10.99.0-bp.13/", "only from the KiCad release");
        let o = install::update_offer_os(Some(p), "ok", "10.99.0-bp.10", &["embed".into(), "drc-zone-guard".into()], "windows").expect("bp.10 is offered the feed's bp.13");
        assert_eq!(o["to"], "10.99.0-bp.13"); assert!(o["reason"].as_str().unwrap().contains("narrow pane"), "{o}");
    }

    /// A loopback web folder serving `files` (name, bytes) for `n` requests; a name not there answers 404.
    fn serve(files: Vec<(&'static str, Vec<u8>)>, n: usize) -> String {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let a = l.local_addr().unwrap();
        std::thread::spawn(move || for c in l.incoming().take(n) { let Ok(mut c) = c else { continue };
            let mut b = [0u8; 2048]; let k = c.read(&mut b).unwrap_or(0); let req = String::from_utf8_lossy(&b[..k]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").trim_start_matches('/').to_string();
            match files.iter().find(|(f, _)| *f == path) {
                Some((_, body)) => { let _ = write!(c, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()); let _ = c.write_all(body); }
                None => { let _ = c.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"); }
            }
        });
        format!("http://{a}")
    }
    #[test] fn the_hub_takes_a_served_pin_keeps_it_across_a_restart_and_falls_back_on_anything_wrong() {
        let d = crate::plat::tmp::dir("pins-feed"); let k = keys();
        let base = serve(vec![("kicad-pins.json", good(5)), ("old.json", good(4)), ("tampered.json", { let mut g = good(6); let n = g.len() - 30; g[n] ^= 1; g })], 8);
        let mut f = Feed::new(); f.hub = "0.16.1".into();
        let get = |n: &str| fetch(&format!("{base}/{n}"), &d.join("scratch"));
        // offline (nothing listens on that port any more) and a 404: the compiled pin stands
        assert!(apply(&mut f, fetch("http://127.0.0.1:9/kicad-pins.json", &d.join("scratch")), &k, &d, Some(&install::LINUX), NOW, "").unwrap_err().contains("could not reach the KiCad pin feed"));
        assert!(apply(&mut f, get("missing.json"), &k, &d, Some(&install::LINUX), NOW, "").is_err());
        assert_eq!(f.taken(&install::LINUX, NOW), None);
        // the published one: taken, and kept on disk
        let (changed, w) = apply(&mut f, get("kicad-pins.json"), &k, &d, Some(&install::LINUX), NOW, "").unwrap();
        assert!(changed && w.contains("10.99.0-bp.13"), "{w}");
        assert_eq!(f.taken(&install::LINUX, NOW).map(|p| p.version), Some("10.99.0-bp.13"));
        assert_eq!(std::fs::read(d.join("pins.json")).unwrap(), good(5));
        assert_eq!(apply(&mut f, get("kicad-pins.json"), &k, &d, Some(&install::LINUX), NOW, "").unwrap().0, false, "the same one again changes nothing");
        // a replay of an older one and a tampered newer one are refused, and the pin stays
        assert!(apply(&mut f, get("old.json"), &k, &d, Some(&install::LINUX), NOW, "").unwrap_err().contains("below the last one seen"));
        assert!(apply(&mut f, get("tampered.json"), &k, &d, Some(&install::LINUX), NOW, "").is_err());
        assert_eq!(f.taken(&install::LINUX, NOW).map(|p| p.version), Some("10.99.0-bp.13")); assert_eq!(std::fs::read(d.join("pins.json")).unwrap(), good(5));
        // a restart: the cached document holds the pin and its sequence, so the old one is still refused
        let mut g = Feed::new(); g.hub = "0.16.1".into();
        g.hold(open(&std::fs::read(d.join("pins.json")).unwrap(), &k).unwrap(), Some(&install::LINUX), NOW, "");
        assert_eq!(g.taken(&install::LINUX, NOW).map(|p| p.version), Some("10.99.0-bp.13"));
        assert!(apply(&mut g, get("old.json"), &k, &d, Some(&install::LINUX), NOW, "").unwrap_err().contains("below the last one seen"));
        // once the document expires the compiled pin is back, whatever was taken
        assert_eq!(g.taken(&install::LINUX, 2_000_000_000), None);
        // a hub too old for the pin keeps the compiled one
        let mut old = Feed::new(); old.hub = "0.16.0".into();
        assert_eq!(apply(&mut old, Ok(good(5)), &k, &d.join("x"), Some(&install::LINUX), NOW, "").unwrap().0, false); assert_eq!(old.taken(&install::LINUX, NOW), None);
    }
    #[test] fn the_feed_is_read_only_from_the_environment_and_the_web() {
        assert_eq!(url_from(None), URL); assert_eq!(url_from(Some("http://127.0.0.1:9/p.json")), "http://127.0.0.1:9/p.json");
        for no in ["", "/tmp/p.json", "file:///tmp/p.json"] { assert_eq!(url_from(Some(no)), URL, "{no:?}") }
        assert!(!on(), "a test build's hubs never read the feed");
    }
    /// What the update row says comes from the pin's own capabilities where it carries them, not from its number.
    #[test] fn what_a_feed_pin_brings_is_said_from_its_capabilities() {
        let pin = |plat: &'static str, caps: &[&str]| { let mut e = entry(plat, "10.99.0-bp.14"); e["caps"] = serde_json::json!(caps);
            let d = open(&seal(&body(4, 2_000_000_000, serde_json::json!({plat: e})), &pair(3), "k1"), &keys()).unwrap();
            package_of(&d.pins[0], compiled(plat).unwrap()) };
        // a bp.14 without global-tables-seeded is not said to bring bp.13's libraries; with it, it is
        let bare = pin("windows-x86_64", &["embed", "drc-zone-guard"]);
        assert!(!install::to13(bare) && install::to13(pin("windows-x86_64", &["global-tables-seeded"])) && install::to13(&install::Package { version: "10.99.0-bp.13", ..install::WINDOWS }), "a compiled pin goes by its number");
        let o = install::update_offer_os(Some(bare), "ok", "10.99.0-bp.10", &["embed".into(), "drc-zone-guard".into()], "windows");
        assert!(o.map_or(true, |o| !o["reason"].as_str().unwrap().contains("narrow pane")));
        // the Mac pane: only a pin with embed-macos is offered over stock KiCad there
        assert!(install::update_offer_os(Some(pin("darwin-arm64", &["global-tables-seeded"])), "stock", "10.0.8", &[], "macos").is_none());
        assert!(install::update_offer_os(Some(pin("darwin-arm64", &["embed-macos", "global-tables-seeded"])), "stock", "10.0.8", &[], "macos").is_some());
    }

    /// min_hub and the expiry are required: a document or a pin without one is refused whole, never read as "any hub" or "never expires".
    #[test] fn a_document_without_min_hub_or_an_expiry_is_refused() {
        let mut e = entry("linux-x86_64", "10.99.0-bp.13"); e.as_object_mut().unwrap().remove("min_hub");
        assert!(open(&seal(&body(4, 2_000_000_000, serde_json::json!({"linux-x86_64": e})), &pair(3), "k1"), &keys()).unwrap_err().contains("no min_hub"));
        let mut e = entry("linux-x86_64", "10.99.0-bp.13"); e["min_hub"] = "".into();
        assert!(open(&seal(&body(4, 2_000_000_000, serde_json::json!({"linux-x86_64": e})), &pair(3), "k1"), &keys()).is_err(), "an empty min_hub");
        for no in [serde_json::json!(null), serde_json::json!(0), serde_json::json!("2000000000"), serde_json::json!(-1)] {
            let mut b: Value = serde_json::from_slice(&body(4, 2_000_000_000, serde_json::json!({"linux-x86_64": entry("linux-x86_64", "10.99.0-bp.13")}))).unwrap();
            if no.is_null() { b.as_object_mut().unwrap().remove("expires"); } else { b["expires"] = no.clone(); }
            assert!(open(&seal(&serde_json::to_vec(&b).unwrap(), &pair(3), "k1"), &keys()).unwrap_err().contains("no expiry"), "expires {no}");
        }
    }
    /// Domain separation, both ways, under one key: a signed release manifest is no pin document, and a signed pin document (its body, or the
    /// envelope) is no release manifest to the updater (`update::run::find`, through `rel::verify` and `rel::parse`), whatever release carries it.
    #[test] fn a_release_manifest_is_no_pin_document_and_a_pin_document_is_no_release_manifest() {
        let kp = pair(3);
        let manifest = br#"{"schema":1,"tag":"v0.16.2","version":"0.16.2","channel":"stable","commit":"c","assets":{"linux-x64":{"name":"backplane-v0.16.2-linux-x64-native.tar.gz","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","size":9}}}"#;
        assert!(rel::parse(manifest).is_ok(), "the manifest is a good one");
        assert!(open(&seal(manifest, &kp, "k1"), &keys()).unwrap_err().contains("not a pin document"));
        // the updater: a release v0.16.2 whose manifest.json is a pin document's signed body (or its whole envelope), signed with the trusted key
        let pin_body = body(4, 2_000_000_000, serde_json::json!({"linux-x86_64": entry("linux-x86_64", "10.99.0-bp.13")}));
        for (what, m) in [("the signed body", pin_body.clone()), ("the envelope", good(4))] {
            let d = crate::plat::tmp::dir("pins-dom"); let sig = kp.sk.sign(&m, None);
            let files: std::collections::HashMap<String, Vec<u8>> = [("manifest.json", m.clone()), ("manifest.json.sig", sig.as_ref().to_vec()), ("manifest.json.keyid", b"k1".to_vec())].into_iter().map(|(n, b)| (format!("mem://{n}"), b)).collect();
            let get = |u: &str, out: &Path, _: u64| -> Result<(), String> { std::fs::write(out, files.get(u).ok_or("404")?).map_err(|e| e.to_string()) };
            let listed = crate::update::run::listing(br#"[{"tag_name":"v0.16.2","prerelease":false,"draft":false,"assets":[{"name":"manifest.json","browser_download_url":"mem://manifest.json"},{"name":"manifest.json.sig","browser_download_url":"mem://manifest.json.sig"},{"name":"manifest.json.keyid","browser_download_url":"mem://manifest.json.keyid"}]}]"#).unwrap();
            let r = crate::update::run::find(&crate::update::run::Ask { current: "0.16.0", channel: "nightly", target: "linux-x64", keys: &keys(), bad: &[] }, &listed, &d, &get);
            assert!(r.as_ref().is_err_and(|e| e.contains("lists no assets") || e.contains("newer Backplane")), "{what}: {r:?}");
        }
    }
    /// The release `kicad-pins` is no update to any channel: its tag is no updater's shape and it carries no signed manifest.
    #[test] fn the_kicad_pins_release_is_never_an_update() {
        let listed = crate::update::run::listing(br#"[{"tag_name":"kicad-pins","prerelease":true,"draft":false,"assets":[{"name":"kicad-pins.json","browser_download_url":"u"}]},
                                                   {"tag_name":"v0.16.0","prerelease":false,"draft":false,"assets":[]}]"#).unwrap();
        assert!(!rel::tag_ok("kicad-pins") && !listed[0].rel.signed);
        let rels: Vec<rel::Rel> = listed.iter().map(|l| l.rel.clone()).collect();
        for ch in ["stable", "beta", "nightly"] { assert!(!rel::wants(ch, &listed[0].rel), "{ch}"); assert_eq!(rel::offer("0.0.1", ch, &rels), None, "{ch}"); }
        // even signed (if the three files were ever there) the tag alone keeps it out
        assert!(!rel::wants("nightly", &rel::Rel { tag: "kicad-pins".into(), prerelease: true, draft: false, signed: true }));
    }
}

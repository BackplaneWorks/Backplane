//! What may be installed: versions, channels and the signed manifest. Pure: no clock, no files, no network, so every rule has a test.
//!
//! A release is offered only when it is signed, names itself correctly, is on this hub's channel and is newer than the running build. Nothing else here decides:
//! the guard (`native/guard`) owns the swap, the health check and the fallback.
use serde_json::Value;
use std::{cmp::Ordering, collections::BTreeMap};

/// A version: "v1.2.3" or "1.2.3", with an optional "-pre" suffix (semver.bend's `Ver`). A prerelease sorts before its release; prereleases of one version sort by their
/// first two numbers, so "-nightly.20260923.2" follows "-nightly.20260923.1" and "-beta.10" follows "-beta.2".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ver { pub major: u64, pub minor: u64, pub patch: u64, pub pre: bool, pd: u64, pn: u64 }

impl Ver {
    pub fn parse(s: &str) -> Ver {
        let s = s.trim(); let s = s.strip_prefix('v').unwrap_or(s);
        let (core, pre) = match s.split_once('-') { Some((c, p)) => (c, Some(p)), None => (s, None) };
        let mut c = core.split('.').map(|x| x.parse::<u64>().unwrap_or(0));
        let (major, minor, patch) = (c.next().unwrap_or(0), c.next().unwrap_or(0), c.next().unwrap_or(0));
        let nums: Vec<u64> = pre.map(|p| p.split('.').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
        Ver { major, minor, patch, pre: pre.is_some(), pd: nums.first().copied().unwrap_or(0), pn: nums.get(1).copied().unwrap_or(0) }
    }
    fn key(&self) -> (u64, u64, u64, bool, u64, u64) { (self.major, self.minor, self.patch, !self.pre, self.pd, self.pn) }
    pub fn cmp(&self, o: &Ver) -> Ordering { self.key().cmp(&o.key()) }
    /// Whether `self` should replace `current`: strictly newer, never the same.
    pub fn newer(&self, current: &Ver) -> bool { self.cmp(current) == Ordering::Greater }
}

/// What a stored `update.channel` means. "nightly" is the 0.14 name of what is now beta; it keeps working and is never rewritten. Anything else is stable.
pub fn channel(raw: &str) -> &'static str { match raw.trim() { "beta" | "nightly" => "beta", _ => "stable" } }

/// The exact shape of a tag this updater accepts: `vX.Y.Z`, `vX.Y.Z-beta.N`, or 0.14's `vX.Y.Z-nightly.YYYYMMDD.N`. It becomes a folder name (`builds/<tag>/`), so nothing else gets through.
pub fn tag_ok(tag: &str) -> bool {
    let num = |s: &str| !s.is_empty() && s.len() <= 10 && s.bytes().all(|b| b.is_ascii_digit());
    let Some(s) = tag.strip_prefix('v') else { return false };
    let (core, pre) = match s.split_once('-') { Some((c, p)) => (c, Some(p)), None => (s, None) };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|p| num(p)) { return false }
    match pre.map(|p| p.split('.').collect::<Vec<_>>()).as_deref() {
        None => true,
        Some(["beta", n]) => num(n),
        Some(["nightly", d, n]) => num(d) && num(n),
        _ => false,
    }
}
/// The channel a tag belongs to, whatever a manifest says of itself.
pub fn tag_channel(tag: &str) -> &'static str { if tag.contains('-') { "beta" } else { "stable" } }

/// One entry of the release list, as far as choosing goes. `signed`: the release carries `manifest.json` and its signature (they are uploaded last, so without them it is half published).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rel { pub tag: String, pub prerelease: bool, pub draft: bool, pub signed: bool }

/// Whether a channel takes a release. Stable takes only full releases; beta takes those and the prereleases.
pub fn wants(channel: &str, r: &Rel) -> bool {
    if r.draft || !r.signed || !tag_ok(&r.tag) { return false }
    let full = !r.prerelease && tag_channel(&r.tag) == "stable";
    full || (channel == "beta" && tag_channel(&r.tag) == "beta")
}
/// The newest release the channel takes that is newer than `current`, whatever order the list came in. Moving from beta to stable never goes back: it waits for a newer stable.
pub fn offer<'a>(current: &str, channel: &str, rels: &'a [Rel]) -> Option<&'a Rel> {
    let cur = Ver::parse(current);
    rels.iter().filter(|r| wants(channel, r) && Ver::parse(&r.tag).newer(&cur)).max_by(|a, b| Ver::parse(&a.tag).cmp(&Ver::parse(&b.tag)))
}

/// This machine's name in a release, or None where no build is made.
pub fn target() -> Option<&'static str> { target_of(std::env::consts::OS, std::env::consts::ARCH) }
pub fn target_of(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) { ("linux", "x86_64") => Some("linux-x64"), ("linux", "aarch64") => Some("linux-arm64"), ("macos", "aarch64") => Some("darwin-arm64"), ("windows", "x86_64") => Some("windows-x64"), _ => None }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset { pub name: String, pub sha256: String, pub size: u64 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest { pub tag: String, pub version: String, pub channel: String, pub commit: String, pub assets: BTreeMap<String, Asset> }

/// The largest archive this will fetch: the manifest is signed, but a mistake there must not fill the disk.
pub const MAX_ASSET: u64 = 2 << 30;

pub fn parse(bytes: &[u8]) -> Result<Manifest, String> {
    let v: Value = serde_json::from_slice(bytes).map_err(|_| "the manifest is not JSON".to_string())?;
    if v["schema"].as_u64() != Some(1) { return Err("this manifest is from a newer Backplane".into()) }
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let mut assets = BTreeMap::new();
    for (t, a) in v["assets"].as_object().ok_or("the manifest lists no assets")? {
        assets.insert(t.clone(), Asset { name: a["name"].as_str().unwrap_or("").into(), sha256: a["sha256"].as_str().unwrap_or("").into(), size: a["size"].as_u64().unwrap_or(0) });
    }
    Ok(Manifest { tag: s("tag"), version: s("version"), channel: s("channel"), commit: s("commit"), assets })
}

/// The public keys this build trusts, from `BACKPLANE_UPDATE_KEYS` when it was compiled: `id:hex,id:hex`. There is no way to add one at run time. Empty: this build cannot update.
pub fn keys_of(raw: &str) -> Vec<(String, [u8; 32])> {
    raw.split(',').filter_map(|e| { let (id, h) = e.trim().split_once(':')?; let b: [u8; 32] = hex::decode(h).ok()?.try_into().ok()?; (!id.is_empty()).then(|| (id.to_string(), b)) }).collect()
}
pub fn keys() -> Vec<(String, [u8; 32])> { keys_of(option_env!("BACKPLANE_UPDATE_KEYS").unwrap_or("")) }

/// `manifest.json`'s bytes against `manifest.json.sig` (64 raw bytes) under the key `manifest.json.keyid` names. The bytes are checked before they are read as anything.
pub fn verify(bytes: &[u8], sig: &[u8], keyid: &str, keys: &[(String, [u8; 32])]) -> Result<(), String> {
    if keys.is_empty() { return Err("this build has no update key, so it cannot update itself".into()) }
    let Some((_, k)) = keys.iter().find(|(id, _)| id == keyid.trim()) else { return Err("the release is signed with a key this build does not trust".into()) };
    let s = ed25519_compact::Signature::from_slice(sig).map_err(|_| "the release's signature is malformed".to_string())?;
    ed25519_compact::PublicKey::new(*k).verify(bytes, &s).map_err(|_| "the release's signature does not match".to_string())
}

/// What a verified manifest must say to be this release for this machine on this channel: a signature proves who wrote it, not that it is the one that was asked for.
pub fn pick<'a>(m: &'a Manifest, tag: &str, channel: &str, target: &str) -> Result<&'a Asset, String> {
    if m.tag != tag || !tag_ok(tag) { return Err("the manifest is for another release".into()) }
    if m.version != tag[1..] { return Err("the manifest's version does not match its tag".into()) }
    if m.channel != tag_channel(tag) { return Err("the manifest's channel does not match its tag".into()) }
    if channel != "beta" && m.channel != "stable" { return Err("that release is not on this channel".into()) }
    let a = m.assets.get(target).ok_or_else(|| format!("no build for {target} in this release yet"))?;
    if a.name != format!("backplane-{tag}-{target}.tar.gz") { return Err("the manifest names an unexpected file".into()) }
    if a.sha256.len() != 64 || !a.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) { return Err("the manifest's checksum is malformed".into()) }
    if a.size == 0 || a.size > MAX_ASSET { return Err("the manifest's size is out of range".into()) }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_compact::{KeyPair, Seed};
    fn v(s: &str) -> Ver { Ver::parse(s) }
    fn rel(t: &str, pre: bool) -> Rel { Rel { tag: t.into(), prerelease: pre, draft: false, signed: true } }
    fn pair() -> KeyPair { KeyPair::from_seed(Seed::new([7; 32])) }
    fn keys_for(p: &KeyPair) -> Vec<(String, [u8; 32])> { vec![("k1".into(), *p.pk)] }
    const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fn manifest(tag: &str, channel: &str) -> Manifest {
        let t = format!(r#"{{"schema":1,"tag":"{tag}","version":"{}","channel":"{channel}","commit":"c","assets":{{"linux-x64":{{"name":"backplane-{tag}-linux-x64.tar.gz","sha256":"{SHA}","size":9}}}}}}"#, &tag[1..]);
        parse(t.as_bytes()).unwrap()
    }

    #[test] fn versions_keep_014s_laws() {
        assert!(!v("1.2.3").newer(&v("v1.2.3")), "update_never_same");
        assert_eq!(v("1.2.3").cmp(&v("1.2.3")), Ordering::Equal, "semver_cmp_refl");
        assert!(v("1.2.3").newer(&v("1.2.3-beta.9")), "semver_pre_before_release");
        assert!(v("v0.3.1-nightly.20260923.2").newer(&v("v0.3.1-nightly.20260923.1")), "semver_nightly_later");
        assert!(v("v0.3.1-nightly.20260924.1").newer(&v("v0.3.1-nightly.20260923.9")), "semver_nightly_next_day");
        assert!(v("0.15.1-beta.10").newer(&v("0.15.1-beta.2")), "numbers, not text");
        assert!(v("0.10.0").newer(&v("0.9.9")), "numbers, not text");
        assert!(v("0.15.0").newer(&v("0.0.2")));
    }
    #[test] fn a_stored_channel_keeps_working() {
        assert_eq!((channel("stable"), channel("beta"), channel("nightly")), ("stable", "beta", "beta"), "0.14's nightly is beta now");
        assert_eq!((channel(""), channel("canary"), channel(" beta ")), ("stable", "stable", "beta"), "the unknown gets the safe one");
    }
    #[test] fn only_the_exact_tag_shapes_pass_because_a_tag_becomes_a_folder() {
        for t in ["v0.15.1", "v0.15.1-beta.2", "v0.3.1-nightly.20260923.1"] { assert!(tag_ok(t), "{t}"); }
        for t in ["", "0.15.1", "v0.15", "v0.15.1.2", "v0.15.1-rc.1", "v0.15.1-beta", "v0.15.1-beta.x", "v0.15.1/../../x", "v../../x", "v0.15.1-beta.2 ", "v٠.1.2", "v99999999999.1.2"] { assert!(!tag_ok(t), "{t:?}"); }
    }
    #[test] fn stable_takes_full_releases_and_beta_takes_both() {
        let rs = [rel("v0.16.0", false), rel("v0.17.0-beta.1", true)];
        assert_eq!(offer("0.15.0", "stable", &rs).unwrap().tag, "v0.16.0");
        assert_eq!(offer("0.15.0", "beta", &rs).unwrap().tag, "v0.17.0-beta.1");
        assert!(!wants("stable", &rel("v0.16.0", true)), "flagged prerelease on GitHub: not stable, whatever the tag says");
        assert!(!wants("stable", &rel("v0.16.0-beta.1", false)), "a beta tag not flagged: still not stable");
    }
    #[test] fn the_order_of_the_list_never_decides() {
        let a = [rel("v0.16.0", false), rel("v0.16.2", false), rel("v0.16.1", false)]; let mut b = a.clone(); b.reverse();
        assert_eq!(offer("0.15.0", "stable", &a).unwrap().tag, "v0.16.2"); assert_eq!(offer("0.15.0", "stable", &b).unwrap().tag, "v0.16.2");
    }
    #[test] fn nothing_older_or_equal_is_ever_offered() {
        let rs = [rel("v0.15.0", false), rel("v0.14.1", false)];
        assert_eq!(offer("0.15.0", "stable", &rs), None);
        assert_eq!(offer("0.16.0-beta.2", "stable", &[rel("v0.15.9", false)]), None, "beta to stable waits for a newer stable");
        assert_eq!(offer("0.16.0-beta.2", "stable", &[rel("v0.16.0", false)]).unwrap().tag, "v0.16.0", "the release itself is newer than its beta");
    }
    #[test] fn a_half_published_release_is_never_offered() {
        let mut d = rel("v0.16.0", false); d.draft = true; let mut u = rel("v0.16.1", false); u.signed = false;
        assert_eq!(offer("0.15.0", "stable", &[d, u]), None);
    }
    #[test] fn targets_are_the_ones_a_release_names() {
        assert_eq!(target_of("linux", "x86_64"), Some("linux-x64")); assert_eq!(target_of("linux", "aarch64"), Some("linux-arm64")); assert_eq!(target_of("macos", "aarch64"), Some("darwin-arm64"));
        assert_eq!(target_of("macos", "x86_64"), None, "no build is made for it"); assert_eq!(target_of("freebsd", "x86_64"), None);
    }
    #[test] fn the_verifier_passes_rfc_8032s_vector() {
        let pk: [u8; 32] = hex::decode("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c").unwrap().try_into().unwrap();
        let sig = hex::decode("92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00").unwrap();
        let k = vec![("r".to_string(), pk)];
        assert_eq!(verify(&[0x72], &sig, "r", &k), Ok(())); assert!(verify(&[0x73], &sig, "r", &k).is_err());
    }
    #[test] fn a_signature_covers_every_byte_and_only_trusted_keys_count() {
        let p = pair(); let k = keys_for(&p); let body = br#"{"schema":1}"#; let sig = p.sk.sign(body, None);
        assert_eq!(verify(body, sig.as_ref(), "k1\n", &k), Ok(()), "the keyid file may end in a newline");
        assert!(verify(br#"{"schema":2}"#, sig.as_ref(), "k1", &k).is_err(), "one byte changed");
        assert!(verify(br#"{"schema":1} "#, sig.as_ref(), "k1", &k).is_err(), "one byte added");
        assert!(verify(body, &sig.as_ref()[..63], "k1", &k).is_err(), "short");
        assert!(verify(body, sig.as_ref(), "k2", &k).unwrap_err().contains("does not trust"));
        let other = KeyPair::from_seed(Seed::new([8; 32]));
        assert!(verify(body, other.sk.sign(body, None).as_ref(), "k1", &k).is_err(), "another key's signature under our id");
    }
    #[test] fn a_build_with_no_key_refuses_and_says_why() {
        let p = pair(); let body = b"x"; assert!(verify(body, p.sk.sign(body, None).as_ref(), "k1", &[]).unwrap_err().contains("no update key"));
        assert_eq!(keys_of(""), vec![]); assert_eq!(keys_of("nocolon,a:zz,:00"), vec![], "a bad entry is no key");
        assert_eq!(keys_of(&format!("a:{}", "00".repeat(32))).len(), 1);
    }
    #[test] fn a_good_manifest_names_its_asset() {
        assert_eq!(pick(&manifest("v0.16.0", "stable"), "v0.16.0", "stable", "linux-x64").unwrap().sha256, SHA);
        assert!(pick(&manifest("v0.16.0-beta.1", "beta"), "v0.16.0-beta.1", "beta", "linux-x64").is_ok());
    }
    #[test] fn an_older_signed_manifest_cannot_stand_in_for_a_newer_release() {
        assert!(pick(&manifest("v0.14.0", "stable"), "v0.16.0", "stable", "linux-x64").unwrap_err().contains("another release"));
    }
    #[test] fn a_beta_cannot_reach_a_stable_hub_by_what_it_says_of_itself() {
        assert!(pick(&manifest("v0.16.0-beta.1", "stable"), "v0.16.0-beta.1", "stable", "linux-x64").unwrap_err().contains("does not match its tag"));
        assert!(pick(&manifest("v0.16.0-beta.1", "beta"), "v0.16.0-beta.1", "stable", "linux-x64").unwrap_err().contains("not on this channel"));
    }
    #[test] fn a_machine_with_no_build_is_told_so() {
        assert!(pick(&manifest("v0.16.0", "stable"), "v0.16.0", "stable", "darwin-arm64").unwrap_err().contains("no build for darwin-arm64"));
    }
    #[test] fn a_manifest_cannot_name_another_file_or_a_bad_checksum_or_size() {
        let m = manifest("v0.16.0", "stable"); let with = |f: &dyn Fn(&mut Asset)| { let mut x = m.clone(); f(x.assets.get_mut("linux-x64").unwrap()); x };
        assert!(pick(&with(&|a| a.name = "../../backplane-hub".into()), "v0.16.0", "stable", "linux-x64").is_err());
        assert!(pick(&with(&|a| a.sha256 = "abc".into()), "v0.16.0", "stable", "linux-x64").is_err());
        assert!(pick(&with(&|a| a.sha256 = SHA.to_uppercase()), "v0.16.0", "stable", "linux-x64").is_err());
        assert!(pick(&with(&|a| a.size = 0), "v0.16.0", "stable", "linux-x64").is_err());
        assert!(pick(&with(&|a| a.size = MAX_ASSET + 1), "v0.16.0", "stable", "linux-x64").is_err());
    }
    #[test] fn a_newer_schema_or_junk_is_refused() {
        assert!(parse(br#"{"schema":2,"assets":{}}"#).unwrap_err().contains("newer")); assert!(parse(b"not json").is_err()); assert!(parse(br#"{"schema":1}"#).is_err());
    }
}

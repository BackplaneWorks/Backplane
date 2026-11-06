//! Finding, fetching and staging a release, then handing it to the guard. The network and `tar` come in as functions, so every way this can fail has a test with no network.
//!
//! The order is the safety: the small signed files are checked before the archive is fetched, the archive is checked before it is unpacked, and a build appears in `builds/<tag>`
//! by one rename, whole or not at all. From there the guard (`native/guard`) owns everything: it alone moves `CURRENT`, and only once the new build has proved itself.
use super::rel::{self, Asset, Rel};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::{Path, PathBuf}};

pub const FEED: &str = "https://api.github.com/repos/BackplaneWorks/Backplane/releases";
/// The signed files, which the release workflow uploads last: a release without all three is half published.
pub const SIGNED: [&str; 3] = ["manifest.json", "manifest.json.sig", "manifest.json.keyid"];
const SMALL: u64 = 1 << 20;

/// Fetch a URL into a file, refusing more than the given bytes.
pub type Get<'a> = &'a dyn Fn(&str, &Path, u64) -> Result<(), String>;
/// Unpack an archive into an empty folder.
pub type Unpack<'a> = &'a dyn Fn(&Path, &Path) -> Result<(), String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed { pub rel: Rel, pub urls: BTreeMap<String, String> }

/// GitHub's release list: what each release is and where its files are.
pub fn listing(body: &[u8]) -> Result<Vec<Listed>, String> {
    let v: Value = serde_json::from_slice(body).map_err(|_| "the release list is not JSON".to_string())?;
    Ok(v.as_array().ok_or("the release list is not a list")?.iter().map(|r| {
        let urls: BTreeMap<String, String> = r["assets"].as_array().map(|a| a.iter().filter_map(|x| Some((x["name"].as_str()?.to_string(), x["browser_download_url"].as_str()?.to_string()))).collect()).unwrap_or_default();
        let signed = SIGNED.iter().all(|n| urls.contains_key(*n));
        Listed { rel: Rel { tag: r["tag_name"].as_str().unwrap_or("").into(), prerelease: r["prerelease"].as_bool().unwrap_or(false), draft: r["draft"].as_bool().unwrap_or(false), signed }, urls }
    }).collect())
}

/// What this hub is and what it trusts. `bad`: builds the guard has marked bad here. They stay the newest release, so without this a failed update would be offered again at the next check, for ever.
pub struct Ask<'a> { pub current: &'a str, pub channel: &'a str, pub target: &'a str, pub keys: &'a [(String, [u8; 32])], pub bad: &'a [String] }
#[derive(Clone, Debug, PartialEq, Eq)]
/// `version` and `commit`: what the signed manifest says this build is. The guard holds a build on probation to them (`EXPECT`): a hub that answers as anything else has failed.
pub struct Found { pub tag: String, pub asset: Asset, pub url: String, pub version: String, pub commit: String }

/// The release to install, verified, or None when there is nothing newer. `dir` is scratch for the signed files.
pub fn find(ask: &Ask, list: &[Listed], dir: &Path, get: Get) -> Result<Option<Found>, String> {
    let rels: Vec<Rel> = list.iter().map(|l| l.rel.clone()).filter(|r| !ask.bad.contains(&r.tag)).collect();
    let Some(r) = rel::offer(ask.current, ask.channel, &rels) else { return Ok(None) };
    let Some(l) = list.iter().find(|l| l.rel.tag == r.tag) else { return Ok(None) };
    let read = |n: &str| -> Result<Vec<u8>, String> {
        let p = dir.join(n); get(&l.urls[n], &p, SMALL)?;
        fs::read(&p).map_err(|e| format!("{n}: {e}"))
    };
    let (body, sig, id) = (read(SIGNED[0])?, read(SIGNED[1])?, read(SIGNED[2])?);
    rel::verify(&body, &sig, &String::from_utf8_lossy(&id), ask.keys)?;
    let m = rel::parse(&body)?;
    let a = rel::pick(&m, &r.tag, ask.channel, ask.target)?.clone();
    let url = l.urls.get(&a.name).ok_or("the release does not carry the file its manifest names")?.clone();
    Ok(Some(Found { tag: r.tag.clone(), asset: a, url, version: m.version.clone(), commit: m.commit.clone() }))
}

fn sha256(p: &Path) -> Result<String, String> {
    let mut f = fs::File::open(p).map_err(|e| e.to_string())?; let (mut h, mut b) = (Sha256::new(), vec![0u8; 1 << 20]);
    loop { let n = f.read(&mut b).map_err(|e| e.to_string())?; if n == 0 { break } h.update(&b[..n]); }
    Ok(hex::encode(h.finalize()))
}
/// A build ready to run: the hub is there and runs, and nothing in it is a link (it could point outside its folder).
fn check_tree(d: &Path) -> Result<(), String> {
    let hub = fs::metadata(d.join("backplane-hub")).map_err(|_| "the archive has no backplane-hub".to_string())?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; if hub.permissions().mode() & 0o111 == 0 { return Err("the archive's backplane-hub cannot run".into()) } }
    if !hub.is_file() { return Err("the archive has no backplane-hub".into()) }
    let mut todo = vec![d.to_path_buf()];
    while let Some(x) = todo.pop() {
        for e in fs::read_dir(&x).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?; let t = e.file_type().map_err(|e| e.to_string())?;
            if t.is_symlink() { return Err("the archive holds a link".into()) }
            if t.is_dir() { todo.push(e.path()) }
        }
    }
    Ok(())
}

/// Fetch, check and unpack, then one rename makes `builds/<tag>`. Scratch sits beside it, on the same disk, and is gone however this ends.
pub fn stage(f: &Found, builds: &Path, get: Get, unpack: Unpack) -> Result<PathBuf, String> {
    let dest = builds.join("builds").join(&f.tag);
    if dest.exists() { return Ok(dest) } // only ever made whole, by the rename below
    let work = builds.join("builds").join(format!(".stage-{}-{}", f.tag, std::process::id()));
    let _ = fs::remove_dir_all(&work); fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let r = (|| {
        let a = work.join("archive.tar.gz"); get(&f.url, &a, f.asset.size)?;
        if fs::metadata(&a).map_err(|e| e.to_string())?.len() != f.asset.size { return Err("the download is the wrong size".to_string()) }
        if sha256(&a)? != f.asset.sha256 { return Err("the download's checksum does not match".to_string()) }
        let x = work.join("build"); fs::create_dir(&x).map_err(|e| e.to_string())?;
        unpack(&a, &x)?; check_tree(&x)?;
        // written after the tree check, by us, from the verified manifest: an archive cannot choose what its build is held to
        fs::write(x.join("EXPECT"), json!({"version":f.version,"commit":f.commit}).to_string()).map_err(|e| e.to_string())?;
        fs::rename(&x, &dest).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_dir_all(&work);
    r.map(|_| dest)
}

/// Ask the guard to move to a build: `<home>/guard/request`, written beside and renamed over. A request already waiting is refused, not replaced: it may be a restart the person asked for.
pub fn request(home: &Path, id: &str, tag: &str, at: u64) -> Result<(), String> {
    let g = home.join("guard");
    if !g.is_dir() { return Err("no guard is running here".into()) }
    let p = g.join("request");
    if p.exists() { return Err("the guard has a request waiting".into()) }
    let tmp = g.join(format!(".request.{}", std::process::id()));
    let body = json!({"id":id,"action":"update","build":tag,"from":"local","at":at}).to_string();
    let r = fs::write(&tmp, body).and_then(|_| fs::rename(&tmp, &p)).map_err(|e| e.to_string());
    if r.is_err() { let _ = fs::remove_file(&tmp); }
    r
}

/// How an update asked for before this start ended. A hub only learns this after a restart, so it is worked out from what it finds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome { Installed, RolledBack(String), Waiting }
/// `bad`: the guard's `BAD` lines (id, tab, why).
pub fn outcome(pending: &str, running: &str, bad: &[(String, String)]) -> Outcome {
    if let Some((_, why)) = bad.iter().find(|(id, _)| id == pending) { return Outcome::RolledBack(why.clone()) }
    if pending.strip_prefix('v').unwrap_or(pending) == running.strip_prefix('v').unwrap_or(running) { Outcome::Installed } else { Outcome::Waiting }
}

/// Builds that may go, by the guard's rule: never `CURRENT`, a pinned build, or the newest good one. Bad ones may go too, unless pinned.
pub fn prunable(all: &[String], current: &str, pins: &[String], good: &[String]) -> Vec<String> {
    let newest = good.last().map(String::as_str).unwrap_or("");
    all.iter().filter(|b| !b.starts_with('.') && *b != current && *b != newest && !pins.contains(b)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_compact::{KeyPair, Seed};
    use std::{cell::RefCell, collections::HashMap};

    struct Rig { d: PathBuf }
    impl Rig { fn new(t: &str) -> Rig { let d = std::env::temp_dir().join(format!("bp-upd-{}-{t}", std::process::id())); let _ = fs::remove_dir_all(&d); fs::create_dir_all(d.join("scratch")).unwrap(); fs::create_dir_all(d.join("b/builds")).unwrap(); Rig { d } } }
    impl Drop for Rig { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.d); } }
    fn pair() -> KeyPair { KeyPair::from_seed(Seed::new([9; 32])) }
    fn keys() -> Vec<(String, [u8; 32])> { vec![("k".into(), *pair().pk)] }
    const T: &str = "linux-x64";
    fn sha(b: &[u8]) -> String { hex::encode(Sha256::digest(b)) }
    /// A feed held in memory: url -> bytes, and what was asked for.
    struct Feed { files: HashMap<String, Vec<u8>>, asked: RefCell<Vec<String>> }
    impl Feed {
        fn new() -> Feed { Feed { files: HashMap::new(), asked: RefCell::new(vec![]) } }
        fn get(&self, url: &str, out: &Path, max: u64) -> Result<(), String> {
            self.asked.borrow_mut().push(url.into());
            let b = self.files.get(url).ok_or("404")?; if b.len() as u64 > max { return Err("too big".into()) } fs::write(out, b).map_err(|e| e.to_string())
        }
        fn release(&mut self, tag: &str, archive: &[u8]) -> Listed {
            let name = format!("backplane-{tag}-{T}.tar.gz");
            let m = json!({"schema":1,"tag":tag,"version":&tag[1..],"channel":rel::tag_channel(tag),"commit":"c","assets":{T:{"name":name,"sha256":sha(archive),"size":archive.len()}}}).to_string().into_bytes();
            let s = pair().sk.sign(&m, None).as_ref().to_vec();
            let mut urls = BTreeMap::new();
            for (n, b) in [("manifest.json", m), ("manifest.json.sig", s), ("manifest.json.keyid", b"k\n".to_vec()), (name.as_str(), archive.to_vec())] { let u = format!("mem://{tag}/{n}"); self.files.insert(u.clone(), b); urls.insert(n.to_string(), u); }
            Listed { rel: Rel { tag: tag.into(), prerelease: tag.contains('-'), draft: false, signed: true }, urls }
        }
    }
    fn ask<'a>(k: &'a [(String, [u8; 32])], bad: &'a [String]) -> Ask<'a> { Ask { current: "0.15.0", channel: "stable", target: T, keys: k, bad } }
    /// "unpacks" by writing a working build, or what `f` makes of the folder
    fn good_tree(_: &Path, d: &Path) -> Result<(), String> {
        fs::write(d.join("backplane-hub"), b"x").map_err(|e| e.to_string())?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(d.join("backplane-hub"), fs::Permissions::from_mode(0o755)).unwrap(); }
        Ok(())
    }

    #[test] fn the_list_says_what_is_signed() {
        let b = br#"[{"tag_name":"v0.16.0","prerelease":false,"draft":false,"assets":[{"name":"manifest.json","browser_download_url":"u1"},{"name":"manifest.json.sig","browser_download_url":"u2"},{"name":"manifest.json.keyid","browser_download_url":"u3"}]},
                     {"tag_name":"v0.17.0","assets":[{"name":"manifest.json","browser_download_url":"u4"}]}]"#;
        let l = listing(b).unwrap(); assert!(l[0].rel.signed); assert!(!l[1].rel.signed, "the signature is uploaded last: not there yet, half published");
        assert!(listing(b"{\"message\":\"rate limited\"}").is_err()); assert!(listing(b"<html>").is_err());
    }
    #[test] fn a_signed_newer_release_is_found_before_its_archive_is_fetched() {
        let r = Rig::new("find"); let mut f = Feed::new(); let l = f.release("v0.16.0", b"archive"); let k = keys();
        let got = find(&ask(&k, &[]), &[l], &r.d.join("scratch"), &|u, o, m| f.get(u, o, m)).unwrap().unwrap();
        assert_eq!(got.tag, "v0.16.0"); assert_eq!(got.asset.sha256, sha(b"archive"));
        assert!(f.asked.borrow().iter().all(|u| u.contains("manifest")), "only the small signed files so far");
    }
    #[test] fn nothing_newer_fetches_nothing() {
        let r = Rig::new("none"); let mut f = Feed::new(); let l = f.release("v0.15.0", b"a"); let k = keys();
        assert_eq!(find(&ask(&k, &[]), &[l], &r.d.join("scratch"), &|u, o, m| f.get(u, o, m)).unwrap(), None); assert!(f.asked.borrow().is_empty());
    }
    #[test] fn a_build_that_failed_here_is_not_offered_again() {
        let r = Rig::new("bad"); let mut f = Feed::new(); let l = f.release("v0.16.0", b"a"); let k = keys(); let bad = vec!["v0.16.0".to_string()];
        assert_eq!(find(&ask(&k, &bad), &[l.clone()], &r.d.join("scratch"), &|u, o, m| f.get(u, o, m)).unwrap(), None, "else update and rollback go round for ever");
        let n = f.release("v0.16.1", b"b");
        assert_eq!(find(&ask(&k, &bad), &[l, n], &r.d.join("scratch"), &|u, o, m| f.get(u, o, m)).unwrap().unwrap().tag, "v0.16.1", "the next release still comes");
    }
    #[test] fn a_tampered_manifest_stops_before_the_archive() {
        let r = Rig::new("tamper"); let mut f = Feed::new(); let l = f.release("v0.16.0", b"a"); let k = keys();
        let u = l.urls["manifest.json"].clone(); let mut m = f.files[&u].clone(); let n = m.len(); m[n - 2] ^= 1; f.files.insert(u, m);
        assert!(find(&ask(&k, &[]), &[l], &r.d.join("scratch"), &|u, o, m| f.get(u, o, m)).unwrap_err().contains("signature"));
        assert!(!f.asked.borrow().iter().any(|u| u.ends_with(".tar.gz")));
    }
    #[test] fn an_older_release_replayed_under_a_new_tag_is_refused() {
        let r = Rig::new("replay"); let mut f = Feed::new(); let old = f.release("v0.15.5", b"a"); let mut l = f.release("v0.16.0", b"b"); let k = keys();
        for n in SIGNED { l.urls.insert(n.into(), old.urls[n].clone()); }
        assert!(find(&ask(&k, &[]), &[l], &r.d.join("scratch"), &|u, o, m| f.get(u, o, m)).unwrap_err().contains("another release"));
    }
    #[test] fn a_build_with_no_key_finds_nothing_it_can_install() {
        let r = Rig::new("nokey"); let mut f = Feed::new(); let l = f.release("v0.16.0", b"a");
        assert!(find(&ask(&[], &[]), &[l], &r.d.join("scratch"), &|u, o, m| f.get(u, o, m)).unwrap_err().contains("no update key"));
    }
    fn found(f: &mut Feed, r: &Rig, b: &[u8]) -> Found { let l = f.release("v0.16.0", b); let k = keys(); let g = &*f; find(&ask(&k, &[]), &[l], &r.d.join("scratch"), &|u, o, m| g.get(u, o, m)).unwrap().unwrap() }
    fn left(r: &Rig) -> Vec<String> { fs::read_dir(r.d.join("b/builds")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect() }

    #[test] fn a_good_archive_becomes_a_build_by_one_rename() {
        let r = Rig::new("stage"); let mut f = Feed::new(); let x = found(&mut f, &r, b"archive");
        let d = stage(&x, &r.d.join("b"), &|u, o, m| f.get(u, o, m), &good_tree).unwrap();
        assert!(d.join("backplane-hub").is_file()); assert_eq!(left(&r), vec!["v0.16.0"], "no scratch left");
        let e: Value = serde_json::from_str(&fs::read_to_string(d.join("EXPECT")).unwrap()).unwrap();
        assert_eq!((e["version"].as_str(), e["commit"].as_str()), (Some("0.16.0"), Some("c")), "the guard's probation check, from the signed manifest");
    }
    #[test] fn a_corrupt_archive_is_never_unpacked_and_leaves_nothing() {
        let r = Rig::new("corrupt"); let mut f = Feed::new(); let x = found(&mut f, &r, b"archive"); f.files.insert(x.url.clone(), b"ARCHIVE".to_vec());
        let e = stage(&x, &r.d.join("b"), &|u, o, m| f.get(u, o, m), &|_, _| panic!("unpacked before the checksum")).unwrap_err();
        assert!(e.contains("checksum")); assert!(left(&r).is_empty());
    }
    #[test] fn a_short_or_long_download_is_refused() {
        let r = Rig::new("size"); let mut f = Feed::new(); let x = found(&mut f, &r, b"archive"); f.files.insert(x.url.clone(), b"arch".to_vec());
        assert!(stage(&x, &r.d.join("b"), &|u, o, m| f.get(u, o, m), &good_tree).unwrap_err().contains("wrong size"));
        f.files.insert(x.url.clone(), b"archive and more".to_vec());
        assert!(stage(&x, &r.d.join("b"), &|u, o, m| f.get(u, o, m), &good_tree).is_err(), "the size cap reaches the fetch"); assert!(left(&r).is_empty());
    }
    #[test] fn an_archive_without_a_hub_leaves_nothing() {
        let r = Rig::new("nohub"); let mut f = Feed::new(); let x = found(&mut f, &r, b"archive");
        assert!(stage(&x, &r.d.join("b"), &|u, o, m| f.get(u, o, m), &|_, _| Ok(())).unwrap_err().contains("no backplane-hub")); assert!(left(&r).is_empty());
    }
    #[cfg(unix)] #[test] fn a_hub_that_cannot_run_or_a_link_is_refused() {
        let r = Rig::new("tree"); let mut f = Feed::new(); let x = found(&mut f, &r, b"archive");
        assert!(stage(&x, &r.d.join("b"), &|u, o, m| f.get(u, o, m), &|_, d| { fs::write(d.join("backplane-hub"), b"x").unwrap(); Ok(()) }).unwrap_err().contains("cannot run"));
        let e = stage(&x, &r.d.join("b"), &|u, o, m| f.get(u, o, m), &|a, d| { good_tree(a, d)?; fs::create_dir(d.join("assets")).unwrap(); std::os::unix::fs::symlink("/etc", d.join("assets/x")).unwrap(); Ok(()) }).unwrap_err();
        assert!(e.contains("link")); assert!(left(&r).is_empty());
    }
    #[test] fn a_failed_fetch_leaves_nothing() {
        let r = Rig::new("fail"); let mut f = Feed::new(); let x = found(&mut f, &r, b"archive");
        assert!(stage(&x, &r.d.join("b"), &|_, _, _| Err("network gone".into()), &good_tree).unwrap_err().contains("network gone")); assert!(left(&r).is_empty());
    }
    #[test] fn a_request_reaches_the_guard_whole_and_never_replaces_one_waiting() {
        let r = Rig::new("req"); assert!(request(&r.d, "u1", "v0.16.0", 5).unwrap_err().contains("no guard"), "nothing would act on it");
        fs::create_dir(r.d.join("guard")).unwrap(); request(&r.d, "u1", "v0.16.0", 5).unwrap();
        let v: Value = serde_json::from_slice(&fs::read(r.d.join("guard/request")).unwrap()).unwrap();
        assert_eq!(v, json!({"id":"u1","action":"update","build":"v0.16.0","from":"local","at":5}));
        assert!(request(&r.d, "u2", "v0.16.1", 6).unwrap_err().contains("waiting"), "it may be a restart the person asked for");
        assert_eq!(fs::read_dir(r.d.join("guard")).unwrap().count(), 1, "no scratch left");
    }
    #[test] fn after_a_restart_the_hub_works_out_how_the_update_ended() {
        assert_eq!(outcome("v0.16.0", "0.16.0", &[]), Outcome::Installed);
        assert_eq!(outcome("v0.16.0", "0.15.0", &[("v0.16.0".into(), "it panicked".into())]), Outcome::RolledBack("it panicked".into()));
        assert_eq!(outcome("v0.16.0", "0.15.0", &[]), Outcome::Waiting, "the guard has not moved yet");
        assert_eq!(outcome("v0.16.0", "0.16.0", &[("v0.16.0".into(), "late".into())]), Outcome::RolledBack("late".into()), "marked bad wins");
    }
    #[test] fn pruning_keeps_what_the_guard_needs() {
        let s = |x: &[&str]| x.iter().map(|y| y.to_string()).collect::<Vec<_>>();
        assert_eq!(prunable(&s(&["a", "b", "c", "d", "e", ".stage-x"]), "e", &s(&["b"]), &s(&["a", "d"])), s(&["a", "c"]), "current e, pinned b, newest good d, scratch never");
    }
}

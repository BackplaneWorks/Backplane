//! Test-only temporary folders and files (`crate::plat::tmp`).
//!
//! Every test folder lives under `std::env::temp_dir()` as `TMPDIR` sets it — except on Unix when `TMPDIR` was left unset or pointed at
//! `/tmp` (a 30 GB RAM tmpfs with an inode quota; `scripts/lib-bptmp.sh`), where it falls back to `BP_TMP` or `~/.cache/bp-tmp` on disk.
//! Dropping `TempDir`, `TempStr` or `TempFile` removes its folder (read-only git packfiles and mode-000 test folders included), whether the
//! test returned normally or unwound from a panic.
#![allow(dead_code)]

use crate::plat::fs::PermissionsExt;
use std::{
    ffi::OsStr,
    fmt, fs,
    ops::Deref,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Pure path resolution for [`base`]: redirects `/tmp` onto `bp_tmp` or `$HOME/.cache/bp-tmp` on Unix.
#[cfg(unix)]
fn redirected_unix(td: &Path, bp_tmp: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    if td != Path::new("/tmp") && !td.starts_with("/tmp/") {
        return None;
    }
    bp_tmp
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p != Path::new("/tmp") && !p.starts_with("/tmp/"))
        .or_else(|| {
            home.filter(|v| !v.is_empty())
                .map(|h| PathBuf::from(h).join(".cache").join("bp-tmp"))
        })
}

/// Base folder for test scratch: `std::env::temp_dir()` (honouring `TMPDIR`), redirected off `/tmp` onto `BP_TMP` / `~/.cache/bp-tmp` on Unix
/// when `TMPDIR` was unset or `/tmp`.
pub fn base() -> PathBuf {
    let t = std::env::temp_dir();
    #[cfg(unix)]
    if let Some(d) = redirected_unix(&t, std::env::var_os("BP_TMP").as_deref(), std::env::var_os("HOME").as_deref()) {
        let _ = fs::create_dir_all(&d);
        return d;
    }
    t
}

fn make_writable(p: &Path) {
    let Ok(m) = fs::symlink_metadata(p) else { return };
    if m.file_type().is_symlink() {
        return;
    }
    if m.is_dir() {
        let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o700));
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                make_writable(&e.path());
            }
        }
    } else {
        let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o600));
    }
}

pub fn rm_tree(p: &Path) {
    if p.as_os_str().is_empty() {
        return;
    }
    let _ = fs::remove_file(p);
    if fs::remove_dir_all(p).is_err() && p.exists() {
        make_writable(p);
        let _ = fs::remove_dir_all(p);
    }
}

fn fresh_path(tag: &str) -> PathBuf {
    let clean: String = tag
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '-' })
        .collect();
    let pfx = if clean.starts_with("bp-") || clean.starts_with("bp ") {
        clean
    } else if clean.is_empty() {
        "bp-t".to_string()
    } else {
        format!("bp-{clean}")
    };
    base().join(format!("{pfx}-{}-{}", std::process::id(), SEQ.fetch_add(1, Relaxed)))
}

#[must_use = "dropping TempDir removes the folder at once; bind it to a named variable"]
pub struct TempDir {
    path: PathBuf,
    kept: Vec<PathBuf>,
}

impl TempDir {
    pub fn path(&self) -> &Path { &self.path }
    pub fn to_path_buf(&self) -> PathBuf { self.path.clone() }
    /// Keeps `other` alive for as long as `self`, removing both when `self` drops.
    pub fn keep(&mut self, mut other: TempDir) {
        if !other.path.as_os_str().is_empty() {
            self.kept.push(std::mem::take(&mut other.path));
        }
        self.kept.append(&mut other.kept);
    }
}

impl Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path { &self.path }
}
impl AsRef<Path> for TempDir { fn as_ref(&self) -> &Path { &self.path } }
impl AsRef<OsStr> for TempDir { fn as_ref(&self) -> &OsStr { self.path.as_os_str() } }
impl fmt::Debug for TempDir { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.path.fmt(f) } }
impl fmt::Display for TempDir { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.path.display().fmt(f) } }
impl PartialEq<Path> for TempDir { fn eq(&self, other: &Path) -> bool { self.path == other } }
impl PartialEq<&Path> for TempDir { fn eq(&self, other: &&Path) -> bool { self.path == *other } }
impl PartialEq<PathBuf> for TempDir { fn eq(&self, other: &PathBuf) -> bool { &self.path == other } }

impl Drop for TempDir {
    fn drop(&mut self) {
        rm_tree(&self.path);
        for p in self.kept.drain(..) {
            rm_tree(&p);
        }
    }
}

#[must_use = "dropping TempStr removes the folder at once; bind it to a named variable"]
pub struct TempStr {
    dir: TempDir,
    s: String,
}

impl TempStr {
    pub fn as_str(&self) -> &str { &self.s }
    pub fn path(&self) -> &Path { &self.dir }
    pub fn keep(&mut self, other: TempDir) { self.dir.keep(other); }
}

impl Deref for TempStr {
    type Target = str;
    fn deref(&self) -> &str { &self.s }
}
impl AsRef<str> for TempStr { fn as_ref(&self) -> &str { &self.s } }
impl AsRef<Path> for TempStr { fn as_ref(&self) -> &Path { &self.dir } }
impl AsRef<OsStr> for TempStr { fn as_ref(&self) -> &OsStr { self.dir.as_ref() } }
impl fmt::Debug for TempStr { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.s.fmt(f) } }
impl fmt::Display for TempStr { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.s.fmt(f) } }

#[must_use = "dropping TempFile removes its folder at once; bind it to a named variable"]
pub struct TempFile {
    _dir: TempDir,
    path: PathBuf,
}

impl TempFile {
    pub fn path(&self) -> &Path { &self.path }
    pub fn to_path_buf(&self) -> PathBuf { self.path.clone() }
}

impl Deref for TempFile {
    type Target = Path;
    fn deref(&self) -> &Path { &self.path }
}
impl AsRef<Path> for TempFile { fn as_ref(&self) -> &Path { &self.path } }
impl AsRef<OsStr> for TempFile { fn as_ref(&self) -> &OsStr { self.path.as_os_str() } }
impl fmt::Debug for TempFile { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.path.fmt(f) } }
impl fmt::Display for TempFile { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.path.display().fmt(f) } }

/// Fresh existing folder `<base>/bp-<tag>-<pid>-<seq>`, removed on drop.
pub fn dir(tag: &str) -> TempDir {
    let p = fresh_path(tag);
    rm_tree(&p);
    fs::create_dir_all(&p).unwrap();
    TempDir { path: p, kept: Vec::new() }
}

/// Fresh non-existent path `<base>/bp-<tag>-<pid>-<seq>`, removed on drop if created by the test.
pub fn uncreated(tag: &str) -> TempDir {
    let p = fresh_path(tag);
    rm_tree(&p);
    TempDir { path: p, kept: Vec::new() }
}

/// Fresh existing folder whose canonical path is returned (for tests comparing against `canonicalize`, e.g. macOS `/private/var`), removed on drop.
pub fn canonical(tag: &str) -> TempDir {
    let p = fresh_path(tag);
    rm_tree(&p);
    fs::create_dir_all(&p).unwrap();
    let c = crate::plat::fs::canonicalize(&p).unwrap();
    let kept = if c == p { Vec::new() } else { vec![p] };
    TempDir { path: c, kept }
}

/// Fresh existing folder as a `String`-like guard, removed on drop.
pub fn dir_str(tag: &str) -> TempStr {
    let d = dir(tag);
    let s = d.to_string_lossy().into_owned();
    TempStr { dir: d, s }
}

/// Fresh existing canonicalized folder as a `String`-like guard, removed on drop.
pub fn canonical_str(tag: &str) -> TempStr {
    let d = canonical(tag);
    let s = d.to_string_lossy().into_owned();
    TempStr { dir: d, s }
}

/// Fresh file path inside its own isolated `TempDir`, removed (along with any sidecars) on drop.
pub fn file(tag: &str, name: &str) -> TempFile {
    let d = dir(tag);
    let path = d.join(name);
    TempFile { _dir: d, path }
}

/// Fresh existing folder `<base>/bp-<tag>-<pid>-<seq>/<leaf>` whose `file_name()` is exact `leaf`, removing the outer `bp-*` folder on drop.
pub fn named(tag: &str, leaf: &str) -> TempDir {
    let outer = fresh_path(tag);
    rm_tree(&outer);
    let inner = outer.join(leaf);
    fs::create_dir_all(&inner).unwrap();
    TempDir { path: inner, kept: vec![outer] }
}

/// Short existing folder for Unix domain sockets (`SUN_LEN` is 104 bytes on macOS, 108 on Linux), removed on drop.
pub fn short_dir(tag: &str) -> TempDir {
    let mut b = base();
    #[cfg(unix)]
    if b.as_os_str().len() > 44 {
        if let Some(h) = std::env::var_os("HOME").filter(|v| !v.is_empty()).map(|h| PathBuf::from(h).join(".cache").join("bp-tmp")).filter(|p| p.as_os_str().len() <= 44) {
            let _ = fs::create_dir_all(&h);
            b = h;
        }
    }
    let p = b.join(format!("bp-{tag}-{}-{}", std::process::id(), SEQ.fetch_add(1, Relaxed)));
    rm_tree(&p);
    fs::create_dir_all(&p).unwrap();
    TempDir { path: p, kept: Vec::new() }
}

/// Short existing folder as a `String`-like guard (for Unix domain sockets), removed on drop.
pub fn short_dir_str(tag: &str) -> TempStr {
    let d = short_dir(tag);
    let s = d.to_string_lossy().into_owned();
    TempStr { dir: d, s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn base_honours_explicit_tmpdir_and_redirects_slash_tmp_onto_bp_tmp() {
        assert_eq!(
            redirected_unix(Path::new("/var/scratch/custom"), Some(OsStr::new("/disk/bp-tmp")), Some(OsStr::new("/h"))),
            None,
            "an explicit non-/tmp TMPDIR is kept as std::env::temp_dir() gave it"
        );
        assert_eq!(
            redirected_unix(Path::new("/tmp"), Some(OsStr::new("/disk/bp-tmp")), Some(OsStr::new("/h"))),
            Some(PathBuf::from("/disk/bp-tmp")),
            "/tmp redirects onto BP_TMP"
        );
        assert_eq!(
            redirected_unix(Path::new("/tmp/sub"), Some(OsStr::new("/tmp")), Some(OsStr::new("/h"))),
            Some(PathBuf::from("/h/.cache/bp-tmp")),
            "BP_TMP=/tmp falls back to ~/.cache/bp-tmp"
        );
    }

    #[test]
    fn drop_guards_remove_readonly_and_unreadable_trees_on_return_and_on_panic() {
        let (p_dir, p_kept, p_named_outer, p_file_dir);
        {
            let mut d = dir("guard-ok");
            p_dir = d.to_path_buf();
            let k = dir("guard-kept");
            p_kept = k.to_path_buf();
            d.keep(k);
            let ro = d.join("pack.idx");
            fs::write(&ro, b"pack").unwrap();
            fs::set_permissions(&ro, fs::Permissions::from_mode(0o444)).unwrap();
            #[cfg(unix)]
            {
                let locked = d.join("locked");
                fs::create_dir_all(&locked).unwrap();
                fs::write(locked.join("inside"), b"x").unwrap();
                fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
            }
            let n = named("guard-named", "leaf");
            assert_eq!(n.file_name().unwrap(), "leaf");
            p_named_outer = n.parent().unwrap().to_path_buf();
            let f = file("guard-file", "a.json");
            fs::write(&f, b"{}").unwrap();
            p_file_dir = f.parent().unwrap().to_path_buf();
        }
        for p in [&p_dir, &p_kept, &p_named_outer, &p_file_dir] {
            assert!(!p.exists(), "left behind after normal drop: {}", p.display());
        }

        let panic_path = std::sync::Mutex::new(PathBuf::new());
        let r = std::panic::catch_unwind(|| {
            let d = dir("guard-panic");
            *panic_path.lock().unwrap() = d.to_path_buf();
            let ro = d.join("readonly");
            fs::write(&ro, b"x").unwrap();
            fs::set_permissions(&ro, fs::Permissions::from_mode(0o444)).unwrap();
            panic!("simulated test failure");
        });
        assert!(r.is_err());
        let p = panic_path.into_inner().unwrap();
        assert!(!p.as_os_str().is_empty() && !p.exists(), "left behind after panic unwind: {}", p.display());
    }

    /// Runs representative tests from the modules that previously leaked `bp-*` directories into `/tmp` (`zone`, `commands`, `bots`,
    /// `acp`, `checkpoint`) with a dedicated scratch `TMPDIR` and fails if any `bp-*` entry (or anything else) remains afterward.
    #[test]
    fn representative_hub_tests_leave_no_bp_dirs_behind() {
        if std::env::var_os("BP_TMP_LEAK_PROBE").is_some() {
            return;
        }
        let exe = std::env::current_exe().unwrap();
        if !exe.file_name().is_some_and(|n| n.to_string_lossy().starts_with("backplane_hub")) {
            return;
        }
        let scratch = dir("leak-probe");
        let mut cases = vec![
            "settings::zone::tests::the_rule_is_the_footer_of_the_tzif_file",
            "commands::tests::the_flag_requests_log_their_event_and_acknowledge",
            "bots::tests::bot_create_makes_a_sub_bot_within_the_depth_and_kid_limits",
        ];
        #[cfg(unix)]
        cases.extend([
            "agents::providers::acp::e2e::grok_turn_matches_the_js_hub",
            "agents::tests::checkpoint::a_turn_waiting_for_the_earlier_checkpoint_says_so_and_goes_on_when_it_ends",
        ]);
        for tc in cases {
            let out = std::process::Command::new(&exe)
                .args([tc, "--exact", "--nocapture", "--test-threads=1"])
                .env("BP_TMP_LEAK_PROBE", "1")
                .env("TMPDIR", scratch.path())
                .env("TMP", scratch.path())
                .env("TEMP", scratch.path())
                .env("BP_TMP", scratch.path())
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(out.status.success() && stdout.contains("1 passed;"), "{tc} did not run cleanly:\n{stdout}{stderr}");
            let left: Vec<String> = fs::read_dir(&scratch)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            assert!(left.is_empty(), "{tc} left temp entries behind in {}: {left:?}", scratch.display());
        }
    }
}
